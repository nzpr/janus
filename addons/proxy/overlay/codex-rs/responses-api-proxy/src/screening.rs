use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn sanitize_request_body(body: &[u8], extra_secret_values: &[String]) -> Vec<u8> {
    sanitize_request_body_with_secret_values(body, extra_secret_values)
}

fn sanitize_request_body_with_secret_values(
    body: &[u8],
    extra_secret_values: &[String],
) -> Vec<u8> {
    let explicit_secret_values = normalize_secret_values(extra_secret_values);

    match serde_json::from_slice::<Value>(body) {
        Ok(mut value) => {
            redact_json_secrets(&mut value, &explicit_secret_values);
            serde_json::to_vec(&value).unwrap_or_else(|_| body.to_vec())
        }
        Err(_) => redact_secrets(
            String::from_utf8_lossy(body).into_owned(),
            &explicit_secret_values,
        )
        .into_bytes(),
    }
}

fn redact_json_secrets(value: &mut Value, explicit_secret_values: &[String]) {
    match value {
        Value::String(text) => {
            *text = redact_secrets(std::mem::take(text), explicit_secret_values);
        }
        Value::Array(items) => {
            for item in items {
                redact_json_secrets(item, explicit_secret_values);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                redact_json_secrets(item, explicit_secret_values);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn redact_secrets(input: String, explicit_secret_values: &[String]) -> String {
    let mut redacted = input;

    for secret in explicit_secret_values {
        if secret.is_empty() {
            continue;
        }
        redacted = replace_literal_secret_occurrences(&redacted, secret);
    }

    redacted
}

fn replace_literal_secret_occurrences(input: &str, secret: &str) -> String {
    let mut redacted = String::with_capacity(input.len());
    let mut search_start = 0;

    while let Some(relative_match) = input[search_start..].find(secret) {
        let match_start = search_start + relative_match;
        let match_end = match_start + secret.len();

        redacted.push_str(&input[search_start..match_start]);
        if is_embedded_in_identifier(input, match_start, match_end) {
            redacted.push_str(secret);
        } else {
            redacted.push_str("[REDACTED_SECRET]");
        }

        search_start = match_end;
    }

    redacted.push_str(&input[search_start..]);
    redacted
}

fn is_embedded_in_identifier(input: &str, match_start: usize, match_end: usize) -> bool {
    let prev_is_ident = input[..match_start]
        .chars()
        .next_back()
        .is_some_and(is_identifier_char);
    let next_is_ident = input[match_end..]
        .chars()
        .next()
        .is_some_and(is_identifier_char);
    prev_is_ident || next_is_ident
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn normalize_secret_values(extra_secret_values: &[String]) -> Vec<String> {
    let mut secrets = BTreeSet::new();

    for value in extra_secret_values {
        secrets.insert(value.clone());
    }

    let mut values = secrets.into_iter().collect::<Vec<_>>();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_redact_without_explicit_secret_values() {
        let request_body = serde_json::json!({
            "model": "gpt-test",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "OPENAI_API_KEY=sk_test_abcdefghijklmnopqrstuvwxyz12",
                }]
            }],
            "stream": false
        });

        let raw_body = serde_json::to_vec(&request_body).expect("serialize body");
        let sanitized_body = sanitize_request_body_with_secret_values(&raw_body, &[]);
        let sanitized_json: Value =
            serde_json::from_slice(&sanitized_body).expect("parse sanitized json");

        assert_eq!(
            sanitized_json["input"][0]["content"][0]["text"],
            "OPENAI_API_KEY=sk_test_abcdefghijklmnopqrstuvwxyz12"
        );
    }

    #[test]
    fn sanitizes_socket_provided_secrets() {
        let request_body = serde_json::json!({
            "input": [{
                "content": [{
                    "text": "first socket-secret-value and second socket-secret-value-2",
                    "type": "input_text"
                }],
                "role": "user",
                "type": "message"
            }],
            "stream": false
        });

        let raw_body = serde_json::to_vec(&request_body).expect("serialize body");
        let sanitized_body = sanitize_request_body_with_secret_values(
            &raw_body,
            &[
                "socket-secret-value".to_string(),
                "socket-secret-value-2".to_string(),
            ],
        );
        let sanitized_json: Value =
            serde_json::from_slice(&sanitized_body).expect("parse sanitized json");

        assert_eq!(
            sanitized_json["input"][0]["content"][0]["text"],
            "first [REDACTED_SECRET] and second [REDACTED_SECRET]"
        );
    }

    #[test]
    fn does_not_redact_identifiers_that_contain_secret_substrings() {
        let request_body = serde_json::json!({
            "input": [{
                "content": [{
                    "text": "Keep OPENAI_API_KEY visible, but redact OPENAI and sk-test-value",
                    "type": "input_text"
                }],
                "role": "user",
                "type": "message"
            }],
            "stream": false
        });

        let raw_body = serde_json::to_vec(&request_body).expect("serialize body");
        let sanitized_body = sanitize_request_body_with_secret_values(
            &raw_body,
            &["OPENAI".to_string(), "sk-test-value".to_string()],
        );
        let sanitized_json: Value =
            serde_json::from_slice(&sanitized_body).expect("parse sanitized json");

        assert_eq!(
            sanitized_json["input"][0]["content"][0]["text"],
            "Keep OPENAI_API_KEY visible, but redact [REDACTED_SECRET] and [REDACTED_SECRET]"
        );
    }
}
