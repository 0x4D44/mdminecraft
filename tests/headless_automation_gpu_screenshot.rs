use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn pick_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local addr").port()
}

fn connect_with_retry(addr: SocketAddr, timeout: Duration) -> TcpStream {
    let start = Instant::now();
    loop {
        match TcpStream::connect(addr) {
            Ok(stream) => return stream,
            Err(err) => {
                if start.elapsed() > timeout {
                    panic!("failed to connect to {addr}: {err}");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn write_json_line(writer: &mut BufWriter<TcpStream>, value: serde_json::Value) {
    serde_json::to_writer(&mut *writer, &value).expect("write request");
    writer.write_all(b"\n").expect("write newline");
    writer.flush().expect("flush");
}

fn read_json_line(reader: &mut BufReader<TcpStream>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read response");
    assert!(!line.trim().is_empty(), "empty response line");
    serde_json::from_str(line.trim()).expect("parse response json")
}

fn wait_for_exit(child: &mut std::process::Child, timeout: Duration) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            panic!("process did not exit within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read_png_dimensions(path: &std::path::Path) -> (u32, u32) {
    let mut file = fs::File::open(path).expect("open png");
    let mut header = [0u8; 24];
    file.read_exact(&mut header).expect("read png header");

    let signature = b"\x89PNG\r\n\x1a\n";
    assert_eq!(&header[0..8], signature, "png signature mismatch");
    assert_eq!(&header[12..16], b"IHDR", "png IHDR missing");

    let width = u32::from_be_bytes(header[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(header[20..24].try_into().unwrap());
    (width, height)
}

#[test]
fn headless_screenshot_gpu_smoke() {
    if std::env::var("MDM_RUN_HEADLESS_GPU_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping (set MDM_RUN_HEADLESS_GPU_TESTS=1 to enable)");
        return;
    }

    let port = pick_free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let screenshot_dir = std::env::temp_dir().join(format!(
        "mdminecraft_headless_gpu_shots_{:016x}",
        rand::random::<u64>()
    ));
    fs::create_dir_all(&screenshot_dir).expect("create screenshot dir");

    let bin = env!("CARGO_BIN_EXE_mdminecraft");
    let mut child = Command::new(bin)
        .args([
            "--headless",
            "--no-audio",
            "--no-save",
            "--world-seed",
            "1",
            "--resolution",
            "320x240",
            "--automation-listen",
            &addr.to_string(),
            "--automation-step",
            "--screenshot-dir",
            screenshot_dir.to_str().unwrap(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn mdminecraft");

    let stream = connect_with_retry(addr, Duration::from_secs(30));
    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .expect("set read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .expect("set write timeout");

    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut writer = BufWriter::new(stream);

    write_json_line(&mut writer, json!({"op":"hello","id":1,"version":1}));
    let hello = read_json_line(&mut reader);
    assert_eq!(hello["event"], "hello");

    write_json_line(
        &mut writer,
        json!({"op":"screenshot","id":2,"tag":"gpu_smoke"}),
    );
    let screenshot = read_json_line(&mut reader);
    assert_eq!(screenshot["event"], "screenshot");
    assert_eq!(screenshot["width"], 320);
    assert_eq!(screenshot["height"], 240);
    let screenshot_path = screenshot["path"].as_str().expect("path").to_string();
    assert!(!screenshot_path.is_empty(), "empty screenshot path");

    let path = std::path::PathBuf::from(&screenshot_path);
    assert!(path.exists(), "screenshot missing: {}", path.display());

    let (w, h) = read_png_dimensions(&path);
    assert_eq!((w, h), (320, 240));

    write_json_line(&mut writer, json!({"op":"shutdown","id":3}));
    let shutdown = read_json_line(&mut reader);
    assert_eq!(shutdown["event"], "ok");

    drop(writer);
    drop(reader);

    let status = wait_for_exit(&mut child, Duration::from_secs(60));
    assert!(status.success(), "mdminecraft exited with {status}");

    let _ = fs::remove_dir_all(&screenshot_dir);
}
