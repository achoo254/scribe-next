//! Danh mục tool MCP (tên, mô tả, schema đầu vào, annotation). Mọi tool chỉ đọc; phần chạy nằm ở `calls.rs`.

use serde_json::{json, Value};

/// Số mục mặc định và tối đa một trang của các tool liệt kê.
pub const LIST_DEFAULT: usize = 50;
pub const LIST_MAX: usize = 200;
/// Số câu mặc định và tối đa một trang của bản chép lời và bản dịch.
pub const SENTENCES_DEFAULT: usize = 200;
pub const SENTENCES_MAX: usize = 1000;

const KEY_HELP: &str = "Recording key from scribe_list_transcripts or the transcriptKey of a file: \
`f:<fileId>` for one uploaded or recorded file, `s:<session>` for a phone recording made of 1-minute segments.";

fn key() -> Value {
    json!({ "type": "string", "minLength": 3, "maxLength": 200, "description": KEY_HELP })
}

fn paging(default: usize, max: usize) -> [(&'static str, Value); 2] {
    [
        ("offset", json!({ "type": "integer", "minimum": 0, "default": 0, "description": "Items to skip (use nextOffset of the previous page)." })),
        ("limit", json!({ "type": "integer", "minimum": 1, "maximum": max, "default": default, "description": "Most items to return; a page may hold fewer when it reaches the size cap." })),
    ]
}

fn object(props: Vec<(&'static str, Value)>, required: &[&str]) -> Value {
    let properties: serde_json::Map<String, Value> = props.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
}

fn tool(name: &str, title: &str, description: &str, input: Value, open_world: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input,
        "annotations": { "title": title, "readOnlyHint": true, "destructiveHint": false, "idempotentHint": !open_world, "openWorldHint": open_world },
    })
}

/// `tools/list`: thứ tự cố định, như nhau với mọi token có `scribe:read`.
pub fn definitions() -> Vec<Value> {
    let lang = json!({ "type": "string", "enum": ["en", "vi"], "description": "Target language of the translation." });
    vec![
        tool(
            "scribe_list_transcripts",
            "List recordings with transcripts",
            "One row per recording that has been sent for transcription, newest first: key, title, start time, \
             duration, segment counts by transcription status and a short preview. Start here to find the key that \
             the other transcript tools take.",
            object(paging(LIST_DEFAULT, LIST_MAX).into(), &[]),
            false,
        ),
        tool(
            "scribe_list_files",
            "List files",
            "Files the user keeps in Scribe, newest first. source=history (default): recordings and uploads of the \
             history page, with transcription status and transcriptKey; source=split: files of the split page (uploads \
             to cut and the pieces cut from them, with their jobId). Media cannot be downloaded through this server.",
            object(
                [vec![("source", json!({ "type": "string", "enum": ["history", "split"], "default": "history", "description": "Which page's files to list." }))], paging(LIST_DEFAULT, LIST_MAX).into()].concat(),
                &[],
            ),
            false,
        ),
        tool(
            "scribe_get_file",
            "Get file details",
            "Details of one file by id (from scribe_list_files or a transcript): name, title, kind, size, duration, \
             transcription status, transcriptKey, expiry. Returns not_found for a file that is not the user's.",
            object(vec![("file_id", json!({ "type": "string", "pattern": "^[0-9a-f]{24}$", "description": "24-character hex file id." }))], &["file_id"]),
            false,
        ),
        tool(
            "scribe_list_jobs",
            "List split jobs",
            "Jobs that cut a file into pieces, newest first, with status (queued, running, done, failed), progress and \
             the resulting pieces. Transcription progress is in scribe_get_transcript_status instead.",
            object(paging(LIST_DEFAULT, LIST_MAX).into(), &[]),
            false,
        ),
        tool(
            "scribe_get_job",
            "Get split job status",
            "Status, progress (0-1), resulting pieces and error of one split job.",
            object(vec![("job_id", json!({ "type": "string", "pattern": "^[0-9a-f]{24}$", "description": "24-character hex job id." }))], &["job_id"]),
            false,
        ),
        tool(
            "scribe_get_transcript_status",
            "Get transcription status",
            "Transcription status of each segment of a recording (none, queued, running, done, failed) and its \
             progress, without the text. Use it to wait for a transcript before reading it.",
            object(vec![("key", key())], &["key"]),
            false,
        ),
        tool(
            "scribe_get_transcript",
            "Read a transcript",
            "The transcript of a recording, one page of sentences at a time, with the user's edits applied. Each sentence \
             has its time in seconds from the start of the recording (`at`), the speaker's name when known and the text. \
             Also returns the title, segments and speaker names. Pages hold at most about 25,000 characters: follow \
             nextOffset while hasMore is true, or use scribe_search to find a passage.",
            object([vec![("key", key())], paging(SENTENCES_DEFAULT, SENTENCES_MAX).into()].concat(), &["key"]),
            false,
        ),
        tool(
            "scribe_export_transcript",
            "Export a transcript",
            "The transcript as a file in one of the formats of the app's export: txt, srt, vtt, md (meeting notes with \
             summary and bookmarks) or docx (returned as an embedded resource). lang=en|vi exports the saved translation \
             instead of the original. Text formats are cut at 100,000 characters (say so in the result); read longer \
             transcripts with scribe_get_transcript.",
            object(
                vec![
                    ("key", key()),
                    ("format", json!({ "type": "string", "enum": ["txt", "srt", "vtt", "md", "docx"], "default": "txt" })),
                    ("lang", json!({ "type": "string", "enum": ["en", "vi"], "description": "Export the saved translation into this language." })),
                ],
                &["key"],
            ),
            false,
        ),
        tool(
            "scribe_get_summary",
            "Get meeting summary",
            "The saved summary of a recording: overview, decisions and action items with times, the model and when it \
             was made; `stale` is true when the transcript changed since. Returns summary null when none was made (this \
             server cannot create one).",
            object(vec![("key", key())], &["key"]),
            false,
        ),
        tool(
            "scribe_get_translation",
            "Read a translation",
            "The saved translation of a recording into en or vi, one page of sentences at a time, aligned with the \
             transcript's segments (null where a sentence is not translated; `outdated` when the original changed). \
             Also returns its status and progress. This server cannot start a translation.",
            object([vec![("key", key()), ("lang", lang)], paging(SENTENCES_DEFAULT, SENTENCES_MAX).into()].concat(), &["key", "lang"]),
            false,
        ),
        tool(
            "scribe_get_speakers",
            "Get speaker names",
            "Speaker ids of a recording's transcript (s0, s1, ...) and the names they are shown with.",
            object(vec![("key", key())], &["key"]),
            false,
        ),
        tool(
            "scribe_get_bookmarks",
            "Get bookmarks",
            "Bookmarks the user set on a recording, in time order: time in seconds from the start and label.",
            object(vec![("key", key())], &["key"]),
            false,
        ),
        tool(
            "scribe_search",
            "Search transcripts",
            "Full-text search over the user's transcripts (accents optional), best matches first: recording key and \
             title, time in seconds, the matching sentence and the ones before and after. Free; at most 50 results.",
            object(
                vec![
                    ("query", json!({ "type": "string", "minLength": 1, "maxLength": 200, "description": "Words to find." })),
                    ("key", json!({ "type": "string", "minLength": 3, "maxLength": 200, "description": format!("Search only this recording. {KEY_HELP}") })),
                    ("limit", json!({ "type": "integer", "minimum": 1, "maximum": 50, "default": 20 })),
                ],
                &["query"],
            ),
            false,
        ),
        tool(
            "scribe_ask",
            "Ask about recordings",
            "Answers a question in Vietnamese from the user's own transcripts (all of them, or one recording with key), \
             citing the sentences it used with their times. COSTS QUOTA: each call uses one of the account's daily \
             questions and counts toward its monthly AI cost cap, exactly as asking in the app; accounts not allowed to \
             transcribe get permission_denied. Try scribe_search first when a quote is enough.",
            object(
                vec![
                    ("question", json!({ "type": "string", "minLength": 1, "maxLength": 500 })),
                    ("key", json!({ "type": "string", "minLength": 3, "maxLength": 200, "description": format!("Ask about this recording only. {KEY_HELP}") })),
                ],
                &["question"],
            ),
            true,
        ),
        tool(
            "scribe_get_usage",
            "Get plan and usage",
            "The account's plan, whether it may transcribe, transcription minutes used and left this month (and today \
             on the free plan) and storage used against the plan's quota.",
            object(vec![], &[]),
            false,
        ),
    ]
}

/// Tool có trong danh mục không (tool lạ là lỗi giao thức, không phải lỗi tool).
pub fn is_known(name: &str) -> bool {
    definitions().iter().any(|t| t["name"] == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_read_only_prefixed_and_closed_to_unknown_arguments() {
        let tools = definitions();
        assert_eq!(tools.len(), 15);
        let mut names = std::collections::HashSet::new();
        for t in &tools {
            let name = t["name"].as_str().expect("name");
            assert!(name.starts_with("scribe_") && names.insert(name), "{name}");
            assert_eq!(t["annotations"]["readOnlyHint"], true, "{name}");
            assert_eq!(t["annotations"]["destructiveHint"], false, "{name}");
            assert_eq!(t["inputSchema"]["type"], "object", "{name}");
            assert_eq!(t["inputSchema"]["additionalProperties"], false, "{name}");
        }
        assert!(is_known("scribe_ask") && !is_known("scribe_delete_file"));
    }
}
