pub(crate) fn line_window_context(
    content: &str,
    line: usize,
    lines_before: usize,
    lines_after: usize,
    max_chars: Option<usize>,
) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let target = line.saturating_sub(1).min(lines.len() - 1);
    let start = target.saturating_sub(lines_before);
    let end = usize::min(target + lines_after + 1, lines.len());
    let context = lines[start..end].join("\n");

    Some(match max_chars {
        Some(limit) => context.trim().chars().take(limit).collect(),
        None => context,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_window_context_returns_neighboring_lines() {
        let content = "one\ntwo\nthree\nfour\nfive";

        assert_eq!(
            line_window_context(content, 3, 2, 2, None).as_deref(),
            Some("one\ntwo\nthree\nfour\nfive")
        );
    }

    #[test]
    fn line_window_context_clamps_edges() {
        let content = "one\ntwo\nthree";

        assert_eq!(
            line_window_context(content, 1, 2, 2, None).as_deref(),
            Some("one\ntwo\nthree")
        );
        assert_eq!(
            line_window_context(content, 99, 0, 0, None).as_deref(),
            Some("three")
        );
    }

    #[test]
    fn line_window_context_can_trim_and_truncate() {
        let content = "  abcdef  ";

        assert_eq!(
            line_window_context(content, 1, 0, 0, Some(3)).as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn line_window_context_returns_none_for_empty_content() {
        assert_eq!(line_window_context("", 1, 2, 2, None), None);
    }
}
