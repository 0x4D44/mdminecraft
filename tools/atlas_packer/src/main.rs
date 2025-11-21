use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use image::{imageops::FilterType, ImageReader, RgbaImage};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about = "Texture atlas packing utility for mdminecraft", long_about = None)]
struct Args {
    /// Directory containing source textures (PNG/JPEG)
    #[arg(short, long)]
    input: PathBuf,

    /// Output atlas image path (PNG)
    #[arg(long, default_value = "atlas.png")]
    output_image: PathBuf,

    /// Output metadata JSON path
    #[arg(long, default_value = "atlas.json")]
    output_meta: PathBuf,

    /// Expected square tile size in pixels; inferred from first texture if omitted
    #[arg(long)]
    tile_size: Option<u32>,

    /// Padding (in pixels) around each tile inside the atlas
    #[arg(long, default_value_t = 2)]
    padding: u32,

    /// Maximum atlas dimension (width & height) in pixels
    #[arg(long, default_value_t = 4096)]
    max_atlas_size: u32,

    /// Override number of columns in the atlas grid (default: auto)
    #[arg(long)]
    columns: Option<u32>,

    /// Allow resizing textures that don't match the expected tile size
    #[arg(long)]
    allow_mixed_sizes: bool,
}

#[derive(Debug)]
struct Texture {
    name: String,
    image: RgbaImage,
}

#[derive(Debug, Serialize)]
struct AtlasMetadata {
    tile_size: u32,
    padding: u32,
    columns: u32,
    rows: u32,
    atlas_width: u32,
    atlas_height: u32,
    entries: Vec<AtlasEntry>,
}

#[derive(Debug, Serialize)]
struct AtlasEntry {
    name: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
}

fn main() -> Result<()> {
    let args = Args::parse();
    run(args)
}

fn run(args: Args) -> Result<()> {
    if !args.input.is_dir() {
        bail!("Input path {:?} is not a directory", args.input);
    }
    let (textures, tile_size) = load_textures(&args)?;
    if textures.is_empty() {
        bail!("No textures found in {}", args.input.display());
    }

    let atlas = build_atlas(&textures, tile_size, &args)?;

    if let Some(parent) = args.output_image.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {parent:?}"))?;
        }
    }
    atlas.image.save(&args.output_image).with_context(|| {
        format!(
            "Failed to write atlas image to {}",
            args.output_image.display()
        )
    })?;

    if let Some(parent) = args.output_meta.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {parent:?}"))?;
        }
    }
    let json = serde_json::to_string_pretty(&atlas.metadata)?;
    fs::write(&args.output_meta, json)
        .with_context(|| format!("Failed to write metadata to {}", args.output_meta.display()))?;

    println!(
        "Packed {} textures into {} ({}x{})",
        textures.len(),
        args.output_image.display(),
        atlas.metadata.atlas_width,
        atlas.metadata.atlas_height
    );
    println!("Metadata written to {}", args.output_meta.display());

    Ok(())
}

struct PackedAtlas {
    image: RgbaImage,
    metadata: AtlasMetadata,
}

fn build_atlas(textures: &[Texture], tile_size: u32, args: &Args) -> Result<PackedAtlas> {
    let padding = args.padding;
    let stride = tile_size + padding * 2;
    let count = textures.len() as u32;
    let columns = args
        .columns
        .filter(|&c| c > 0)
        .unwrap_or_else(|| (f64::from(count).sqrt().ceil() as u32).max(1));
    let rows = ((count + columns - 1) / columns).max(1);

    let width = columns * stride;
    let height = rows * stride;

    if width > args.max_atlas_size || height > args.max_atlas_size {
        bail!(
            "Atlas size {}x{} exceeds max {}",
            width,
            height,
            args.max_atlas_size
        );
    }

    let mut atlas = RgbaImage::new(width, height);
    let mut entries = Vec::with_capacity(textures.len());

    for (idx, tex) in textures.iter().enumerate() {
        let idx = idx as u32;
        let col = idx % columns;
        let row = idx / columns;
        let dest_x = col * stride + padding;
        let dest_y = row * stride + padding;

        copy_into(&tex.image, &mut atlas, dest_x, dest_y);

        let u0 = dest_x as f32 / width as f32;
        let v0 = dest_y as f32 / height as f32;
        let u1 = (dest_x + tile_size) as f32 / width as f32;
        let v1 = (dest_y + tile_size) as f32 / height as f32;

        entries.push(AtlasEntry {
            name: tex.name.clone(),
            x: dest_x,
            y: dest_y,
            width: tile_size,
            height: tile_size,
            u0,
            v0,
            u1,
            v1,
        });
    }

    let metadata = AtlasMetadata {
        tile_size,
        padding,
        columns,
        rows,
        atlas_width: width,
        atlas_height: height,
        entries,
    };

    Ok(PackedAtlas {
        image: atlas,
        metadata,
    })
}

fn copy_into(src: &RgbaImage, dst: &mut RgbaImage, x: u32, y: u32) {
    for yy in 0..src.height() {
        for xx in 0..src.width() {
            let pixel = src.get_pixel(xx, yy);
            dst.put_pixel(x + xx, y + yy, *pixel);
        }
    }
}

fn load_textures(args: &Args) -> Result<(Vec<Texture>, u32)> {
    let mut textures = BTreeMap::new();
    let mut inferred_size = args.tile_size;

    for entry in WalkDir::new(&args.input)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_texture_file(path) {
            continue;
        }

        let image = ImageReader::open(path)
            .with_context(|| format!("Failed to open {}", path.display()))?
            .decode()
            .with_context(|| format!("Failed to decode {}", path.display()))?
            .to_rgba8();

        let (w, h) = image.dimensions();
        if w != h {
            bail!("Texture {} is not square ({}x{})", path.display(), w, h);
        }

        let target_size = match inferred_size {
            Some(size) => size,
            None => {
                inferred_size = Some(w);
                w
            }
        };

        let final_image = if w != target_size && args.allow_mixed_sizes {
            println!(
                "Resizing {} from {}x{} to {}x{}",
                path.display(),
                w,
                h,
                target_size,
                target_size
            );
            image::imageops::resize(&image, target_size, target_size, FilterType::Nearest)
        } else if w != target_size {
            bail!(
                "Texture {} has size {} but expected {} (use --allow-mixed-sizes to resize)",
                path.display(),
                w,
                target_size
            );
        } else {
            image
        };

        let name = texture_name(path, &args.input);
        if textures.contains_key(&name) {
            bail!("Duplicate texture name detected: {name}");
        }
        textures.insert(name, final_image);
    }

    if textures.is_empty() {
        bail!("No texture assets found under {}", args.input.display());
    }

    let tile_size = inferred_size.expect("at least one texture provides size");
    let textures = textures
        .into_iter()
        .map(|(name, image)| Texture { name, image })
        .collect();
    Ok((textures, tile_size))
}

fn is_texture_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase()),
        Some(ext) if matches!(ext.as_str(), "png" | "jpg" | "jpeg")
    )
}

fn texture_name(path: &Path, root: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let mut rel_no_ext = rel.to_path_buf();
    rel_no_ext.set_extension("");
    let mut name = rel_no_ext.to_string_lossy().replace('\\', "/");
    if name.starts_with("./") {
        name = name.replacen("./", "", 1);
    }
    if name.starts_with('/') {
        name.remove(0);
    }
    name
}
