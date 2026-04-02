use serde_json::{Value, json};

/// Maintained schema version tag for machine-readable CLI envelopes.
const CLI_JSON_SCHEMA_VERSION: &str = "jcim-cli.v2";

/// Build the stable JSON error envelope payload without rendering it to a string yet.
pub(super) fn json_error_value(message: &str) -> Value {
    json!({
        "schema_version": CLI_JSON_SCHEMA_VERSION,
        "kind": "error",
        "message": message,
    })
}

/// Wrap one JSON payload in the maintained CLI schema/version envelope.
pub(super) fn json_envelope(kind: &str, payload: Value) -> Value {
    let mut envelope = match payload {
        Value::Object(object) => object,
        other => {
            let mut object = serde_json::Map::new();
            object.insert("value".to_string(), other);
            object
        }
    };
    envelope.insert(
        "schema_version".to_string(),
        Value::String(CLI_JSON_SCHEMA_VERSION.to_string()),
    );
    envelope.insert("kind".to_string(), Value::String(kind.to_string()));
    Value::Object(envelope)
}

/// Render one CLI error message as the stable JSON error envelope.
pub(super) fn json_error(message: &str) -> String {
    json_error_value(message).to_string()
}

/// Render one JSON envelope to stdout for machine-readable CLI output.
pub(super) fn print_json_value(kind: &str, payload: Value) {
    println!("{}", json_envelope(kind, payload));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_envelope_keeps_schema_and_kind_stable_for_object_payloads() {
        let envelope = json_envelope(
            "simulation.summary",
            json!({
                "schema_version": "override-me",
                "kind": "override-me",
                "value": 7,
            }),
        );

        assert_eq!(envelope["schema_version"], CLI_JSON_SCHEMA_VERSION);
        assert_eq!(envelope["kind"], "simulation.summary");
        assert_eq!(envelope["value"], 7);
    }

    #[test]
    fn json_envelope_wraps_scalars_and_errors_use_the_standard_shape() {
        let wrapped = json_envelope("count", json!(7));
        assert_eq!(wrapped["schema_version"], CLI_JSON_SCHEMA_VERSION);
        assert_eq!(wrapped["kind"], "count");
        assert_eq!(wrapped["value"], 7);

        let error = json_error_value("boom");
        assert_eq!(error["schema_version"], CLI_JSON_SCHEMA_VERSION);
        assert_eq!(error["kind"], "error");
        assert_eq!(error["message"], "boom");
    }
}
