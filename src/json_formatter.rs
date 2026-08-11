use serde::Serialize;
use serde_json::ser::{PrettyFormatter, Serializer};

const JSON_EDITOR_MIN_HEIGHT: f32 = 180.0;
const JSON_EDITOR_RESERVED_HEIGHT: f32 = 140.0;
const JSON_FORMATTER_MIN_WINDOW_HEIGHT: f32 = 360.0;

pub fn line_numbers(text: &str) -> String {
    let line_count = text.split('\n').count();
    let width = line_count.to_string().len();
    (1..=line_count)
        .map(|line| format!("{line:>width$}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JsonFormat {
    #[default]
    PrettyFourSpaces,
    PrettyTwoSpaces,
    Compact,
    EscapedString,
}

impl JsonFormat {
    pub const ALL: [JsonFormat; 4] = [
        JsonFormat::PrettyFourSpaces,
        JsonFormat::PrettyTwoSpaces,
        JsonFormat::Compact,
        JsonFormat::EscapedString,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            JsonFormat::PrettyFourSpaces => "4 空格美化",
            JsonFormat::PrettyTwoSpaces => "2 空格美化",
            JsonFormat::Compact => "压缩单行",
            JsonFormat::EscapedString => "转义为 JSON 字符串",
        }
    }
}

pub fn format_json(input: &str, format: JsonFormat) -> Result<String, serde_json::Error> {
    let value = serde_json::from_str::<serde_json::Value>(input)?;

    match format {
        JsonFormat::PrettyFourSpaces => {
            let mut buf = Vec::new();
            let formatter = PrettyFormatter::with_indent(b"    ");
            let mut ser = Serializer::with_formatter(&mut buf, formatter);
            value.serialize(&mut ser)?;
            Ok(String::from_utf8(buf).unwrap())
        }
        JsonFormat::PrettyTwoSpaces => {
            let mut buf = Vec::new();
            let formatter = PrettyFormatter::with_indent(b"  ");
            let mut ser = Serializer::with_formatter(&mut buf, formatter);
            value.serialize(&mut ser)?;
            Ok(String::from_utf8(buf).unwrap())
        }
        JsonFormat::Compact => serde_json::to_string(&value),
        JsonFormat::EscapedString => serde_json::to_string(input),
    }
}

pub fn json_editor_height(available_height: f32) -> f32 {
    (available_height - JSON_EDITOR_RESERVED_HEIGHT).max(JSON_EDITOR_MIN_HEIGHT)
}

pub const fn json_formatter_min_window_height() -> f32 {
    JSON_FORMATTER_MIN_WINDOW_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::{
        format_json, json_editor_height, json_formatter_min_window_height, line_numbers,
        JsonFormat, JSON_EDITOR_MIN_HEIGHT, JSON_EDITOR_RESERVED_HEIGHT,
    };

    #[test]
    fn json_editor_height_grows_with_available_space() {
        assert!(json_editor_height(900.0) > json_editor_height(600.0));
    }

    #[test]
    fn json_editor_height_respects_minimum() {
        assert_eq!(json_editor_height(100.0), 180.0);
    }

    #[test]
    fn minimum_window_height_leaves_room_for_editor_and_controls() {
        assert!(
            json_formatter_min_window_height() - JSON_EDITOR_RESERVED_HEIGHT
                >= JSON_EDITOR_MIN_HEIGHT
        );
    }

    #[test]
    fn formats_json_with_four_spaces() {
        let input = r#"{"name":"小花","items":[1,true]}"#;

        assert_eq!(
            format_json(input, JsonFormat::PrettyFourSpaces).unwrap(),
            "{\n    \"items\": [\n        1,\n        true\n    ],\n    \"name\": \"小花\"\n}"
        );
    }

    #[test]
    fn formats_json_with_two_spaces() {
        let input = r#"{"name":"小花","items":[1,true]}"#;

        assert_eq!(
            format_json(input, JsonFormat::PrettyTwoSpaces).unwrap(),
            "{\n  \"items\": [\n    1,\n    true\n  ],\n  \"name\": \"小花\"\n}"
        );
    }

    #[test]
    fn formats_json_as_compact_single_line() {
        assert_eq!(
            format_json(" { \"a\" : 1 } ", JsonFormat::Compact).unwrap(),
            r#"{"a":1}"#
        );
    }

    #[test]
    fn escapes_the_original_valid_json_text_as_a_json_string() {
        let original = "{\n  \"a\": \"x\"\n}";

        assert_eq!(
            format_json(original, JsonFormat::EscapedString).unwrap(),
            serde_json::to_string(original).unwrap()
        );
    }

    #[test]
    fn rejects_invalid_json_for_every_format() {
        for format in JsonFormat::ALL {
            assert!(format_json("{invalid", format).is_err());
        }
    }

    #[test]
    fn defaults_to_four_space_pretty_formatting() {
        assert_eq!(JsonFormat::default(), JsonFormat::PrettyFourSpaces);
    }

    #[test]
    fn format_labels_are_stable_and_ordered() {
        assert_eq!(
            JsonFormat::ALL.map(JsonFormat::label),
            ["4 空格美化", "2 空格美化", "压缩单行", "转义为 JSON 字符串",]
        );
    }

    #[test]
    fn line_numbers_cover_empty_single_and_multiline_text() {
        assert_eq!(line_numbers(""), "1");
        assert_eq!(line_numbers("{}"), "1");
        assert_eq!(line_numbers("{\n  \"a\": 1\n}"), "1\n2\n3");
        assert_eq!(line_numbers("{}\n"), "1\n2");
    }

    #[test]
    fn line_numbers_align_to_the_largest_number() {
        let text = (0..12).map(|_| "x").collect::<Vec<_>>().join("\n");
        assert!(line_numbers(&text).starts_with(" 1\n 2"));
        assert!(line_numbers(&text).ends_with("\n12"));
    }
}
