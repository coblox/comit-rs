use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Frame {
    #[serde(rename = "type")]
    pub frame_type: FrameType,
    pub payload: JsonValue,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum FrameType {
    Request,
    Response,

    // Too lazy ATM to write a proper deserializer that preserves the value 🙃
    #[serde(other)]
    Other,
}

impl Frame {
    pub fn new(frame_type: FrameType, payload: JsonValue) -> Self {
        Self {
            frame_type,
            payload,
        }
    }
}
