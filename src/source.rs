use crate::diagnostic::Position;
use unicode_width::UnicodeWidthChar;
pub struct Source {
    pub text: String,
    pub path: String,
    pub bom: usize,
    starts: Vec<usize>,
}
impl Source {
    pub fn new(text: String, path: String) -> Self {
        let bom = if text.starts_with('\u{feff}') { 3 } else { 0 };
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self {
            text,
            path,
            bom,
            starts,
        }
    }
    pub fn parsed_text(&self) -> &str {
        &self.text[self.bom..]
    }
    pub fn position(&self, offset: usize) -> Position {
        let offset = (offset + self.bom).min(self.text.len());
        let line = self.starts.partition_point(|&p| p <= offset) - 1;
        // BOM is part of the byte range, but not a visible source column.
        let start = self.starts[line] + if line == 0 { self.bom } else { 0 };
        Position {
            line: line + 1,
            column: self.text[start..offset].chars().count() + 1,
        }
    }
    pub fn excerpt(&self, start: usize, end: usize) -> (String, String) {
        let position = self.position(start);
        let end_position = self.position(end);
        let line = self.text[self.starts[position.line - 1]..]
            .split('\n')
            .next()
            .unwrap_or("")
            .trim_end_matches('\r')
            .trim_start_matches('\u{feff}');
        let mut parts = Vec::new();
        let mut column = 0;
        for ch in line.chars() {
            let part = if ch == '\t' {
                " ".repeat(4 - column % 4)
            } else {
                safe(&ch.to_string())
            };
            let width = part.chars().map(|c| c.width().unwrap_or(0)).sum::<usize>();
            parts.push((part, width));
            column += width;
        }
        let begin = position.column.saturating_sub(1).min(parts.len());
        let mut window = begin;
        let mut prefix_width = 0;
        while window > 0 && prefix_width + parts[window - 1].1 <= 60 {
            window -= 1;
            prefix_width += parts[window].1;
        }
        let mut rendered = if window > 0 {
            "…".to_string()
        } else {
            String::new()
        };
        let mut width = usize::from(window > 0);
        let left = width + prefix_width;
        let mut right = left + 1;
        for (index, (part, w)) in parts.iter().enumerate().skip(window) {
            if width + w > 199 {
                rendered.push('…');
                break;
            }
            rendered.push_str(part);
            width += w;
            if index >= begin
                && (end_position.line > position.line || index + 1 < end_position.column)
            {
                right = width.max(left + 1);
            }
        }
        (
            rendered,
            format!(
                "{}{}",
                " ".repeat(left),
                "^".repeat(right.saturating_sub(left).max(1))
            ),
        )
    }
}
pub fn safe(value: &str) -> String {
    value.chars().map(|c| if c.is_control() || matches!(c,'\u{061c}'|'\u{200e}'|'\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {format!("\\u{{{:x}}}",c as u32)} else {c.to_string()}).collect()
}
