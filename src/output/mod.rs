pub mod human;

use serde_json::{Value, json};

/// Standard JSON envelope per spec section 5.3.
pub fn success(command: &str, data: Value) -> Value {
    json!({
        "status": "ok",
        "command": command,
        "data": data,
        "error": null
    })
}

pub fn error(command: &str, code: &str, message: &str) -> Value {
    json!({
        "status": "error",
        "command": command,
        "data": null,
        "error": {
            "code": code,
            "message": message
        }
    })
}
