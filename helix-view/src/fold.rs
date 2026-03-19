use std::ops::Range;

/// The source that produced a fold range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldSource {
    TreeSitter,
    Indent,
    Selection,
}

/// A folded range, representing lines hidden in a view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldRange {
    /// First line of the fold (always displayed).
    pub start_line: usize,
    /// Last line of the fold (hidden).
    pub end_line: usize,
    /// Where this fold originated from.
    pub source: FoldSource,
}

/// Per-view fold state. Tracks which regions are currently folded.
#[derive(Debug, Clone, Default)]
pub struct FoldState {
    /// Sorted by start_line, non-overlapping.
    folded: Vec<FoldRange>,
}

impl FoldState {
    pub fn new() -> Self {
        Self {
            folded: Vec::new(),
        }
    }

    /// Fold a region at the given line using pre-computed foldable ranges.
    /// Finds the innermost foldable range that contains `line` and folds it.
    pub fn fold_at(&mut self, line: usize, foldable_ranges: &[(usize, usize)]) -> bool {
        let range = foldable_ranges
            .iter()
            .filter(|(start, end)| *start <= line && line <= *end && *end > *start)
            .min_by_key(|(start, end)| end - start);

        if let Some(&(start, end)) = range {
            self.add_fold(start, end, FoldSource::TreeSitter)
        } else {
            false
        }
    }

    /// Fold an explicit selection range (supports multi-selection).
    pub fn fold_selection(&mut self, start_line: usize, end_line: usize) -> bool {
        if end_line <= start_line {
            return false;
        }
        self.add_fold(start_line, end_line, FoldSource::Selection)
    }

    /// Unfold any fold that covers the given line.
    pub fn unfold_at(&mut self, line: usize) -> bool {
        let idx = self.folded.iter().position(|f| f.start_line <= line && line <= f.end_line);
        if let Some(idx) = idx {
            self.folded.remove(idx);
            true
        } else {
            false
        }
    }

    /// Toggle fold at the given line.
    pub fn toggle_at(&mut self, line: usize, foldable_ranges: &[(usize, usize)]) -> bool {
        if self.is_fold_start(line).is_some() || self.is_folded(line) {
            self.unfold_at(line)
        } else {
            self.fold_at(line, foldable_ranges)
        }
    }

    /// Fold all foldable ranges.
    pub fn fold_all(&mut self, foldable_ranges: &[(usize, usize)]) {
        self.folded.clear();
        for &(start, end) in foldable_ranges {
            if end > start {
                self.folded.push(FoldRange {
                    start_line: start,
                    end_line: end,
                    source: FoldSource::TreeSitter,
                });
            }
        }
        self.normalize();
    }

    /// Unfold everything.
    pub fn unfold_all(&mut self) {
        self.folded.clear();
    }

    /// Check if a line is hidden by a fold (not the fold header line).
    pub fn is_folded(&self, line: usize) -> bool {
        self.folded
            .iter()
            .any(|f| line > f.start_line && line <= f.end_line)
    }

    /// If `line` is the start of a fold, return the fold range.
    pub fn is_fold_start(&self, line: usize) -> Option<&FoldRange> {
        self.folded.iter().find(|f| f.start_line == line)
    }

    /// Returns the total number of hidden lines.
    pub fn folded_line_count(&self) -> usize {
        self.folded
            .iter()
            .map(|f| f.end_line - f.start_line)
            .sum()
    }

    /// Iterator over all folded ranges.
    pub fn iter(&self) -> impl Iterator<Item = &FoldRange> {
        self.folded.iter()
    }

    /// Returns fold ranges as (start_char, end_char) pairs for use in TextAnnotations.
    /// `start_char` is the first char of the line AFTER the fold header (first hidden line).
    /// `end_char` is the first char of the line AFTER the last hidden line.
    pub fn char_ranges(&self, text: helix_core::RopeSlice) -> Vec<Range<usize>> {
        let line_count = text.len_lines();
        self.folded
            .iter()
            .filter_map(|f| {
                let skip_start_line = f.start_line + 1;
                let skip_end_line = f.end_line + 1;
                if skip_start_line >= line_count {
                    return None;
                }
                let start_char = text.line_to_char(skip_start_line);
                let end_char = if skip_end_line >= line_count {
                    text.len_chars()
                } else {
                    text.line_to_char(skip_end_line)
                };
                if start_char < end_char {
                    Some(start_char..end_char)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns char positions where fold indicators ("⋯") should appear.
    /// Each position is the newline char at the end of a fold header line.
    pub fn fold_indicator_positions(&self, text: helix_core::RopeSlice) -> Vec<usize> {
        let line_count = text.len_lines();
        self.folded
            .iter()
            .filter_map(|f| {
                if f.start_line + 1 >= line_count {
                    return None;
                }
                let next_line_start = text.line_to_char(f.start_line + 1);
                if next_line_start > 0 {
                    Some(next_line_start - 1)
                } else {
                    None
                }
            })
            .collect()
    }

    fn add_fold(&mut self, start_line: usize, end_line: usize, source: FoldSource) -> bool {
        // Don't add if already folded at this position
        if self.folded.iter().any(|f| f.start_line == start_line) {
            return false;
        }

        self.folded.push(FoldRange {
            start_line,
            end_line,
            source,
        });
        self.normalize();
        true
    }

    fn normalize(&mut self) {
        self.folded.sort_by_key(|f| f.start_line);
        self.folded.dedup_by(|b, a| {
            // Merge overlapping ranges
            if b.start_line <= a.end_line {
                a.end_line = a.end_line.max(b.end_line);
                true
            } else {
                false
            }
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_fold_unfold() {
        let mut state = FoldState::new();
        let ranges = vec![(0, 5), (2, 4), (7, 10)];

        assert!(state.fold_at(3, &ranges));
        assert!(state.is_fold_start(2).is_some());
        assert!(state.is_folded(3));
        assert!(!state.is_folded(0));

        assert!(state.unfold_at(3));
        assert!(!state.is_folded(3));
    }

    #[test]
    fn test_fold_selection() {
        let mut state = FoldState::new();
        assert!(state.fold_selection(5, 10));
        assert!(state.is_fold_start(5).is_some());
        assert!(state.is_folded(7));
        assert!(!state.is_folded(5));
        assert!(!state.fold_selection(5, 5));
    }

    #[test]
    fn test_fold_all() {
        let mut state = FoldState::new();
        let ranges = vec![(0, 3), (5, 8)];
        state.fold_all(&ranges);
        assert_eq!(state.folded.len(), 2);
        assert_eq!(state.folded_line_count(), 6);
    }

    #[test]
    fn test_toggle() {
        let mut state = FoldState::new();
        let ranges = vec![(0, 5)];

        state.toggle_at(0, &ranges);
        assert!(state.is_fold_start(0).is_some());

        state.toggle_at(0, &ranges);
        assert!(state.is_fold_start(0).is_none());
    }
}
