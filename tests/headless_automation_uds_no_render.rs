#![cfg(unix)]

use serde_json::json;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn pick_free_uds_path() -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "mdminecraft_automation_{:016x}.sock",
        rand::random::<u64>()
    ));
    if base.exists() {
        let _ = std::fs::remove_file(&base);
    }
    base
}

fn connect_with_retry(path: &Path, timeout: Duration) -> UnixStream {
    let start = Instant::now();
    loop {
        match UnixStream::connect(path) {
            Ok(stream) => return stream,
            Err(err) => {
                if start.elapsed() > timeout {
                    panic!("failed to connect to {}: {err}", path.display());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn write_json_line(writer: &mut BufWriter<UnixStream>, value: serde_json::Value) {
    serde_json::to_writer(&mut *writer, &value).expect("write request");
    writer.write_all(b"\n").expect("write newline");
    writer.flush().expect("flush");
}

fn read_json_line(reader: &mut BufReader<UnixStream>) -> serde_json::Value {
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

#[test]
fn headless_step_mode_uds_no_render_smoke() {
    let sock_path = pick_free_uds_path();

    // Reserve the pathname briefly so the test fails fast if the path is unusable.
    let _probe = UnixListener::bind(&sock_path).expect("bind probe unix socket");
    drop(_probe);
    let _ = std::fs::remove_file(&sock_path);

    let bin = env!("CARGO_BIN_EXE_mdminecraft");
    let mut child = Command::new(bin)
        .args([
            "--headless",
            "--no-render",
            "--no-audio",
            "--no-save",
            "--world-seed",
            "1",
            "--automation-uds",
            sock_path.to_str().unwrap(),
            "--automation-step",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mdminecraft");

    let stream = connect_with_retry(&sock_path, Duration::from_secs(10));
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("set read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .expect("set write timeout");

    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut writer = BufWriter::new(stream);

    write_json_line(&mut writer, json!({"op":"hello","id":1,"version":1}));
    let hello = read_json_line(&mut reader);
    assert_eq!(hello["event"], "hello");

    write_json_line(&mut writer, json!({"op":"get_state","id":2}));
    let state = read_json_line(&mut reader);
    assert_eq!(state["event"], "state");
    let tick0 = state["tick"].as_u64().expect("tick");

    write_json_line(&mut writer, json!({"op":"step","id":3,"ticks":3}));
    let stepped = read_json_line(&mut reader);
    assert_eq!(stepped["event"], "stepped");
    assert_eq!(stepped["tick"].as_u64().expect("tick"), tick0 + 3);

    write_json_line(&mut writer, json!({"op":"screenshot","id":4,"tag":"test"}));
    let screenshot = read_json_line(&mut reader);
    assert_eq!(screenshot["event"], "error");
    assert_eq!(screenshot["code"], "unsupported");

    write_json_line(&mut writer, json!({"op":"shutdown","id":5}));
    let shutdown = read_json_line(&mut reader);
    assert_eq!(shutdown["event"], "ok");

    drop(writer);
    drop(reader);

    let status = wait_for_exit(&mut child, Duration::from_secs(20));
    assert!(status.success(), "mdminecraft exited with {status}");

    let _ = std::fs::remove_file(&sock_path);
}
