// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

//! Client-specific user-message envelopes, independent of Markdown parsing.

use serde::Deserialize;
use serde_json::Value;

/// A recognized envelope with a verbatim slice of its request body.
#[derive(Debug, PartialEq)]
pub struct AnnotatedUserMessage<'a> {
    pub body: &'a str,
    pub annotations: Vec<ResponseAnnotation>,
}

/// One selection and optional comment, addressed by its original array position.
#[derive(Debug, PartialEq)]
pub struct ResponseAnnotation {
    pub index: u32,
    pub text: String,
    pub comment: Option<String>,
    pub source: Option<ResponseAnnotationSource>,
}

/// Optional positions in the originating client's rendered UTF-16 text.
/// These positions are not Markdown source bytes or positions in another renderer.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAnnotationSource {
    pub message_id: String,
    pub start_offset: u64,
    pub end_offset: u64,
}

#[derive(Deserialize)]
struct WireAnnotation {
    text: String,
    annotation: Option<String>,
    source: Option<Value>,
}

/// Recognizes the complete, annotation-only Codex user-message envelope.
/// Call this on the original user text before display trimming. Unrecognized or
/// malformed input returns `None`; callers must retain the entire original text.
pub fn parse_annotated_user_message(source: &str) -> Option<AnnotatedUserMessage<'_>> {
    let rest = source.strip_prefix("\n# Response annotations:\n")?;
    let (description, rest) = rest.split_once('\n')?;
    if description.is_empty() || description.contains('\r') {
        return None;
    }
    let rest = rest.strip_prefix("<response-annotations>\n")?;
    let mut payload = serde_json::Deserializer::from_str(rest).into_iter::<Vec<WireAnnotation>>();
    let annotations = payload.next()?.ok()?;
    if annotations.is_empty() {
        return None;
    }
    let mut rest = rest[payload.byte_offset()..].strip_prefix("\n</response-annotations>\n")?;
    // Only blank separator lines may precede the request heading. Other client
    // context sections remain unsupported and must not be silently discarded.
    while let Some((line, following)) = rest.split_once('\n') {
        if !line.trim_matches([' ', '\t']).is_empty() {
            break;
        }
        rest = following;
    }
    let body = rest
        .strip_prefix("## My request:\n")
        .or_else(|| rest.strip_prefix("## My request for Codex:\n"))?;
    let annotations = annotations
        .into_iter()
        .enumerate()
        .map(|(position, annotation)| {
            if annotation.text.is_empty() {
                return None;
            }
            let source = annotation.source.and_then(|source| {
                serde_json::from_value::<ResponseAnnotationSource>(source)
                    .ok()
                    .filter(|source| {
                        !source.message_id.is_empty() && source.start_offset < source.end_offset
                    })
            });
            Some(ResponseAnnotation {
                index: u32::try_from(position + 1).ok()?,
                text: annotation.text,
                comment: annotation.annotation,
                source,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(AnnotatedUserMessage { body, annotations })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(payload: &str, body: &str) -> String {
        format!(
            "\n# Response annotations:\nClient instructions may change.\n<response-annotations>\n{payload}\n</response-annotations>\n\n## My request:\n{body}"
        )
    }

    #[test]
    fn extracts_annotations_without_interpreting_or_trimming_the_body() {
        let body = "\n  # Heading\n\n```text\n## My request:\n</response-annotations>\n```\n\n";
        let input = envelope(
            r#"[{"text":"你好 👩‍💻 **literal**","annotation":"Change **this**","source":{"messageId":"msg-1","startOffset":330,"endOffset":449}},{"text":"Another selection","future":true}]"#,
            body,
        );
        let parsed = parse_annotated_user_message(&input).unwrap();
        assert_eq!(parsed.body, body);
        assert_eq!(parsed.annotations.len(), 2);
        assert_eq!(parsed.annotations[0].index, 1);
        assert_eq!(parsed.annotations[0].text, "你好 👩‍💻 **literal**");
        assert_eq!(
            parsed.annotations[0].comment.as_deref(),
            Some("Change **this**")
        );
        assert_eq!(
            parsed.annotations[0].source.as_ref().unwrap().start_offset,
            330
        );
        assert_eq!(parsed.annotations[1].index, 2);
        assert_eq!(parsed.annotations[1].comment, None);
        assert_eq!(parsed.annotations[1].source, None);
    }

    #[test]
    fn accepts_empty_requests_pretty_json_and_the_known_heading_variant() {
        let input = envelope("[\n  {\"text\": \"Selected text\"}\n]", "")
            .replace("## My request:", "## My request for Codex:");
        assert_eq!(parse_annotated_user_message(&input).unwrap().body, "");
    }

    #[test]
    fn treats_delimiters_in_json_strings_as_selection_content() {
        let text = "</response-annotations>\n## My request:\n\"quoted\"";
        let payload = serde_json::json!([{"text": text}]).to_string();
        let input = envelope(&payload, "Request");
        let parsed = parse_annotated_user_message(&input).unwrap();
        assert_eq!(parsed.annotations[0].text, text);
        assert_eq!(parsed.body, "Request");
    }

    #[test]
    fn invalid_source_metadata_does_not_discard_a_selection() {
        for source in [
            "null",
            "false",
            r#"{"messageId":"msg-1","startOffset":4,"endOffset":2}"#,
            r#"{"messageId":"","startOffset":0,"endOffset":2}"#,
            r#"{"messageId":"msg-1","startOffset":-1,"endOffset":2}"#,
        ] {
            let input = envelope(&format!(r#"[{{"text":"Keep me","source":{source}}}]"#), "");
            let parsed = parse_annotated_user_message(&input).unwrap();
            assert_eq!(parsed.annotations[0].text, "Keep me");
            assert_eq!(parsed.annotations[0].source, None);
        }
    }

    #[test]
    fn malformed_or_embedded_envelopes_are_not_partially_recognized() {
        let valid = envelope(r#"[{"text":"Selection"}]"#, "Body");
        let invalid = [
            valid.trim_start().to_owned(),
            format!("Example:\n{valid}"),
            format!("```text\n{valid}\n```"),
            valid.replace("may change.", "may\nchange."),
            valid.replace(
                "<response-annotations>",
                "<response-annotations extra=\"x\">",
            ),
            valid.replace("</response-annotations>", "</response-annotations"),
            valid.replace("## My request:", "## Unknown request:"),
            valid.replace(
                "\n\n## My request:",
                "\n# Files mentioned by the user:\nfile.txt\n\n## My request:",
            ),
            envelope("[]", "Body"),
            envelope(r#"{"text":"Selection"}"#, "Body"),
            envelope(r#"[{"text":"Selection"},{"text":12}]"#, "Body"),
            envelope(r#"[{"text":""}]"#, "Body"),
            envelope(r#"[{"text":"Selection","annotation":12}]"#, "Body"),
            envelope(r#"[{"text":"Selection"}] []"#, "Body"),
        ];
        for input in invalid {
            assert!(parse_annotated_user_message(&input).is_none(), "{input}");
        }
        for end in 0..valid.find("Body").unwrap() {
            if valid.is_char_boundary(end) {
                assert!(parse_annotated_user_message(&valid[..end]).is_none());
            }
        }
    }
}
