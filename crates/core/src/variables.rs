use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Segment {
    Text { raw: String },
    Variable { raw: String, name: String },
    Placeholder { raw: String },
}

impl Segment {
    pub fn raw(&self) -> &str {
        match self {
            Self::Text { raw } | Self::Variable { raw, .. } | Self::Placeholder { raw } => raw,
        }
    }
}

pub fn parse(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut text_start = 0;
    let mut index = 0;

    while index < content.len() {
        if let Some((segment, end)) = token_at(content, index) {
            if text_start < index {
                segments.push(Segment::Text {
                    raw: content[text_start..index].to_string(),
                });
            }
            segments.push(segment);
            index = end;
            text_start = end;
        } else {
            let width = content[index..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            index += width;
        }
    }

    if text_start < content.len() {
        segments.push(Segment::Text {
            raw: content[text_start..].to_string(),
        });
    }
    segments
}

pub fn variable_names(segments: &[Segment]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for segment in segments {
        if let Segment::Variable { name, .. } = segment {
            if seen.insert(name.as_str()) {
                names.push(name.clone());
            }
        }
    }
    names
}

pub fn interpolate(segments: &[Segment], values: &HashMap<String, String>) -> String {
    let mut output = String::new();
    for segment in segments {
        match segment {
            Segment::Variable { raw, name } => match values.get(name) {
                Some(value) if !value.is_empty() => output.push_str(value),
                _ => output.push_str(raw),
            },
            Segment::Text { raw } | Segment::Placeholder { raw } => output.push_str(raw),
        }
    }
    output
}

fn token_at(content: &str, index: usize) -> Option<(Segment, usize)> {
    let tail = content.get(index..)?;
    if tail.starts_with("{{") {
        let close_offset = tail.get(2..)?.find("}}")?;
        let interior_end = index + 2 + close_offset;
        let interior = &content[index + 2..interior_end];
        if interior
            .chars()
            .any(|character| matches!(character, '{' | '}' | '\n' | '\r'))
        {
            return None;
        }
        let name = interior.trim_matches(|character| matches!(character, ' ' | '\t'));
        if name.is_empty() {
            return None;
        }
        let end = interior_end + 2;
        return Some((
            Segment::Variable {
                raw: content[index..end].to_string(),
                name: name.to_string(),
            },
            end,
        ));
    }

    if !tail.starts_with('[') {
        return None;
    }
    let bytes = tail.as_bytes();
    if !bytes.get(1).is_some_and(u8::is_ascii_uppercase) {
        return None;
    }
    for (offset, byte) in bytes.iter().copied().enumerate().skip(2) {
        if byte == b']' {
            let end = index + offset + 1;
            return Some((
                Segment::Placeholder {
                    raw: content[index..end].to_string(),
                },
                end,
            ));
        }
        if !(byte.is_ascii_uppercase() || matches!(byte, b' ' | b'_')) {
            return None;
        }
    }
    None
}
