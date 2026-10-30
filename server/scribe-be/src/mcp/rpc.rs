//! JSON-RPC 2.0 tối thiểu cho MCP Streamable HTTP không trạng thái: mỗi POST đúng một message object, không batch.

use serde_json::{json, Map, Value};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

/// Phiên bản giao thức MCP endpoint nói được, mới nhất trước. Client xin bản khác thì nhận bản mới nhất (spec
/// "Lifecycle": client tự quyết có dùng tiếp không).
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Một message client gửi lên.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    /// Có `id`: phải trả lời.
    Request { id: Value, method: String, params: Value },
    /// Notification, hoặc câu trả lời của client cho request của server: nhận rồi trả 202, không có body.
    Accepted,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Mảng message (batch): MCP mới đã bỏ, endpoint không nhận.
    Batch,
    /// Không phải đúng một object JSON-RPC 2.0 hợp lệ.
    Invalid,
}

pub fn parse(body: &[u8]) -> Result<Incoming, ParseError> {
    if body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'[') {
        return Err(ParseError::Batch);
    }
    let value: Value = serde_json::from_slice(body).map_err(|_| ParseError::Invalid)?;
    let Value::Object(msg) = value else { return Err(ParseError::Invalid) };
    if msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(ParseError::Invalid);
    }
    match msg.get("method") {
        Some(Value::String(method)) => match msg.get("id") {
            None => Ok(Incoming::Accepted),
            Some(id) if is_valid_id(id) => {
                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                if !(params.is_object() || params.is_null()) {
                    return Err(ParseError::Invalid);
                }
                Ok(Incoming::Request { id: id.clone(), method: method.clone(), params })
            }
            Some(_) => Err(ParseError::Invalid),
        },
        Some(_) => Err(ParseError::Invalid),
        None if is_client_response(&msg) => Ok(Incoming::Accepted),
        None => Err(ParseError::Invalid),
    }
}

/// MCP: id là chuỗi hoặc số nguyên, không `null`.
fn is_valid_id(id: &Value) -> bool {
    id.is_string() || id.is_i64() || id.is_u64()
}

fn is_client_response(msg: &Map<String, Value>) -> bool {
    msg.get("id").is_some_and(is_valid_id) && (msg.contains_key("result") || msg.contains_key("error"))
}

pub fn success(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Phiên bản trả lời `initialize`: đúng bản client xin nếu nói được, không thì bản mới nhất.
pub fn negotiate(requested: Option<&str>) -> &'static str {
    PROTOCOL_VERSIONS.iter().copied().find(|v| Some(*v) == requested).unwrap_or(PROTOCOL_VERSIONS[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_need_one_jsonrpc_object_with_a_string_or_integer_id() {
        let req = parse(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).expect("hợp lệ");
        assert_eq!(req, Incoming::Request { id: json!(1), method: "tools/list".into(), params: Value::Null });
        let named = parse(br#"{"jsonrpc":"2.0","id":"a","method":"ping","params":{}}"#).expect("hợp lệ");
        assert!(matches!(named, Incoming::Request { ref id, .. } if id == "a"));

        assert_eq!(parse(br#" [{"jsonrpc":"2.0","id":1,"method":"ping"}]"#), Err(ParseError::Batch));
        for bad in [
            &br#"{"id":1,"method":"ping"}"#[..],
            br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
            br#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":7}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[1]}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":"ping"} {}"#,
            br#""ping""#,
            b"",
        ] {
            assert_eq!(parse(bad), Err(ParseError::Invalid), "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn notifications_and_client_responses_are_accepted_without_reply() {
        assert_eq!(parse(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), Ok(Incoming::Accepted));
        assert_eq!(parse(br#"{"jsonrpc":"2.0","id":3,"result":{}}"#), Ok(Incoming::Accepted));
        assert_eq!(parse(br#"{"jsonrpc":"2.0","id":3}"#), Err(ParseError::Invalid));
    }

    #[test]
    fn version_negotiation_keeps_a_known_version_and_falls_back_to_the_newest() {
        assert_eq!(negotiate(Some("2025-06-18")), "2025-06-18");
        assert_eq!(negotiate(Some("1999-01-01")), PROTOCOL_VERSIONS[0]);
        assert_eq!(negotiate(None), PROTOCOL_VERSIONS[0]);
    }
}
