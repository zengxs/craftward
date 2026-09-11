// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashMap, ops::Range};

use crate::semantic::{CodeComment, MappedText};

pub(crate) struct Attribute {
    pub value: MappedText,
    // Decoded UTF-8 byte boundaries mapped back to the original attribute.
    source_offsets: Vec<usize>,
}

impl Attribute {
    pub fn source_range(&self, range: Range<usize>) -> Range<usize> {
        self.source_offsets[range.start]..self.source_offsets[range.end]
    }
}

pub(crate) struct ParsedComment {
    pub comment: CodeComment,
    pub body: Attribute,
}

/// Recognizes one complete standalone directive before Markdown parses its values.
pub(crate) fn parse(source: &str, range: Range<usize>) -> Option<ParsedComment> {
    let input = source.get(range.clone())?.trim_end();
    let mut cursor = "::code-comment{".len();
    input.strip_prefix("::code-comment{")?;
    let mut attributes = HashMap::new();
    loop {
        whitespace(input, &mut cursor);
        if input.as_bytes().get(cursor) == Some(&b'}') {
            cursor += 1;
            break;
        }
        let start = cursor;
        while input
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            cursor += 1;
        }
        let name = input.get(start..cursor)?;
        if !matches!(
            name,
            "title" | "body" | "file" | "start" | "end" | "priority"
        ) {
            return None;
        }
        whitespace(input, &mut cursor);
        if input.as_bytes().get(cursor) != Some(&b'=') {
            return None;
        }
        cursor += 1;
        whitespace(input, &mut cursor);
        let quoted = input.as_bytes().get(cursor) == Some(&b'"');
        if matches!(name, "title" | "body" | "file") && !quoted {
            return None;
        }
        let value = attribute(source, input, range.start, &mut cursor, quoted)?;
        if attributes.insert(name, value).is_some() {
            return None;
        }
        if !input
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'}')
        {
            return None;
        }
    }
    if cursor != input.len() {
        return None;
    }
    let title = attributes.remove("title")?.value;
    let file = attributes.remove("file")?.value;
    let body = attributes.remove("body")?;
    if [&title.text, &file.text, &body.value.text]
        .iter()
        .any(|text| text.trim().is_empty())
    {
        return None;
    }
    let number = |name| -> Option<Option<u32>> {
        let Some(attribute) = attributes.get(name) else {
            return Some(None);
        };
        let text = &attribute.value.text;
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Some(Some(text.parse().ok()?))
    };
    let start = number("start")?;
    let end = number("end")?;
    let priority = number("priority")?;
    if start.is_some_and(|line| line == 0 || line > i32::MAX as u32)
        || end.is_some_and(|line| start.is_none_or(|first| line < first) || line > i32::MAX as u32)
        || priority.is_some_and(|priority| priority > 3)
    {
        return None;
    }
    Some(ParsedComment {
        comment: CodeComment {
            title,
            file,
            start,
            end,
            priority,
        },
        body,
    })
}

fn whitespace(input: &str, cursor: &mut usize) {
    while input
        .as_bytes()
        .get(*cursor)
        .is_some_and(u8::is_ascii_whitespace)
    {
        *cursor += 1;
    }
}

fn attribute(
    source: &str,
    input: &str,
    offset: usize,
    cursor: &mut usize,
    quoted: bool,
) -> Option<Attribute> {
    if quoted {
        *cursor += 1;
    }
    let start = *cursor;
    let mut text = String::new();
    let mut source_offsets = vec![offset + start];
    loop {
        let character = input.get(*cursor..)?.chars().next()?;
        if (quoted && character == '"')
            || (!quoted && (character.is_ascii_whitespace() || character == '}'))
        {
            break;
        }
        if !quoted && !character.is_ascii_digit() {
            return None;
        }
        let original_start = *cursor;
        let mut decoded = character;
        *cursor += character.len_utf8();
        if quoted && character == '\\' {
            let escaped = input.get(*cursor..)?.chars().next()?;
            let replacement = match escaped {
                '"' | '\\' => Some(escaped),
                'n' => Some('\n'),
                'r' => Some('\r'),
                't' => Some('\t'),
                _ => None,
            };
            if let Some(replacement) = replacement {
                decoded = replacement;
                *cursor += escaped.len_utf8();
            }
        }
        text.push(decoded);
        for byte in 1..decoded.len_utf8() {
            source_offsets.push(offset + original_start + byte);
        }
        source_offsets.push(offset + *cursor);
    }
    let value = MappedText::new(source, offset + start..offset + *cursor, text);
    if quoted {
        *cursor += 1;
    }
    Some(Attribute {
        value,
        source_offsets,
    })
}
