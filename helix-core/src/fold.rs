use crate::RopeSlice;

/// A foldable range represented as a line range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldableRange {
    /// The first line of the foldable region (always displayed).
    pub start_line: usize,
    /// The last line of the foldable region (hidden when folded).
    pub end_line: usize,
}

/// Compute foldable ranges based on indentation levels.
/// Used as a fallback when no tree-sitter fold query is available.
pub fn indent_based_fold_ranges(text: RopeSlice, tab_width: usize) -> Vec<FoldableRange> {
    let line_count = text.len_lines();
    if line_count == 0 {
        return Vec::new();
    }

    let indent_levels: Vec<Option<usize>> = (0..line_count)
        .map(|i| {
            let line = text.line(i);
            let trimmed_len = line.chars().take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r').count();
            let line_len = line.len_chars();
            let is_blank = line_len == 0
                || line.chars().all(|c| c == '\n' || c == '\r' || c == ' ' || c == '\t');
            if is_blank {
                None
            } else {
                Some(trimmed_len / tab_width.max(1))
            }
        })
        .collect();

    // Resolve blank lines: inherit the minimum indent of surrounding non-blank lines
    let mut resolved: Vec<usize> = vec![0; line_count];
    for i in 0..line_count {
        if let Some(level) = indent_levels[i] {
            resolved[i] = level;
        } else {
            let prev = (0..i)
                .rev()
                .find_map(|j| indent_levels[j])
                .unwrap_or(0);
            let next = (i + 1..line_count)
                .find_map(|j| indent_levels[j])
                .unwrap_or(0);
            resolved[i] = prev.min(next);
        }
    }

    let mut ranges = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new(); // (start_line, indent_level)

    for i in 0..line_count {
        let level = resolved[i];
        // Close any ranges where indentation has returned to the starting level
        while let Some(&(start, start_level)) = stack.last() {
            if level <= start_level {
                let end_line = i - 1;
                if end_line > start {
                    ranges.push(FoldableRange {
                        start_line: start,
                        end_line,
                    });
                }
                stack.pop();
            } else {
                break;
            }
        }

        // Start new range if next line has higher indentation
        if i + 1 < line_count && resolved[i + 1] > level {
            stack.push((i, level));
        }
    }

    // Close remaining open ranges
    while let Some((start, _)) = stack.pop() {
        let end_line = line_count - 1;
        if end_line > start {
            ranges.push(FoldableRange {
                start_line: start,
                end_line,
            });
        }
    }

    ranges.sort_by_key(|r| r.start_line);
    ranges
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Rope;

    #[test]
    fn test_indent_fold_simple() {
        let text = Rope::from("fn main() {\n    let x = 1;\n    let y = 2;\n}\n");
        let ranges = indent_based_fold_ranges(text.slice(..), 4);
        assert!(!ranges.is_empty());
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 2);
    }

    #[test]
    fn test_indent_fold_nested() {
        let text = Rope::from("a\n  b\n    c\n    d\n  e\nf\n");
        let ranges = indent_based_fold_ranges(text.slice(..), 2);
        assert!(ranges.len() >= 2);
    }

    #[test]
    fn test_indent_fold_empty() {
        let text = Rope::from("");
        let ranges = indent_based_fold_ranges(text.slice(..), 4);
        assert!(ranges.is_empty());
    }
}
