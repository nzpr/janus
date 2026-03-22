use codex_protocol::models::ResponseItem;
use codex_secrets::redact_json_secrets;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    None,
    Zstd,
}

pub(crate) fn attach_item_ids(payload_json: &mut Value, original_items: &[ResponseItem]) {
    let Some(input_value) = payload_json.get_mut("input") else {
        return;
    };
    let Value::Array(items) = input_value else {
        return;
    };

    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        if let ResponseItem::Reasoning { id, .. }
        | ResponseItem::Message { id: Some(id), .. }
        | ResponseItem::WebSearchCall { id: Some(id), .. }
        | ResponseItem::FunctionCall { id: Some(id), .. }
        | ResponseItem::ToolSearchCall { id: Some(id), .. }
        | ResponseItem::LocalShellCall { id: Some(id), .. }
        | ResponseItem::CustomToolCall { id: Some(id), .. } = item
        {
            if id.is_empty() {
                continue;
            }

            if let Some(obj) = value.as_object_mut() {
                obj.insert("id".to_string(), Value::String(id.clone()));
            }
        }
    }
}

pub(crate) fn sanitize_request_payload(payload_json: &mut Value) {
    redact_json_secrets(payload_json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_request_payload_redacts_nested_secret_strings() {
        let mut payload = serde_json::json!({
            "instructions": "Use key sk_test_abcdefghijklmnopqrstuvwxyz12 carefully",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Bearer abcdefghijklmnopqrstuvwxyz123456"
                        }
                    ]
                },
                {
                    "type": "function_call",
                    "arguments": "{\"token\":\"ghp_abcdefghijklmnopqrstuvwxyzABCDEF1234\"}"
                }
            ]
        });

        sanitize_request_payload(&mut payload);

        let serialized = payload.to_string();
        assert!(!serialized.contains("sk_test_abcdefghijklmnopqrstuvwxyz12"));
        assert!(!serialized.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!serialized.contains("ghp_abcdefghijklmnopqrstuvwxyzABCDEF1234"));
        assert!(serialized.contains("[REDACTED_SECRET]"));
    }
}
