use serde::{Deserialize, Serialize};

pub const JSON_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonError {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonEnvelope {
    pub schema_version: u32,
    pub command: String,
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<JsonError>,
}

impl JsonEnvelope {
    pub fn success(command: &str, data: serde_json::Value) -> Self {
        Self {
            schema_version: JSON_SCHEMA_VERSION,
            command: command.to_string(),
            success: true,
            data,
            error: None,
        }
    }

    pub fn failure(command: &str, message: impl Into<String>) -> Self {
        Self {
            schema_version: JSON_SCHEMA_VERSION,
            command: command.to_string(),
            success: false,
            data: serde_json::Value::Null,
            error: Some(JsonError {
                message: message.into(),
            }),
        }
    }

    pub fn to_pretty_string(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|err| err.to_string())
    }

    pub fn print(&self) -> Result<(), String> {
        println!("{}", self.to_pretty_string()?);
        Ok(())
    }
}
