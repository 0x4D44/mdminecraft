//! Tests for render pipeline creation and shader management.
//!
//! TDD Phase: RED - These tests are written first and expected to fail.

use super::super::{GpuContext, GpuContextConfig, RenderPipelineBuilder};

/// Helper to get the standard bind group layouts for chunk shaders.
fn chunk_bind_layouts() -> (Vec<wgpu::BindGroupLayoutEntry>, Vec<wgpu::BindGroupLayoutEntry>) {
    let view_proj_layout = vec![wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }];

    let texture_layout = vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ];

    (view_proj_layout, texture_layout)
}

/// Test basic render pipeline creation for chunk rendering.
#[test]
fn create_chunk_render_pipeline_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_vertex_format::<crate::MeshVertex>()
        .with_depth_test(true)
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .build();

    assert!(pipeline.is_ok(), "Chunk render pipeline should be created");
}

/// Test pipeline with invalid shader fails gracefully.
#[test]
fn invalid_shader_code_is_rejected() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let invalid_shader = "this is not valid WGSL code!";

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(invalid_shader)
        .with_fragment_shader(invalid_shader)
        .build();

    assert!(pipeline.is_err(), "Invalid shader should be rejected");
}

/// Test pipeline creation with depth test enabled.
#[test]
fn pipeline_with_depth_test_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_depth_test(true)
        .with_depth_compare(wgpu::CompareFunction::Less)
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .build();

    assert!(pipeline.is_ok(), "Pipeline with depth test should succeed");
}

/// Test pipeline creation with depth test disabled.
#[test]
fn pipeline_without_depth_test_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_depth_test(false)
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .build();

    assert!(
        pipeline.is_ok(),
        "Pipeline without depth test should succeed"
    );
}

/// Test pipeline with different primitive topologies.
#[test]
fn pipeline_with_triangle_list_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_topology(wgpu::PrimitiveTopology::TriangleList)
        .build();

    assert!(pipeline.is_ok(), "Triangle list topology should work");
}

/// Test pipeline with different cull modes.
#[test]
fn pipeline_with_back_face_culling_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_cull_mode(Some(wgpu::Face::Back))
        .build();

    assert!(pipeline.is_ok(), "Back-face culling should work");
}

/// Test pipeline with no culling.
#[test]
fn pipeline_without_culling_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_cull_mode(None)
        .build();

    assert!(pipeline.is_ok(), "No culling should work");
}

/// Test that pipeline validates vertex format compatibility.
#[test]
fn pipeline_validates_vertex_format() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    // Our MeshVertex should be compatible with the chunk shader
    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_vertex_format::<crate::MeshVertex>()
        .build();

    assert!(pipeline.is_ok(), "MeshVertex format should be compatible");
}

/// Test multiple pipelines can coexist.
#[test]
fn multiple_pipelines_can_coexist() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline1 = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_depth_test(true)
        .build();

    let pipeline2 = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_depth_test(false)
        .build();

    assert!(pipeline1.is_ok(), "First pipeline should succeed");
    assert!(pipeline2.is_ok(), "Second pipeline should succeed");
}

/// Test pipeline with blend modes.
#[test]
fn pipeline_with_alpha_blending_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_blend_mode(wgpu::BlendState::ALPHA_BLENDING)
        .build();

    assert!(pipeline.is_ok(), "Alpha blending should work");
}

/// Test pipeline with bind group layouts for uniforms.
#[test]
fn pipeline_with_uniform_bind_group_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_bind_group_layout(
            0,
            &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        )
        .build();

    assert!(pipeline.is_ok(), "Pipeline with bind groups should work");
}

/// Test pipeline can be queried for its properties.
#[test]
fn pipeline_properties_are_queryable() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_label("test_pipeline")
        .build()
        .expect("Pipeline creation");

    let info = pipeline.info();
    assert_eq!(info.label, "test_pipeline");
}

/// Test pipeline with MSAA (multi-sample anti-aliasing).
#[test]
fn pipeline_with_msaa_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_sample_count(4) // 4x MSAA
        .build();

    // May fail on some hardware - that's OK
    if pipeline.is_err() {
        println!("MSAA not supported (expected on some hardware)");
    }
}

/// Test pipeline creation is deterministic.
#[test]
fn pipeline_creation_is_deterministic() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline1 = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_depth_test(true)
        .build()
        .expect("First pipeline");

    let pipeline2 = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_depth_test(true)
        .build()
        .expect("Second pipeline");

    // Both pipelines should have identical configurations
    assert_eq!(pipeline1.info().depth_test, pipeline2.info().depth_test);
    assert_eq!(pipeline1.info().cull_mode, pipeline2.info().cull_mode);
}

/// Test shader preprocessing/validation.
#[test]
fn shader_validation_detects_errors() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    // Shader with syntax error
    let bad_shader = r#"
        @vertex
        fn vs_main() -> @builtin(position) vec4<f32> {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0)  // Missing semicolon
        }
    "#;

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(bad_shader)
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .build();

    assert!(
        pipeline.is_err(),
        "Shader validation should catch syntax errors"
    );
}

/// Test shader with missing entry points is rejected.
#[test]
fn shader_without_entry_points_is_rejected() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    // Valid WGSL but no entry points
    let no_entry_shader = r#"
        struct VertexOutput {
            @builtin(position) position: vec4<f32>,
        };
    "#;

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(no_entry_shader)
        .with_fragment_shader(no_entry_shader)
        .build();

    assert!(pipeline.is_err(), "Shader without entry points should fail");
}

/// Test pipeline can specify custom entry point names.
#[test]
fn pipeline_with_custom_entry_points_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_vertex_entry("vs_main")
        .with_fragment_entry("fs_main")
        .build();

    assert!(pipeline.is_ok(), "Custom entry points should work");
}

/// Test pipeline with different front face winding orders.
#[test]
fn pipeline_with_ccw_front_face_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_front_face(wgpu::FrontFace::Ccw)
        .build();

    assert!(pipeline.is_ok(), "CCW front face should work");
}

/// Test pipeline supports polygon mode configuration.
#[test]
fn pipeline_with_wireframe_mode_succeeds() {
    let config = GpuContextConfig::default_headless();
    let context = GpuContext::new(config).expect("Context creation");
    let (view_proj, texture) = chunk_bind_layouts();

    let pipeline = RenderPipelineBuilder::new(&context)
        .with_vertex_shader(include_str!("../../../shaders/chunk_vertex.wgsl"))
        .with_fragment_shader(include_str!("../../../shaders/chunk_fragment.wgsl"))
        .with_bind_group_layout(0, &view_proj)
        .with_bind_group_layout(1, &texture)
        .with_polygon_mode(wgpu::PolygonMode::Line)
        .build();

    // Wireframe requires a feature - may fail if not supported
    if pipeline.is_err() {
        println!("Wireframe mode not supported (requires POLYGON_MODE_LINE feature)");
    }
}
