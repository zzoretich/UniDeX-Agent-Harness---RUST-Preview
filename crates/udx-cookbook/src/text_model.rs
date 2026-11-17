//! The text, selection and marked (composing) range of a text field, edited the way GPUI's input handler asks.
//!
//! The platform layer never edits a field itself. On every OS it calls the field's `EntityInputHandler` with committed
//! text (`replace_text_in_range`), composition updates from an input method or a dead key
//! (`replace_and_mark_text_in_range`), the end of a composition (`unmark_text`), and questions about the text. Ranges in
//! those calls are UTF-16 code units, while the text is a UTF-8 `String`, so every call converts. This model holds
//! that logic without GPUI, so it is tested here; the `text_input` example wires it to a window. Decision 0010 measured
//! the sequences in the tests below through GPUI's macOS view (E2 to E6).
//!
//! Known limitation: movement and deletion are by Unicode scalar value, not by grapheme cluster. A letter with a
//! combining accent takes two backspaces, the caret can stop between the letter and its accent, and an emoji joined
//! with zero-width joiners is split. A real field needs grapheme-cluster segmentation, from a segmentation library or
//! the platform's text services; this crate adds no dependency, so the tests `backspace_removes_one_scalar_not_one_grapheme`
//! and `the_caret_can_land_inside_a_grapheme_cluster` pin the current behaviour.
//!
//! Known limitation: line breaks are not normalised. Committed text in a single-line field has `\n` turned into a
//! space, but composition text (`replace_and_mark_text_in_range`) is inserted as it is, and `\r` is never normalised
//! in either path. A real single-line field must reject or replace every line break, `\r` included.
//!
//! Written for the platform trial in its own words, with GPUI's `examples/input.rs` (Apache-2.0) read as reference.

use std::ops::Range;

/// A field's editable state. Offsets and ranges are UTF-8 byte offsets unless a name says UTF-16.
#[derive(Clone, Debug, Default)]
pub struct TextModel {
    text: String,
    selection: Range<usize>,
    marked: Option<Range<usize>>,
    multiline: bool,
}

/// The field's text as accessibility text runs: one per line, each keeping its line break, and the caret as
/// (line, character index in that line). Screen readers count characters and follow the caret only through text runs;
/// with just a value, VoiceOver read the text but reported zero characters (decision 0010, E21).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRuns {
    pub lines: Vec<String>,
    pub caret: (usize, usize),
}

impl TextModel {
    pub fn new(multiline: bool) -> Self {
        TextModel {
            multiline,
            ..TextModel::default()
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// The selection; empty means a caret.
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    pub fn caret(&self) -> usize {
        self.selection.end
    }

    /// The text an input method is composing, shown underlined until it is committed or cancelled.
    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    /// The UTF-8 offset of a UTF-16 offset. An offset inside a surrogate pair moves to the end of that character.
    pub fn from_utf16(&self, units: usize) -> usize {
        let mut seen = 0;
        for (index, ch) in self.text.char_indices() {
            if seen >= units {
                return index;
            }
            seen += ch.len_utf16();
        }
        self.text.len()
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.to_utf16(range.start)..self.to_utf16(range.end)
    }

    /// The UTF-8 range of a UTF-16 range, always in order: a range whose start is after its end is the same range
    /// with its ends swapped. Every range that enters the model passes through here, so the stored selection and
    /// marked range are never inverted and can be sliced by.
    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let (a, b) = (self.from_utf16(range.start), self.from_utf16(range.end));
        a.min(b)..a.max(b)
    }

    pub fn len_utf16(&self) -> usize {
        self.text.encode_utf16().count()
    }

    pub fn selection_utf16(&self) -> Range<usize> {
        self.range_to_utf16(&self.selection)
    }

    pub fn marked_utf16(&self) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| self.range_to_utf16(range))
    }

    /// The text of a UTF-16 range and the range actually used, after snapping to character boundaries.
    pub fn text_for_range_utf16(&self, range: Range<usize>) -> (String, Range<usize>) {
        let actual = self.range_from_utf16(&range);
        (
            self.text[actual.clone()].to_string(),
            self.range_to_utf16(&actual),
        )
    }

    /// What an edit replaces when the platform names no range: the marked text, or else the selection.
    fn target(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        let range = range_utf16
            .map(|range| self.range_from_utf16(&range))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        range.start.min(range.end)..range.start.max(range.end)
    }

    /// Committed text. Replaces `range` (UTF-16), or else the marked text, or else the selection, and ends any
    /// composition. A single-line field turns line breaks into spaces.
    pub fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, text: &str) {
        let text = if self.multiline {
            text.to_string()
        } else {
            text.replace('\n', " ")
        };
        let range = self.target(range_utf16);
        self.text.replace_range(range.clone(), &text);
        let caret = range.start + text.len();
        self.selection = caret..caret;
        self.marked = None;
    }

    /// A composition update. Replaces the same range as [`Self::replace_text_in_range`], marks the new text (an empty
    /// text ends the composition) and places the selection inside it; `selected_utf16` is relative to the new text.
    pub fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
    ) {
        let range = self.target(range_utf16);
        self.text.replace_range(range.clone(), text);
        self.marked = (!text.is_empty()).then(|| range.start..range.start + text.len());
        let start_utf16 = self.to_utf16(range.start);
        let inserted_utf16 = text.encode_utf16().count();
        self.selection = match selected_utf16 {
            Some(selected) => self.range_from_utf16(
                &(start_utf16 + selected.start.min(inserted_utf16)
                    ..start_utf16 + selected.end.min(inserted_utf16)),
            ),
            None => {
                let caret = range.start + text.len();
                caret..caret
            }
        };
    }

    /// Ends the composition and keeps its text.
    pub fn unmark_text(&mut self) {
        self.marked = None;
    }

    pub fn set_selection_utf16(&mut self, range: Range<usize>) {
        self.selection = self.range_from_utf16(&range);
    }

    fn previous_char(&self, offset: usize) -> usize {
        self.text[..offset]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index)
    }

    fn next_char(&self, offset: usize) -> usize {
        self.text[offset..]
            .chars()
            .next()
            .map_or(offset, |ch| offset + ch.len_utf8())
    }

    /// Deletes the selection, or the character before the caret.
    pub fn backspace(&mut self) {
        if self.selection.is_empty() {
            self.selection = self.previous_char(self.caret())..self.caret();
        }
        let range = self.range_to_utf16(&self.selection);
        self.replace_text_in_range(Some(range), "");
    }

    /// Moves the caret one character left, or to the start of a selection.
    pub fn move_left(&mut self) {
        let to = if self.selection.is_empty() {
            self.previous_char(self.caret())
        } else {
            self.selection.start
        };
        self.selection = to..to;
    }

    /// Moves the caret one character right, or to the end of a selection.
    pub fn move_right(&mut self) {
        let to = if self.selection.is_empty() {
            self.next_char(self.caret())
        } else {
            self.selection.end
        };
        self.selection = to..to;
    }

    /// Each line's start offset and text, for laying the field out.
    pub fn lines(&self) -> Vec<(usize, &str)> {
        let mut start = 0;
        self.text
            .split('\n')
            .map(|line| {
                let at = start;
                start += line.len() + 1;
                (at, line)
            })
            .collect()
    }

    /// A line's pieces as (length, inside the marked text), empty pieces left out: what the field underlines.
    pub fn marked_segments(&self, line_start: usize, line_len: usize) -> Vec<(usize, bool)> {
        let end = line_start + line_len;
        let (m0, m1) = match &self.marked {
            Some(m) if m.start < end && m.end > line_start => (
                m.start.clamp(line_start, end) - line_start,
                m.end.clamp(line_start, end) - line_start,
            ),
            _ => (line_len, line_len),
        };
        [(m0, false), (m1 - m0, true), (line_len - m1, false)]
            .into_iter()
            .filter(|(len, _)| *len > 0)
            .collect()
    }

    pub fn text_runs(&self) -> TextRuns {
        let lines = self.lines();
        let last = lines.len() - 1;
        let caret_offset = self.caret();
        let mut caret = (0, 0);
        let mut runs = Vec::with_capacity(lines.len());
        for (index, (start, line)) in lines.into_iter().enumerate() {
            if (start..=start + line.len()).contains(&caret_offset) {
                caret = (index, self.text[start..caret_offset].chars().count());
            }
            runs.push(if index == last {
                line.to_string()
            } else {
                format!("{line}\n")
            });
        }
        TextRuns { lines: runs, caret }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A range with its start after its end, built without a literal that clippy rejects: what a hostile or buggy
    /// platform caller could send.
    fn rev(start: usize, end: usize) -> Range<usize> {
        Range { start, end }
    }

    /// The state, after checking the model's invariant: the selection and the marked range are in order, inside the
    /// text and on character boundaries, so a copier can slice by them.
    fn state(m: &TextModel) -> (&str, Option<Range<usize>>, Range<usize>) {
        let ranges = [Some(m.selection()), m.marked()];
        for range in ranges.into_iter().flatten() {
            assert!(range.start <= range.end, "inverted range {range:?}");
            assert!(m.text().get(range.clone()).is_some(), "bad range {range:?}");
        }
        (m.text(), m.marked(), m.selection())
    }

    /// The sequence the platform trial drove through GPUI's macOS view and its test platform (decision 0010).
    fn compose_sequence(m: &mut TextModel) {
        m.replace_text_in_range(None, "a");
        m.replace_text_in_range(None, "b");
        assert_eq!(state(m), ("ab", None, 2..2));

        // Composition, as an input method sends it: begin, update twice, commit.
        m.replace_and_mark_text_in_range(None, "k", Some(1..1));
        assert_eq!(state(m), ("abk", Some(2..3), 3..3));
        m.replace_and_mark_text_in_range(None, "か", Some(1..1));
        assert_eq!(state(m), ("abか", Some(2..5), 5..5));
        m.replace_and_mark_text_in_range(None, "かん", Some(2..2));
        assert_eq!(state(m), ("abかん", Some(2..8), 8..8));
        // What `marked_text_range` answers, in UTF-16 units: the candidate window is placed from it.
        assert_eq!(m.marked_utf16(), Some(2..4));
        m.replace_text_in_range(None, "漢");
        assert_eq!(state(m), ("ab漢", None, 5..5));

        // A dead key: the accent is marked text until the next key commits the composed letter.
        m.replace_and_mark_text_in_range(None, "´", Some(1..1));
        assert_eq!(state(m), ("ab漢´", Some(5..7), 7..7));
        m.replace_text_in_range(None, "é");
        assert_eq!(state(m), ("ab漢é", None, 7..7));

        // A cancelled composition: the marked text is replaced by nothing, then the composition ends.
        m.replace_and_mark_text_in_range(None, "にほ", Some(2..2));
        assert_eq!(m.marked(), Some(7..13));
        m.replace_and_mark_text_in_range(None, "", Some(0..0));
        m.unmark_text();
        assert_eq!(state(m), ("ab漢é", None, 7..7));

        m.backspace();
        assert_eq!(state(m), ("ab漢", None, 5..5));
    }

    #[test]
    fn single_line_composition_dead_key_and_cancel() {
        let mut m = TextModel::new(false);
        compose_sequence(&mut m);
        // A single-line field turns a line break into a space.
        m.replace_text_in_range(None, "\n");
        assert_eq!(m.text(), "ab漢 ");
    }

    #[test]
    fn multi_line_composition_and_newline() {
        let mut m = TextModel::new(true);
        compose_sequence(&mut m);
        m.replace_text_in_range(None, "\n");
        m.replace_text_in_range(None, "x");
        assert_eq!(m.text(), "ab漢\nx");
        assert_eq!(m.lines(), vec![(0, "ab漢"), (6, "x")]);
    }

    #[test]
    fn utf16_offsets_count_surrogate_pairs() {
        let mut m = TextModel::new(false);
        m.replace_text_in_range(None, "a😀b");
        assert_eq!(m.len_utf16(), 4);
        assert_eq!(m.to_utf16(1), 1);
        assert_eq!(m.to_utf16(5), 3);
        assert_eq!(m.from_utf16(3), 5);
        // Inside the emoji's surrogate pair: snapped to the end of the character.
        assert_eq!(m.from_utf16(2), 5);
        assert_eq!(m.from_utf16(99), m.text().len());
        assert_eq!(m.text_for_range_utf16(1..3), ("😀".to_string(), 1..3));
        m.set_selection_utf16(1..3);
        assert_eq!(m.selection(), 1..5);
        assert_eq!(m.selection_utf16(), 1..3);
    }

    #[test]
    fn replacing_an_explicit_range_and_the_selection() {
        let mut m = TextModel::new(false);
        m.replace_text_in_range(None, "hello world");
        m.replace_text_in_range(Some(0..5), "HELLO");
        assert_eq!(state(&m), ("HELLO world", None, 5..5));
        m.set_selection_utf16(6..11);
        m.replace_text_in_range(None, "there");
        assert_eq!(state(&m), ("HELLO there", None, 11..11));
        // A selection given past the inserted text is clamped to it.
        m.replace_and_mark_text_in_range(None, "ab", Some(5..9));
        assert_eq!(state(&m), ("HELLO thereab", Some(11..13), 13..13));
    }

    #[test]
    fn caret_moves_by_characters_and_collapses_a_selection() {
        let mut m = TextModel::new(false);
        m.replace_text_in_range(None, "aé漢");
        m.move_left();
        assert_eq!(m.caret(), 3);
        m.move_left();
        assert_eq!(m.caret(), 1);
        m.move_left();
        m.move_left();
        assert_eq!(m.caret(), 0);
        m.move_right();
        m.move_right();
        assert_eq!(m.caret(), 3);
        m.set_selection_utf16(1..3);
        m.move_left();
        assert_eq!(m.selection(), 1..1);
        m.set_selection_utf16(0..2);
        m.move_right();
        assert_eq!(m.selection(), 3..3);
        m.set_selection_utf16(0..3);
        m.backspace();
        assert_eq!(state(&m), ("", None, 0..0));
        m.backspace();
        assert_eq!(state(&m), ("", None, 0..0));
    }

    #[test]
    fn marked_segments_split_a_line_where_the_composition_starts_and_ends() {
        let mut m = TextModel::new(true);
        m.replace_text_in_range(None, "ab\ncd");
        assert_eq!(m.marked_segments(0, 2), vec![(2, false)]);
        m.set_selection_utf16(1..1);
        m.replace_and_mark_text_in_range(None, "か", Some(1..1));
        // "aかb\ncd": the composition is bytes 1..4 of the first line and does not touch the second.
        assert_eq!(
            m.marked_segments(0, 5),
            vec![(1, false), (3, true), (1, false)]
        );
        assert_eq!(m.marked_segments(6, 2), vec![(2, false)]);
        assert_eq!(m.marked_segments(6, 0), vec![]);
    }

    #[test]
    fn text_runs_keep_line_breaks_and_place_the_caret() {
        let mut m = TextModel::new(true);
        assert_eq!(
            m.text_runs(),
            TextRuns {
                lines: vec![String::new()],
                caret: (0, 0)
            }
        );
        m.replace_text_in_range(None, "ab\ncdé");
        assert_eq!(
            m.text_runs(),
            TextRuns {
                lines: vec!["ab\n".to_string(), "cdé".to_string()],
                caret: (1, 3)
            }
        );
        m.set_selection_utf16(1..1);
        assert_eq!(m.text_runs().caret, (0, 1));
    }

    /// "a😀b" with the caret at the end: bytes a=0, the emoji 1..5, b=5..6; UTF-16 units 0, 1..3, 3.
    fn emoji_model() -> TextModel {
        let mut m = TextModel::new(false);
        m.replace_text_in_range(None, "a😀b");
        m
    }

    #[test]
    fn text_for_range_survives_hostile_ranges() {
        let m = emoji_model();
        // Out of range: clamped to the end.
        assert_eq!(m.text_for_range_utf16(2..99), ("b".to_string(), 3..4));
        // Entirely past the end: empty, at the end.
        assert_eq!(m.text_for_range_utf16(50..60), (String::new(), 4..4));
        // Inverted: the same range with its ends in order.
        assert_eq!(m.text_for_range_utf16(rev(3, 1)), ("😀".to_string(), 1..3));
        // Splitting a surrogate pair: both ends snap to the end of the character.
        assert_eq!(m.text_for_range_utf16(2..2), (String::new(), 3..3));
        assert_eq!(m.text_for_range_utf16(1..2), ("😀".to_string(), 1..3));
        assert_eq!(m.text_for_range_utf16(2..3), (String::new(), 3..3));
    }

    #[test]
    fn set_selection_orders_and_clamps_hostile_ranges() {
        let mut m = emoji_model();
        m.set_selection_utf16(rev(3, 1));
        assert_eq!(state(&m).2, 1..5);
        m.set_selection_utf16(rev(60, 50));
        assert_eq!(state(&m).2, 6..6);
        m.set_selection_utf16(2..99);
        assert_eq!(state(&m).2, 5..6);
        m.set_selection_utf16(2..2);
        assert_eq!(state(&m).2, 5..5);
        m.set_selection_utf16(rev(99, 0));
        assert_eq!(state(&m).2, 0..6);
    }

    #[test]
    fn replace_text_orders_and_clamps_hostile_ranges() {
        let mut m = emoji_model();
        // Inverted: replaces the same text as the ordered range would.
        m.replace_text_in_range(Some(rev(3, 1)), "X");
        assert_eq!(state(&m), ("aXb", None, 2..2));
        // Out of range: the end is clamped.
        m.replace_text_in_range(Some(1..99), "Y");
        assert_eq!(state(&m), ("aY", None, 2..2));
        // Entirely past the end: an insertion at the end.
        m.replace_text_in_range(Some(50..60), "Z");
        assert_eq!(state(&m), ("aYZ", None, 3..3));
        // Inverted and past the end.
        m.replace_text_in_range(Some(rev(60, 50)), "W");
        assert_eq!(state(&m), ("aYZW", None, 4..4));
        // Splitting a surrogate pair replaces the whole character.
        let mut m = emoji_model();
        m.replace_text_in_range(Some(1..2), "X");
        assert_eq!(state(&m), ("aXb", None, 2..2));
        let mut m = emoji_model();
        m.replace_text_in_range(Some(2..2), "X");
        assert_eq!(state(&m), ("a😀Xb", None, 6..6));
    }

    #[test]
    fn replace_and_mark_orders_and_clamps_hostile_ranges() {
        let mut m = emoji_model();
        m.replace_and_mark_text_in_range(Some(rev(3, 1)), "xy", Some(1..1));
        assert_eq!(state(&m), ("axyb", Some(1..3), 2..2));

        let mut m = emoji_model();
        m.replace_and_mark_text_in_range(Some(50..60), "xy", None);
        assert_eq!(state(&m), ("a😀bxy", Some(6..8), 8..8));

        let mut m = emoji_model();
        m.replace_and_mark_text_in_range(Some(1..99), "xy", None);
        assert_eq!(state(&m), ("axy", Some(1..3), 3..3));

        let mut m = emoji_model();
        m.replace_and_mark_text_in_range(Some(2..2), "xy", None);
        assert_eq!(state(&m), ("a😀xyb", Some(5..7), 7..7));

        // An inverted or out-of-range selection inside the new text is ordered and clamped to it.
        let mut m = emoji_model();
        m.replace_and_mark_text_in_range(Some(0..0), "abc", Some(rev(2, 0)));
        assert_eq!(state(&m), ("abca😀b", Some(0..3), 0..2));
        m.replace_and_mark_text_in_range(None, "abc", Some(rev(99, 40)));
        assert_eq!(state(&m), ("abca😀b", Some(0..3), 3..3));
        // An inverted selection that splits a surrogate pair of the new text.
        let mut m = TextModel::new(false);
        m.replace_and_mark_text_in_range(None, "😀", Some(rev(2, 1)));
        assert_eq!(state(&m), ("😀", Some(0..4), 4..4));
    }

    #[test]
    fn hostile_ranges_never_leave_the_model_inconsistent() {
        let sizes = [0usize, 1, 2, 3, 4, 5, 99];
        for &a in &sizes {
            for &b in &sizes {
                let mut m = emoji_model();
                m.set_selection_utf16(a..b);
                state(&m);
                m.replace_and_mark_text_in_range(Some(a..b), "é", Some(b..a));
                state(&m);
                m.replace_text_in_range(Some(b..a), "😀");
                state(&m);
                m.backspace();
                state(&m);
                let _ = m.text_for_range_utf16(a..b);
            }
        }
    }

    #[test]
    fn backspace_removes_one_scalar_not_one_grapheme() {
        // Known limitation: "e" plus U+0301 is one letter on screen and two scalar values, so it takes two
        // backspaces. A real field needs grapheme-cluster deletion from a segmentation library or the platform.
        let mut m = TextModel::new(false);
        m.replace_text_in_range(None, "e\u{301}");
        m.backspace();
        assert_eq!(state(&m), ("e", None, 1..1));
        m.backspace();
        assert_eq!(state(&m), ("", None, 0..0));

        // A joined emoji is split: the family sequence is several scalars joined by zero-width joiners.
        let mut m = TextModel::new(false);
        m.replace_text_in_range(None, "👨\u{200d}👩\u{200d}👧");
        m.backspace();
        assert_eq!(m.text(), "👨\u{200d}👩\u{200d}");
    }

    #[test]
    fn the_caret_can_land_inside_a_grapheme_cluster() {
        // Known limitation (see above): arrow movement is by scalar value, so the caret stops between a base
        // letter and its combining mark.
        let mut m = TextModel::new(false);
        m.replace_text_in_range(None, "e\u{301}x");
        m.move_left();
        m.move_left();
        assert_eq!(m.caret(), 1);
        assert_eq!(&m.text()[..m.caret()], "e");
        m.move_right();
        assert_eq!(m.caret(), 3);
    }
}
