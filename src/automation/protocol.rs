use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct RequestId(Value);

impl RequestId {
    pub fn new(value: Value) -> Option<Self> {
        match value {
            Value::String(_) | Value::Number(_) => Some(Self(value)),
            _ => None,
        }
    }

    pub fn into_value(self) -> Value {
        self.0
    }
}

#[derive(Debug)]
pub enum Request {
    Hello(HelloRequest),
    SetActions(SetActionsRequest),
    Pulse(PulseRequest),
    SetView(SetViewRequest),
    Command(CommandRequest),
    GetState(GetStateRequest),
    Step(StepRequest),
    Screenshot(ScreenshotRequest),
    Shutdown(ShutdownRequest),
    Unknown { id: Option<RequestId>, op: String },
}

impl Request {
    pub fn request_id(&self) -> Option<RequestId> {
        match self {
            Request::Hello(req) => req.id.clone(),
            Request::SetActions(req) => req.id.clone(),
            Request::Pulse(req) => req.id.clone(),
            Request::SetView(req) => req.id.clone(),
            Request::Command(req) => req.id.clone(),
            Request::GetState(req) => req.id.clone(),
            Request::Step(req) => req.id.clone(),
            Request::Screenshot(req) => req.id.clone(),
            Request::Shutdown(req) => req.id.clone(),
            Request::Unknown { id, .. } => id.clone(),
        }
    }
}

#[derive(Debug)]
pub struct HelloRequest {
    pub id: Option<RequestId>,
    pub version: u32,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ActionsPatch {
    pub move_x: Option<f32>,
    pub move_y: Option<f32>,
    pub move_z: Option<f32>,
    pub sprint: Option<bool>,
    pub crouch: Option<bool>,
    pub jump_hold: Option<bool>,
    pub attack_hold: Option<bool>,
    pub use_hold: Option<bool>,
    pub hotbar_slot: Option<u8>,
}

#[derive(Debug)]
pub struct SetActionsRequest {
    pub id: Option<RequestId>,
    pub actions: ActionsPatch,
}

#[derive(Debug, Clone, Default)]
pub struct PulsePatch {
    pub jump_click: bool,
    pub attack_click: bool,
    pub use_click: bool,
    pub hotbar_slot: Option<u8>,
}

#[derive(Debug)]
pub struct PulseRequest {
    pub id: Option<RequestId>,
    pub actions: PulsePatch,
}

#[derive(Debug)]
pub struct SetViewRequest {
    pub id: Option<RequestId>,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug)]
pub struct CommandRequest {
    pub id: Option<RequestId>,
    pub line: String,
}

#[derive(Debug)]
pub struct GetStateRequest {
    pub id: Option<RequestId>,
}

#[derive(Debug)]
pub struct StepRequest {
    pub id: Option<RequestId>,
    pub ticks: u64,
}

#[derive(Debug)]
pub struct ScreenshotRequest {
    pub id: Option<RequestId>,
    pub tag: Option<String>,
}

#[derive(Debug)]
pub struct ShutdownRequest {
    pub id: Option<RequestId>,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    BadRequest,
    Unauthorized,
    Unsupported,
    Busy,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::BadRequest => "bad_request",
            ErrorCode::Unauthorized => "unauthorized",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::Busy => "busy",
            ErrorCode::Internal => "internal",
        }
    }
}

#[derive(Debug)]
pub struct ProtocolError {
    pub id: Option<RequestId>,
    pub code: ErrorCode,
    pub message: String,
}

pub fn decode_request(line: &str) -> Result<Request, ProtocolError> {
    let value: Value = serde_json::from_str(line).map_err(|err| ProtocolError {
        id: None,
        code: ErrorCode::BadRequest,
        message: format!("invalid JSON: {err}"),
    })?;

    let obj = value.as_object().ok_or_else(|| ProtocolError {
        id: None,
        code: ErrorCode::BadRequest,
        message: "request must be a JSON object".to_string(),
    })?;

    let id = obj.get("id").cloned().and_then(RequestId::new);

    let op = obj
        .get("op")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProtocolError {
            id: id.clone(),
            code: ErrorCode::BadRequest,
            message: "missing or invalid string field `op`".to_string(),
        })?;

    match op {
        "hello" => {
            let version =
                obj.get("version")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ProtocolError {
                        id: id.clone(),
                        code: ErrorCode::BadRequest,
                        message: "missing or invalid numeric field `version`".to_string(),
                    })? as u32;
            let token = obj
                .get("token")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            Ok(Request::Hello(HelloRequest { id, version, token }))
        }
        "set_actions" => {
            let actions = obj
                .get("actions")
                .and_then(|v| v.as_object())
                .ok_or_else(|| ProtocolError {
                    id: id.clone(),
                    code: ErrorCode::BadRequest,
                    message: "missing or invalid object field `actions`".to_string(),
                })?;
            let actions = parse_actions_patch(actions).map_err(|message| ProtocolError {
                id: id.clone(),
                code: ErrorCode::BadRequest,
                message,
            })?;
            Ok(Request::SetActions(SetActionsRequest { id, actions }))
        }
        "pulse" => {
            let actions = obj
                .get("actions")
                .and_then(|v| v.as_object())
                .ok_or_else(|| ProtocolError {
                    id: id.clone(),
                    code: ErrorCode::BadRequest,
                    message: "missing or invalid object field `actions`".to_string(),
                })?;
            let actions = parse_pulse_patch(actions).map_err(|message| ProtocolError {
                id: id.clone(),
                code: ErrorCode::BadRequest,
                message,
            })?;
            Ok(Request::Pulse(PulseRequest { id, actions }))
        }
        "set_view" => {
            let yaw = obj
                .get("yaw")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| ProtocolError {
                    id: id.clone(),
                    code: ErrorCode::BadRequest,
                    message: "missing or invalid numeric field `yaw`".to_string(),
                })? as f32;
            let pitch = obj
                .get("pitch")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| ProtocolError {
                    id: id.clone(),
                    code: ErrorCode::BadRequest,
                    message: "missing or invalid numeric field `pitch`".to_string(),
                })? as f32;
            Ok(Request::SetView(SetViewRequest { id, yaw, pitch }))
        }
        "command" => {
            let line = obj
                .get("line")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ProtocolError {
                    id: id.clone(),
                    code: ErrorCode::BadRequest,
                    message: "missing or invalid string field `line`".to_string(),
                })?
                .to_string();
            Ok(Request::Command(CommandRequest { id, line }))
        }
        "get_state" => Ok(Request::GetState(GetStateRequest { id })),
        "step" => {
            let ticks = obj
                .get("ticks")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ProtocolError {
                    id: id.clone(),
                    code: ErrorCode::BadRequest,
                    message: "missing or invalid numeric field `ticks`".to_string(),
                })?;
            Ok(Request::Step(StepRequest { id, ticks }))
        }
        "screenshot" => {
            let tag = obj.get("tag").and_then(|v| v.as_str()).map(str::to_string);
            Ok(Request::Screenshot(ScreenshotRequest { id, tag }))
        }
        "shutdown" => Ok(Request::Shutdown(ShutdownRequest { id })),
        other => Ok(Request::Unknown {
            id,
            op: other.to_string(),
        }),
    }
}

fn parse_actions_patch(obj: &serde_json::Map<String, Value>) -> Result<ActionsPatch, String> {
    if obj.contains_key("jump_click")
        || obj.contains_key("attack_click")
        || obj.contains_key("use_click")
    {
        return Err("click fields are not allowed in set_actions (use `pulse`)".to_string());
    }

    Ok(ActionsPatch {
        move_x: parse_optional_f32(obj, "move_x")?,
        move_y: parse_optional_f32(obj, "move_y")?,
        move_z: parse_optional_f32(obj, "move_z")?,
        sprint: parse_optional_bool(obj, "sprint")?,
        crouch: parse_optional_bool(obj, "crouch")?,
        jump_hold: parse_optional_bool(obj, "jump_hold")?,
        attack_hold: parse_optional_bool(obj, "attack_hold")?,
        use_hold: parse_optional_bool(obj, "use_hold")?,
        hotbar_slot: parse_optional_u8(obj, "hotbar_slot")?,
    })
}

fn parse_pulse_patch(obj: &serde_json::Map<String, Value>) -> Result<PulsePatch, String> {
    Ok(PulsePatch {
        jump_click: parse_optional_bool(obj, "jump_click")?.unwrap_or(false),
        attack_click: parse_optional_bool(obj, "attack_click")?.unwrap_or(false),
        use_click: parse_optional_bool(obj, "use_click")?.unwrap_or(false),
        hotbar_slot: parse_optional_u8(obj, "hotbar_slot")?,
    })
}

fn parse_optional_f32(
    obj: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<f32>, String> {
    match obj.get(key) {
        None => Ok(None),
        Some(v) => v
            .as_f64()
            .map(|f| f as f32)
            .ok_or_else(|| format!("invalid numeric field `{key}`"))
            .map(Some),
    }
}

fn parse_optional_bool(
    obj: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<bool>, String> {
    match obj.get(key) {
        None => Ok(None),
        Some(v) => v
            .as_bool()
            .ok_or_else(|| format!("invalid boolean field `{key}`"))
            .map(Some),
    }
}

fn parse_optional_u8(
    obj: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<u8>, String> {
    match obj.get(key) {
        None => Ok(None),
        Some(v) => {
            let raw = v
                .as_u64()
                .ok_or_else(|| format!("invalid numeric field `{key}`"))?;
            let value: u8 = raw
                .try_into()
                .map_err(|_| format!("field `{key}` out of range"))?;
            Ok(Some(value))
        }
    }
}

pub fn event_hello(id: Option<RequestId>, capabilities: &[&str]) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("event".to_string(), Value::String("hello".to_string()));
    obj.insert(
        "version".to_string(),
        Value::Number((PROTOCOL_VERSION as u64).into()),
    );
    obj.insert(
        "capabilities".to_string(),
        Value::Array(
            capabilities
                .iter()
                .map(|cap| Value::String((*cap).to_string()))
                .collect(),
        ),
    );
    if let Some(id) = id {
        obj.insert("id".to_string(), id.into_value());
    }
    Value::Object(obj)
}

pub fn event_ok(id: Option<RequestId>) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("event".to_string(), Value::String("ok".to_string()));
    if let Some(id) = id {
        obj.insert("id".to_string(), id.into_value());
    }
    Value::Object(obj)
}

pub fn event_error(id: Option<RequestId>, code: ErrorCode, message: impl Into<String>) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("event".to_string(), Value::String("error".to_string()));
    obj.insert("code".to_string(), Value::String(code.as_str().to_string()));
    obj.insert("message".to_string(), Value::String(message.into()));
    if let Some(id) = id {
        obj.insert("id".to_string(), id.into_value());
    }
    Value::Object(obj)
}

pub fn event_command_result(
    id: Option<RequestId>,
    tick: u64,
    ok: bool,
    lines: Vec<String>,
) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "event".to_string(),
        Value::String("command_result".to_string()),
    );
    obj.insert("tick".to_string(), Value::Number(tick.into()));
    obj.insert("ok".to_string(), Value::Bool(ok));
    obj.insert(
        "lines".to_string(),
        Value::Array(lines.into_iter().map(Value::String).collect()),
    );
    if let Some(id) = id {
        obj.insert("id".to_string(), id.into_value());
    }
    Value::Object(obj)
}

#[allow(clippy::too_many_arguments)]
pub fn event_state(
    id: Option<RequestId>,
    tick: u64,
    dimension: &str,
    pos: [f32; 3],
    yaw: f32,
    pitch: f32,
    health: f32,
    hunger: f32,
) -> Value {
    fn float(value: f64) -> Value {
        if !value.is_finite() {
            return Value::Number(0_u64.into());
        }
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::Number(0_u64.into()))
    }

    let mut player = serde_json::Map::new();
    player.insert(
        "pos".to_string(),
        Value::Array(pos.into_iter().map(|v| float(v as f64)).collect()),
    );
    player.insert("yaw".to_string(), float(yaw as f64));
    player.insert("pitch".to_string(), float(pitch as f64));
    player.insert("health".to_string(), float(health as f64));
    player.insert("hunger".to_string(), float(hunger as f64));

    let mut obj = serde_json::Map::new();
    obj.insert("event".to_string(), Value::String("state".to_string()));
    obj.insert("tick".to_string(), Value::Number(tick.into()));
    obj.insert(
        "dimension".to_string(),
        Value::String(dimension.to_string()),
    );
    obj.insert("player".to_string(), Value::Object(player));
    if let Some(id) = id {
        obj.insert("id".to_string(), id.into_value());
    }
    Value::Object(obj)
}

pub fn event_screenshot(
    id: Option<RequestId>,
    tick: u64,
    path: String,
    width: u32,
    height: u32,
) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("event".to_string(), Value::String("screenshot".to_string()));
    obj.insert("tick".to_string(), Value::Number(tick.into()));
    obj.insert("path".to_string(), Value::String(path));
    obj.insert("width".to_string(), Value::Number((width as u64).into()));
    obj.insert("height".to_string(), Value::Number((height as u64).into()));
    if let Some(id) = id {
        obj.insert("id".to_string(), id.into_value());
    }
    Value::Object(obj)
}

pub fn event_stepped(id: Option<RequestId>, tick: u64) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("event".to_string(), Value::String("stepped".to_string()));
    obj.insert("tick".to_string(), Value::Number(tick.into()));
    if let Some(id) = id {
        obj.insert("id".to_string(), id.into_value());
    }
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hello_parses_with_id() {
        let line = r#"{"op":"hello","id":1,"version":1}"#;
        let req = decode_request(line).expect("should parse");
        let Request::Hello(hello) = req else {
            panic!("expected hello");
        };
        assert_eq!(hello.version, 1);
        assert!(hello.id.is_some());
    }

    #[test]
    fn rejects_non_object() {
        let err = decode_request("[]").unwrap_err();
        assert_eq!(err.code.as_str(), "bad_request");
    }

    #[test]
    fn set_actions_rejects_click_fields() {
        let line = r#"{"op":"set_actions","actions":{"jump_click":true}}"#;
        let err = decode_request(line).unwrap_err();
        assert_eq!(err.code.as_str(), "bad_request");
        assert!(err.message.contains("pulse"));
    }

    #[test]
    fn decode_missing_op_is_bad_request() {
        let err = decode_request(r#"{"id":1}"#).unwrap_err();
        assert_eq!(err.code.as_str(), "bad_request");
        assert_eq!(err.id.unwrap().into_value(), json!(1));
    }

    #[test]
    fn hello_parses_token() {
        let req = decode_request(r#"{"op":"hello","id":"a","version":1,"token":"t"}"#).unwrap();
        match req {
            Request::Hello(hello) => {
                assert_eq!(hello.token.as_deref(), Some("t"));
                assert_eq!(hello.id.unwrap().into_value(), json!("a"));
            }
            other => panic!("unexpected request: {other:?}"),
        }
    }

    #[test]
    fn pulse_defaults_clicks_false() {
        let req = decode_request(r#"{"op":"pulse","id":1,"actions":{}}"#).unwrap();
        match req {
            Request::Pulse(pulse) => {
                assert!(!pulse.actions.jump_click);
                assert!(!pulse.actions.attack_click);
                assert!(!pulse.actions.use_click);
            }
            other => panic!("unexpected request: {other:?}"),
        }
    }

    #[test]
    fn request_id_extraction_works_for_all_variants() {
        let req = decode_request(r#"{"op":"shutdown","id":"x"}"#).unwrap();
        assert_eq!(req.request_id().unwrap().into_value(), json!("x"));
    }
}
