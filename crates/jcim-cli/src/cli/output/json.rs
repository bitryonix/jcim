use serde_json::{Value, json};

const CLI_JSON_SCHEMA_VERSION: &str = "jcim-cli.v2";

pub(super) fn json_error(message: &str) -> String {
    json!({
        "schema_version": CLI_JSON_SCHEMA_VERSION,
        "kind": "error",
        "message": message,
    })
    .to_string()
}

pub(super) fn print_json_value(kind: &str, payload: Value) {
    let mut envelope = serde_json::Map::new();
    envelope.insert(
        "schema_version".to_string(),
        Value::String(CLI_JSON_SCHEMA_VERSION.to_string()),
    );
    envelope.insert("kind".to_string(), Value::String(kind.to_string()));
    match payload {
        Value::Object(object) => {
            envelope.extend(object);
        }
        other => {
            envelope.insert("value".to_string(), other);
        }
    }
    println!("{}", Value::Object(envelope));
}
