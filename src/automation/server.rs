use crate::automation::controller::{AutomationEndpoint, AutomationMsg};
use crate::automation::protocol::{self, ErrorCode, Request};
use anyhow::Result;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
#[cfg(unix)]
use std::{
    os::unix::{fs::FileTypeExt, net::UnixListener},
    os::unix::{net::SocketAddr as UnixSocketAddr, net::UnixStream},
};

pub struct AutomationServerHandle {
    pub endpoint: AutomationEndpoint,
    #[allow(dead_code)]
    join: thread::JoinHandle<()>,
}

struct AutomationServerConfig {
    token: Option<String>,
    log: Option<AutomationLog>,
}

#[derive(Clone)]
struct AutomationLog {
    writer: Arc<Mutex<BufWriter<std::fs::File>>>,
}

impl AutomationLog {
    fn open(path: &PathBuf) -> Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            writer: Arc::new(Mutex::new(BufWriter::new(file))),
        })
    }

    fn write_json(&self, value: &serde_json::Value) {
        if let Ok(mut guard) = self.writer.lock() {
            if serde_json::to_writer(&mut *guard, value).is_ok() {
                let _ = guard.write_all(b"\n");
                let _ = guard.flush();
            }
        }
    }
}

pub struct AutomationServer;

impl AutomationServer {
    pub fn start(
        addr: SocketAddr,
        token: Option<String>,
        log_path: Option<PathBuf>,
    ) -> Result<AutomationServerHandle> {
        let (to_game_tx, to_game_rx) = mpsc::sync_channel::<AutomationMsg>(256);

        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(false)?;

        let log = match log_path {
            Some(path) => match AutomationLog::open(&path) {
                Ok(log) => Some(log),
                Err(err) => {
                    tracing::warn!(%err, path = %path.display(), "Failed to open automation log");
                    None
                }
            },
            None => None,
        };

        let cfg = AutomationServerConfig { token, log };
        let controller_active = Arc::new(AtomicBool::new(false));

        let join = thread::spawn(move || {
            tracing::info!(addr = %addr, "Automation server listening");
            loop {
                let (stream, peer) = match listener.accept() {
                    Ok(conn) => conn,
                    Err(err) => {
                        tracing::warn!(%err, "Automation server accept failed");
                        continue;
                    }
                };

                let cfg = AutomationServerConfig {
                    token: cfg.token.clone(),
                    log: cfg.log.clone(),
                };
                let controller_active = Arc::clone(&controller_active);
                let to_game_tx = to_game_tx.clone();
                thread::spawn(move || {
                    handle_connection(stream, peer.to_string(), cfg, controller_active, to_game_tx);
                });
            }
        });

        Ok(AutomationServerHandle {
            endpoint: AutomationEndpoint { rx: to_game_rx },
            join,
        })
    }

    #[cfg(unix)]
    pub fn start_uds(
        path: PathBuf,
        token: Option<String>,
        log_path: Option<PathBuf>,
    ) -> Result<AutomationServerHandle> {
        let (to_game_tx, to_game_rx) = mpsc::sync_channel::<AutomationMsg>(256);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if path.exists() {
            let meta = std::fs::metadata(&path)?;
            if meta.file_type().is_socket() {
                std::fs::remove_file(&path)?;
            } else {
                anyhow::bail!(
                    "--automation-uds path exists and is not a socket: {}",
                    path.display()
                );
            }
        }

        let listener = UnixListener::bind(&path)?;

        let log = match log_path {
            Some(path) => match AutomationLog::open(&path) {
                Ok(log) => Some(log),
                Err(err) => {
                    tracing::warn!(%err, path = %path.display(), "Failed to open automation log");
                    None
                }
            },
            None => None,
        };

        let cfg = AutomationServerConfig { token, log };
        let controller_active = Arc::new(AtomicBool::new(false));

        let join = thread::spawn(move || {
            tracing::info!(path = %path.display(), "Automation server listening (uds)");
            loop {
                let (stream, peer) = match listener.accept() {
                    Ok(conn) => conn,
                    Err(err) => {
                        tracing::warn!(%err, "Automation server accept failed");
                        continue;
                    }
                };

                let peer_label = peer_label_uds(&peer);
                let cfg = AutomationServerConfig {
                    token: cfg.token.clone(),
                    log: cfg.log.clone(),
                };
                let controller_active = Arc::clone(&controller_active);
                let to_game_tx = to_game_tx.clone();
                thread::spawn(move || {
                    handle_connection(stream, peer_label, cfg, controller_active, to_game_tx);
                });
            }
        });

        Ok(AutomationServerHandle {
            endpoint: AutomationEndpoint { rx: to_game_rx },
            join,
        })
    }
}

trait AutomationStream: Read + Write + Send + 'static {
    fn try_clone(&self) -> std::io::Result<Self>
    where
        Self: Sized;
    fn shutdown(&self, how: Shutdown) -> std::io::Result<()>;
}

impl AutomationStream for TcpStream {
    fn try_clone(&self) -> std::io::Result<Self> {
        TcpStream::try_clone(self)
    }

    fn shutdown(&self, how: Shutdown) -> std::io::Result<()> {
        TcpStream::shutdown(self, how)
    }
}

#[cfg(unix)]
impl AutomationStream for UnixStream {
    fn try_clone(&self) -> std::io::Result<Self> {
        UnixStream::try_clone(self)
    }

    fn shutdown(&self, how: Shutdown) -> std::io::Result<()> {
        UnixStream::shutdown(self, how)
    }
}

#[cfg(unix)]
fn peer_label_uds(peer: &UnixSocketAddr) -> String {
    peer.as_pathname()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| format!("{peer:?}"))
}

fn handle_connection<S: AutomationStream>(
    mut stream: S,
    peer: String,
    cfg: AutomationServerConfig,
    controller_active: Arc<AtomicBool>,
    to_game: SyncSender<AutomationMsg>,
) {
    let claimed = controller_active
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();

    if !claimed {
        let mut writer = BufWriter::new(&mut stream);
        let _ = write_value_logged(
            &mut writer,
            &protocol::event_error(None, ErrorCode::Busy, "controller already connected"),
            cfg.log.as_ref(),
            &peer,
        );
        drop(writer);
        let _ = stream.shutdown(Shutdown::Both);
        return;
    }

    if let Some(log) = &cfg.log {
        log.write_json(&serde_json::json!({"event":"connect","peer":peer.to_string()}));
    }

    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!(%err, "Failed to clone automation stream");
            controller_active.store(false, Ordering::SeqCst);
            return;
        }
    });
    let mut writer = BufWriter::new(stream);

    let mut authed = false;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // disconnect
            Ok(n) => {
                if n > protocol::MAX_LINE_BYTES {
                    if let Some(log) = &cfg.log {
                        log.write_json(&serde_json::json!({
                            "event":"request_error",
                            "peer": peer.as_str(),
                            "code": ErrorCode::BadRequest.as_str(),
                            "message": "line too large",
                            "id": serde_json::Value::Null,
                        }));
                    }
                    let _ = write_value_logged(
                        &mut writer,
                        &protocol::event_error(None, ErrorCode::BadRequest, "line too large"),
                        cfg.log.as_ref(),
                        &peer,
                    );
                    break;
                }
            }
            Err(err) => {
                tracing::warn!(%err, "Automation read failed");
                break;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req = match protocol::decode_request(trimmed) {
            Ok(req) => req,
            Err(err) => {
                let request_id = err.id.clone();
                let message = err.message.clone();
                if let Some(log) = &cfg.log {
                    log.write_json(&serde_json::json!({
                        "event":"request_error",
                        "peer": peer.as_str(),
                        "code": err.code.as_str(),
                        "message": message.as_str(),
                        "id": request_id.clone().map(|id| id.into_value()),
                    }));
                }
                let _ = write_value_logged(
                    &mut writer,
                    &protocol::event_error(request_id, err.code, message),
                    cfg.log.as_ref(),
                    &peer,
                );
                continue;
            }
        };

        if let Some(log) = &cfg.log {
            log.write_json(&request_log_value(&peer, &req));
        }

        match req {
            Request::Hello(hello) => {
                if authed {
                    let _ = write_value_logged(
                        &mut writer,
                        &protocol::event_error(
                            hello.id,
                            ErrorCode::BadRequest,
                            "hello already completed",
                        ),
                        cfg.log.as_ref(),
                        &peer,
                    );
                    continue;
                }

                if hello.version != protocol::PROTOCOL_VERSION {
                    let _ = write_value_logged(
                        &mut writer,
                        &protocol::event_error(
                            hello.id,
                            ErrorCode::Unsupported,
                            format!(
                                "unsupported protocol version {}, expected {}",
                                hello.version,
                                protocol::PROTOCOL_VERSION
                            ),
                        ),
                        cfg.log.as_ref(),
                        &peer,
                    );
                    break;
                }

                if let Some(expected) = &cfg.token {
                    if hello.token.as_deref() != Some(expected.as_str()) {
                        let _ = write_value_logged(
                            &mut writer,
                            &protocol::event_error(
                                hello.id,
                                ErrorCode::Unauthorized,
                                "invalid token",
                            ),
                            cfg.log.as_ref(),
                            &peer,
                        );
                        break;
                    }
                }

                authed = true;
                let capabilities = [
                    "hello",
                    "set_actions",
                    "pulse",
                    "set_view",
                    "command",
                    "get_state",
                    "step",
                    "screenshot",
                    "shutdown",
                ];
                let event = protocol::event_hello(hello.id, &capabilities);
                let _ = write_value_logged(&mut writer, &event, cfg.log.as_ref(), &peer);

                let _ = to_game.send(AutomationMsg::Connected);
            }
            Request::Unknown { id, op } => {
                if !authed {
                    let _ = write_value_logged(
                        &mut writer,
                        &protocol::event_error(id, ErrorCode::Unauthorized, "hello required"),
                        cfg.log.as_ref(),
                        &peer,
                    );
                    break;
                }
                let _ = write_value_logged(
                    &mut writer,
                    &protocol::event_error(
                        id,
                        ErrorCode::Unsupported,
                        format!("unknown op `{op}`"),
                    ),
                    cfg.log.as_ref(),
                    &peer,
                );
            }
            other => {
                if !authed {
                    let _ = write_value_logged(
                        &mut writer,
                        &protocol::event_error(
                            other.request_id(),
                            ErrorCode::Unauthorized,
                            "hello required",
                        ),
                        cfg.log.as_ref(),
                        &peer,
                    );
                    break;
                }

                let (resp_tx, resp_rx) = mpsc::sync_channel(1);
                let completion_event = completion_event_for_request(&other);
                let timeout = timeout_for_request(&other);
                let request_id = other.request_id();
                match to_game.try_send(AutomationMsg::Request {
                    request: other,
                    respond_to: resp_tx,
                }) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {
                        let _ = write_value_logged(
                            &mut writer,
                            &protocol::event_error(request_id, ErrorCode::Busy, "server busy"),
                            cfg.log.as_ref(),
                            &peer,
                        );
                        continue;
                    }
                    Err(TrySendError::Disconnected(_)) => break,
                }

                loop {
                    match resp_rx.recv_timeout(timeout) {
                        Ok(value) => {
                            let done = is_completion_event(completion_event, &value);
                            let _ =
                                write_value_logged(&mut writer, &value, cfg.log.as_ref(), &peer);
                            if done {
                                break;
                            }
                        }
                        Err(_) => {
                            let _ = write_value_logged(
                                &mut writer,
                                &protocol::event_error(
                                    None,
                                    ErrorCode::Internal,
                                    "timeout waiting for response",
                                ),
                                cfg.log.as_ref(),
                                &peer,
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    if let Some(log) = &cfg.log {
        log.write_json(&serde_json::json!({"event":"disconnect","peer":peer.to_string()}));
    }

    if authed {
        let _ = to_game.send(AutomationMsg::Disconnected);
    }

    controller_active.store(false, Ordering::SeqCst);
}

fn completion_event_for_request(request: &Request) -> &'static str {
    match request {
        Request::Hello(_) => "hello",
        Request::SetActions(_) | Request::Pulse(_) | Request::SetView(_) | Request::Shutdown(_) => {
            "ok"
        }
        Request::Command(_) => "command_result",
        Request::GetState(_) => "state",
        Request::Step(_) => "stepped",
        Request::Screenshot(_) => "screenshot",
        Request::Unknown { .. } => "error",
    }
}

fn timeout_for_request(request: &Request) -> Duration {
    match request {
        // These operations can legitimately take a while on slower machines (eg. software GPU
        // readback for screenshots, or large command-driven edits).
        Request::Step(_) => Duration::from_secs(120),
        Request::Screenshot(_) => Duration::from_secs(120),
        Request::Command(_) => Duration::from_secs(60),
        _ => Duration::from_secs(30),
    }
}

fn is_completion_event(expected_event: &str, value: &serde_json::Value) -> bool {
    let event = value
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if event == "error" {
        return true;
    }
    event == expected_event
}

fn write_value_logged<W: Write>(
    writer: &mut W,
    value: &serde_json::Value,
    log: Option<&AutomationLog>,
    peer: &str,
) -> Result<()> {
    if let Some(log) = log {
        log.write_json(&serde_json::json!({
            "event":"response",
            "peer": peer,
            "payload": value,
        }));
    }
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn request_log_value(peer: &str, req: &Request) -> serde_json::Value {
    match req {
        Request::Hello(hello) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"hello",
            "id": hello.id.clone().map(|id| id.into_value()),
            "version": hello.version,
            "token_present": hello.token.is_some(),
        }),
        Request::SetActions(req) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"set_actions",
            "id": req.id.clone().map(|id| id.into_value()),
            "actions": {
                "move_x": req.actions.move_x,
                "move_y": req.actions.move_y,
                "move_z": req.actions.move_z,
                "sprint": req.actions.sprint,
                "crouch": req.actions.crouch,
                "jump_hold": req.actions.jump_hold,
                "attack_hold": req.actions.attack_hold,
                "use_hold": req.actions.use_hold,
                "hotbar_slot": req.actions.hotbar_slot,
            }
        }),
        Request::Pulse(req) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"pulse",
            "id": req.id.clone().map(|id| id.into_value()),
            "actions": {
                "jump_click": req.actions.jump_click,
                "attack_click": req.actions.attack_click,
                "use_click": req.actions.use_click,
                "hotbar_slot": req.actions.hotbar_slot,
            }
        }),
        Request::SetView(req) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"set_view",
            "id": req.id.clone().map(|id| id.into_value()),
            "yaw": req.yaw,
            "pitch": req.pitch,
        }),
        Request::Command(req) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"command",
            "id": req.id.clone().map(|id| id.into_value()),
            "line": req.line,
        }),
        Request::GetState(req) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"get_state",
            "id": req.id.clone().map(|id| id.into_value()),
        }),
        Request::Step(req) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"step",
            "id": req.id.clone().map(|id| id.into_value()),
            "ticks": req.ticks,
        }),
        Request::Screenshot(req) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"screenshot",
            "id": req.id.clone().map(|id| id.into_value()),
            "tag": req.tag,
        }),
        Request::Shutdown(req) => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op":"shutdown",
            "id": req.id.clone().map(|id| id.into_value()),
        }),
        Request::Unknown { id, op } => serde_json::json!({
            "event":"request",
            "peer": peer,
            "op": op,
            "id": id.clone().map(|id| id.into_value()),
        }),
    }
}
