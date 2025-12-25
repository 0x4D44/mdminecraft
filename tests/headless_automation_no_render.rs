use serde_json::json;
use std::io::{BufRead, BufReader, BufWriter, Write};
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

#[test]
fn headless_step_mode_no_render_smoke() {
    let port = pick_free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let screenshot_dir = std::env::temp_dir().join(format!(
        "mdminecraft_headless_shots_{:016x}",
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&screenshot_dir).expect("create screenshot dir");

    let bin = env!("CARGO_BIN_EXE_mdminecraft");
    let mut child = Command::new(bin)
        .args([
            "--headless",
            "--no-render",
            "--no-audio",
            "--no-save",
            "--world-seed",
            "1",
            "--automation-listen",
            &addr.to_string(),
            "--automation-step",
            "--screenshot-dir",
            screenshot_dir.to_str().unwrap(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mdminecraft");

    let stream = connect_with_retry(addr, Duration::from_secs(5));
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
    assert_eq!(hello["version"], 1);

    write_json_line(&mut writer, json!({"op":"get_state","id":2}));
    let state = read_json_line(&mut reader);
    assert_eq!(state["event"], "state");
    let tick0 = state["tick"].as_u64().expect("tick");

    write_json_line(
        &mut writer,
        json!({"op":"command","id":3,"line":"/tp 0 80 0"}),
    );
    let cmd = read_json_line(&mut reader);
    assert_eq!(cmd["event"], "command_result");
    assert_eq!(cmd["ok"], true);

    write_json_line(&mut writer, json!({"op":"step","id":4,"ticks":5}));
    let stepped_tick = loop {
        let msg = read_json_line(&mut reader);
        match msg["event"].as_str() {
            Some("stepped") => break msg["tick"].as_u64().expect("stepped tick"),
            Some("screenshot") => continue,
            Some(other) => panic!("unexpected event during step: {other} ({msg})"),
            None => panic!("missing event field during step: {msg}"),
        }
    };
    assert_eq!(stepped_tick, tick0 + 5);

    write_json_line(&mut writer, json!({"op":"screenshot","id":5,"tag":"test"}));
    let screenshot = read_json_line(&mut reader);
    assert_eq!(screenshot["event"], "error");
    assert_eq!(screenshot["code"], "unsupported");

    write_json_line(&mut writer, json!({"op":"shutdown","id":6}));
    let shutdown = read_json_line(&mut reader);
    assert_eq!(shutdown["event"], "ok");

    drop(writer);
    drop(reader);

    let status = wait_for_exit(&mut child, Duration::from_secs(20));
    assert!(status.success(), "mdminecraft exited with {status}");

    let _ = std::fs::remove_dir_all(&screenshot_dir);
}
