/// Accumulates text from stream-json assistant events and yields complete lines.
///
/// No ANSI stripping. No JSON parsing. No SQLite. No Tauri.
/// This is the only place the text accumulation concern lives.
pub struct TextBufExtractor {
    buf: String,
}

impl Default for TextBufExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TextBufExtractor {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    /// Push a text chunk and return any complete lines (split on '\n').
    ///
    /// Uses `drain()` to consume matched portions in-place, avoiding a full
    /// re-allocation of the tail on every extracted line.
    pub fn push(&mut self, text: &str) -> Vec<String> {
        self.buf.push_str(text);
        let mut lines = Vec::new();
        while let Some(pos) = self.buf.find('\n') {
            // Drain up to and including the '\n', collect the chars before it as
            // the line (excluding the newline itself).
            let line: String = self.buf.drain(..=pos).take(pos).collect();
            lines.push(line);
        }
        lines
    }

    /// Return and clear the remaining buffer (no trailing newline).
    /// Call after on_complete to flush the tail of the last event.
    pub fn flush(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buf))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_single_complete_line() {
        let mut ex = TextBufExtractor::new();
        let lines = ex.push("hello\n");
        assert_eq!(lines, vec!["hello"]);
        assert_eq!(ex.flush(), None);
    }

    #[test]
    fn push_partial_then_complete() {
        let mut ex = TextBufExtractor::new();
        assert!(ex.push("hel").is_empty());
        let lines = ex.push("lo\n");
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn push_multiple_lines() {
        let mut ex = TextBufExtractor::new();
        let lines = ex.push("a\nb\nc\n");
        assert_eq!(lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn flush_tail() {
        let mut ex = TextBufExtractor::new();
        ex.push("no newline here");
        assert_eq!(ex.flush(), Some("no newline here".to_string()));
        assert_eq!(ex.flush(), None);
    }

    #[test]
    fn poe_event_spanning_chunks() {
        let mut ex = TextBufExtractor::new();
        assert!(ex.push(r#"{"poe":"br"#).is_empty());
        let lines2 = ex.push("{\"ief\":\"ok\"}\n");
        assert_eq!(lines2.len(), 1);
    }
}
