use crate::automation::protocol::Request;
use serde_json::Value;
use std::sync::mpsc::{Receiver, SyncSender};

pub enum AutomationMsg {
    Connected,
    Disconnected,
    Request {
        request: Request,
        respond_to: SyncSender<Value>,
    },
}

pub struct AutomationEndpoint {
    pub rx: Receiver<AutomationMsg>,
}
