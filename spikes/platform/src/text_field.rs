//! A minimal text field, single-line or multi-line, written for this trial.
//!
//! What a GPUI text field has to provide, as this trial found it:
//! - An entity that implements `EntityInputHandler`. The platform layer never edits the text itself: on every OS it
//!   calls this trait (through `ElementInputHandler` and GPUI's `PlatformInputHandler`) with committed text
//!   (`replace_text_in_range`), composition updates (`replace_and_mark_text_in_range`), the end of a composition
//!   (`unmark_text`), and questions about the text (`marked_text_range`, `selected_text_range`, `text_for_range`).
//!   Ranges are in UTF-16 code units, so the field converts to and from its UTF-8 string.
//! - `bounds_for_range`, which is how the OS places the input method's candidate window next to the marked text.
//!   It answers from the last painted layout, so after every edit the field calls
//!   `window.invalidate_character_coordinates()`, which makes the platform ask again after the next frame (on macOS
//!   `NSTextInputContext.invalidateCharacterCoordinates`). Asked before that frame, it answers for the old text.
//! - An element that, while painting, registers the handler with `window.handle_input(focus, handler)`. Only the
//!   focused field's handler reaches the platform window, at the end of the frame.
//! - Actions for editing keys (backspace, arrows, enter). A keystroke bound to an action never reaches the input
//!   handler; an unbound printable keystroke does, as text.
//! - Accessibility, which nothing provides by default: an id, a role (`TextInput` or `MultilineTextInput`), a label
//!   and the current value, on an element that tracks the focus handle.
//!
//! Written in this trial's own words; GPUI's `examples/input.rs` (Apache-2.0) was read as reference.

use std::ops::Range;

use gpui::{
    App, Bounds, Context, CursorStyle, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, InspectorElementId, KeyBinding,
    LayoutId, PaintQuad, Pixels, Point, Role, ShapedLine, SharedString, Style, TextRun,
    UTF16Selection, UnderlineStyle, Window, actions, div, fill, point, prelude::*, px, relative,
    rgb, size,
};

actions!(
    trial_text_field,
    [
        Backspace, Delete, Left, Right, Home, End, Newline, SelectAll
    ]
);

/// Key context of every field; the multi-line one adds `multiline`.
pub const KEY_CONTEXT: &str = "TrialTextField";

/// Bindings the fields need. `enter` inserts a newline only in the multi-line field.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", Delete, Some(KEY_CONTEXT)),
        KeyBinding::new("left", Left, Some(KEY_CONTEXT)),
        KeyBinding::new("right", Right, Some(KEY_CONTEXT)),
        KeyBinding::new("home", Home, Some(KEY_CONTEXT)),
        KeyBinding::new("end", End, Some(KEY_CONTEXT)),
        KeyBinding::new(crate::sec!("a"), SelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", Newline, Some("TrialTextField && multiline")),
    ]);
}

pub struct TextField {
    pub focus: FocusHandle,
    pub id: SharedString,
    pub label: SharedString,
    pub multiline: bool,
    /// The text, UTF-8.
    pub text: String,
    /// The selection, as UTF-8 byte offsets; empty means a caret.
    pub selection: Range<usize>,
    /// The marked (composing) text, as UTF-8 byte offsets.
    pub marked: Option<Range<usize>>,
    /// One entry per line: its start offset and its shaped layout, from the last paint.
    layout: Vec<(usize, ShapedLine)>,
    last_bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
    /// Every call the input handler received, newest last; shown in the window and read by the self-check.
    pub input_log: Vec<String>,
}

impl TextField {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        multiline: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            // A tracked focus handle carries its own tab-stop flag; `div().tab_stop(true)` only affects the handle a
            // `focusable()` div creates for itself.
            focus: cx.focus_handle().tab_stop(true),
            id: id.into(),
            label: label.into(),
            multiline,
            text: String::new(),
            selection: 0..0,
            marked: None,
            layout: Vec::new(),
            last_bounds: None,
            line_height: px(22.),
            input_log: Vec::new(),
        }
    }

    pub fn last_bounds(&self) -> Option<Bounds<Pixels>> {
        self.last_bounds
    }

    fn caret(&self) -> usize {
        self.selection.end
    }

    fn log(&mut self, entry: String) {
        self.input_log.push(entry);
        if self.input_log.len() > 40 {
            self.input_log.remove(0);
        }
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut units = 0;
        for (index, ch) in self.text.char_indices() {
            if units >= offset {
                return index;
            }
            units += ch.len_utf16();
        }
        self.text.len()
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn previous_char(&self, offset: usize) -> usize {
        self.text[..offset]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next_char(&self, offset: usize) -> usize {
        self.text[offset..]
            .chars()
            .next()
            .map_or(offset, |c| offset + c.len_utf8())
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selection = offset..offset;
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            let start = self.previous_char(self.caret());
            self.selection = start..self.caret();
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            let end = self.next_char(self.caret());
            self.selection = self.caret()..end;
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let to = if self.selection.is_empty() {
            self.previous_char(self.caret())
        } else {
            self.selection.start
        };
        self.move_to(to, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let to = if self.selection.is_empty() {
            self.next_char(self.caret())
        } else {
            self.selection.end
        };
        self.move_to(to, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selection = 0..self.text.len();
        cx.notify();
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.log("action newline".into());
        self.replace_text_in_range(None, "\n", window, cx);
    }

    /// Line index and line start for a UTF-8 offset, from the last layout.
    fn line_for_offset(&self, offset: usize) -> Option<(usize, usize, &ShapedLine)> {
        let mut found = None;
        for (index, (start, line)) in self.layout.iter().enumerate() {
            if *start <= offset {
                found = Some((index, *start, line));
            }
        }
        found
    }
}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selection),
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.log("unmark_text".into());
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.log(format!("replace_text_in_range({range_utf16:?}, {text:?})"));
        let text = if self.multiline {
            text.to_string()
        } else {
            text.replace('\n', " ")
        };
        let range = range_utf16
            .map(|r| self.range_from_utf16(&r))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        self.text.replace_range(range.clone(), &text);
        let caret = range.start + text.len();
        self.selection = caret..caret;
        self.marked = None;
        window.notify_text_input_changed(&self.focus);
        // The text moved: after the next frame, the input method asks again where the candidate window goes.
        window.invalidate_character_coordinates();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.log(format!(
            "replace_and_mark_text_in_range({range_utf16:?}, {new_text:?}, {new_selected_range_utf16:?})"
        ));
        let range = range_utf16
            .map(|r| self.range_from_utf16(&r))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        self.text.replace_range(range.clone(), new_text);
        self.marked = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        // The new selection is relative to the inserted text, in UTF-16 units.
        let start_utf16 = self.offset_to_utf16(range.start);
        let inserted_utf16 = new_text.encode_utf16().count();
        self.selection = match new_selected_range_utf16 {
            Some(selected) => self.range_from_utf16(
                &(start_utf16 + selected.start.min(inserted_utf16)
                    ..start_utf16 + selected.end.min(inserted_utf16)),
            ),
            None => {
                let caret = range.start + new_text.len();
                caret..caret
            }
        };
        window.notify_text_input_changed(&self.focus);
        window.invalidate_character_coordinates();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let line_height = self.line_height;
        let (index, start, line) = self.line_for_offset(range.start)?;
        let line_end = start + line.text.len();
        let x0 = line.x_for_index(range.start - start);
        let x1 = line.x_for_index(range.end.min(line_end) - start);
        let top = element_bounds.top() + line_height * index as f32;
        Some(Bounds::from_corners(
            point(element_bounds.left() + x0, top),
            point(element_bounds.left() + x1, top + line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let local = bounds.localize(&position)?;
        let index = ((local.y / self.line_height) as usize).min(self.layout.len().checked_sub(1)?);
        let (start, line) = &self.layout[index];
        let offset = start + line.closest_index_for_x(local.x);
        Some(self.offset_to_utf16(offset))
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection = self.range_from_utf16(&range_utf16);
        cx.notify();
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.text.encode_utf16().count())
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        let lines = if self.multiline { 4.0 } else { 1.0 };
        let role = if self.multiline {
            Role::MultilineTextInput
        } else {
            Role::TextInput
        };
        div()
            .id(ElementId::Name(self.id.clone()))
            .role(role)
            .aria_label(self.label.clone())
            .aria_value(SharedString::from(self.text.clone()))
            .when(window.is_a11y_active(), |this| {
                let runs = text_runs(&self.text, self.selection.end);
                this.a11y_synthetic_children(move |builder| push_text_runs(builder, runs))
            })
            .track_focus(&self.focus)
            .key_context(if self.multiline {
                "TrialTextField multiline"
            } else {
                KEY_CONTEXT
            })
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::newline))
            .on_click(cx.listener(|this, _, window, cx| {
                window.focus(&this.focus, cx);
            }))
            .w_full()
            .h(self.line_height * lines + px(8.))
            .p(px(4.))
            .border_1()
            .border_color(if focused {
                rgb(0x2563eb)
            } else {
                rgb(0x9ca3af)
            })
            .rounded_md()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111827))
            .line_height(self.line_height)
            .text_size(px(16.))
            .child(TextFieldElement { field: cx.entity() })
    }
}

/// The field's text as AccessKit text runs, one per line (a line keeps its line break), and the caret as
/// (line, character index). AccessKit answers a screen reader's character count, caret and text-range questions
/// only for a node with `TextRun` children; with just a value, VoiceOver reads the text but reports zero characters
/// (the self-check shows both). Written for this trial; the widget kit's `a11y_text.rs` does the same, more fully.
struct TextRuns {
    lines: Vec<String>,
    caret: (usize, usize),
}

fn text_runs(text: &str, caret: usize) -> TextRuns {
    let mut lines = Vec::new();
    let mut caret_at = (0, 0);
    let mut start = 0;
    let pieces: Vec<&str> = text.split('\n').collect();
    for (index, piece) in pieces.iter().enumerate() {
        let last = index + 1 == pieces.len();
        let end = start + piece.len();
        if caret >= start && caret <= end {
            caret_at = (index, text[start..caret].chars().count());
        }
        lines.push(if last {
            piece.to_string()
        } else {
            format!("{piece}\n")
        });
        start = end + 1;
    }
    TextRuns {
        lines,
        caret: caret_at,
    }
}

fn push_text_runs(builder: &mut gpui::A11ySubtreeBuilder, runs: TextRuns) {
    use gpui::accesskit;
    let ids: Vec<_> = (0..runs.lines.len())
        .map(|index| builder.synthetic_node_id(("text-run", index)))
        .collect();
    for (id, line) in ids.iter().zip(&runs.lines) {
        let mut node = accesskit::Node::new(accesskit::Role::TextRun);
        node.set_value(line.clone());
        node.set_character_lengths(line.chars().map(|c| c.len_utf8() as u8).collect::<Vec<_>>());
        builder.push_child(*id, node);
    }
    if let Some(node) = ids.get(runs.caret.0) {
        let caret = accesskit::TextPosition {
            node: *node,
            character_index: runs.caret.1,
        };
        builder
            .parent_node()
            .set_text_selection(accesskit::TextSelection {
                anchor: caret,
                focus: caret,
            });
    }
}

/// The element that lays out and paints the text and registers the input handler.
struct TextFieldElement {
    field: Entity<TextField>,
}

struct Prepainted {
    lines: Vec<(usize, ShapedLine)>,
    caret: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextFieldElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextFieldElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepainted;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepainted {
        let field = self.field.read(cx);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = field.line_height;
        let base = TextRun {
            len: 0,
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let mut lines = Vec::new();
        let mut start = 0;
        for text in field.text.split('\n') {
            let end = start + text.len();
            // Underline the part of the marked range that falls on this line.
            let mut runs = Vec::new();
            match &field.marked {
                Some(marked) if marked.start < end && marked.end > start => {
                    let m0 = marked.start.clamp(start, end) - start;
                    let m1 = marked.end.clamp(start, end) - start;
                    for (len, underline) in [(m0, false), (m1 - m0, true), (text.len() - m1, false)]
                    {
                        if len > 0 {
                            runs.push(TextRun {
                                len,
                                underline: underline.then_some(UnderlineStyle {
                                    color: Some(style.color),
                                    thickness: px(1.),
                                    wavy: false,
                                }),
                                ..base.clone()
                            });
                        }
                    }
                }
                _ => {
                    if !text.is_empty() {
                        runs.push(TextRun {
                            len: text.len(),
                            ..base.clone()
                        });
                    }
                }
            }
            let shaped = window.text_system().shape_line(
                SharedString::from(text.to_string()),
                font_size,
                &runs,
                None,
            );
            lines.push((start, shaped));
            start = end + 1;
        }

        let caret_offset = field.selection.end;
        let mut caret = None;
        let mut selection = Vec::new();
        for (index, (line_start, line)) in lines.iter().enumerate() {
            let line_end = line_start + line.text.len();
            let top = bounds.top() + line_height * index as f32;
            if field.selection.is_empty() {
                if caret_offset >= *line_start && caret_offset <= line_end {
                    caret = Some(fill(
                        Bounds::new(
                            point(
                                bounds.left() + line.x_for_index(caret_offset - line_start),
                                top,
                            ),
                            size(px(2.), line_height),
                        ),
                        rgb(0x2563eb),
                    ));
                }
            } else {
                let s = field.selection.start.max(*line_start);
                let e = field.selection.end.min(line_end);
                if s <= e && field.selection.start <= line_end && field.selection.end >= *line_start
                {
                    selection.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + line.x_for_index(s - line_start), top),
                            point(
                                bounds.left() + line.x_for_index(e - line_start),
                                top + line_height,
                            ),
                        ),
                        rgb(0xbfdbfe),
                    ));
                }
            }
        }
        Prepainted {
            lines,
            caret,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        prepainted: &mut Prepainted,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.field.read(cx).focus.clone();
        // This is the whole contract with the platform layer: the focused field's handler is handed to the
        // platform window at the end of the frame, and every OS input path calls into it.
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.field.clone()),
            cx,
        );
        for quad in prepainted.selection.drain(..) {
            window.paint_quad(quad);
        }
        let line_height = self.field.read(cx).line_height;
        for (index, (_, line)) in prepainted.lines.iter().enumerate() {
            let origin = point(bounds.left(), bounds.top() + line_height * index as f32);
            line.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                .ok();
        }
        if focus.is_focused(window)
            && let Some(caret) = prepainted.caret.take()
        {
            window.paint_quad(caret);
        }
        let lines = std::mem::take(&mut prepainted.lines);
        self.field.update(cx, |field, _| {
            field.layout = lines;
            field.last_bounds = Some(bounds);
        });
    }
}

#[cfg(test)]
mod tests {
    //! Headless: GPUI's test platform, no display. These run on every CI runner and drive the same entry points the
    //! platform layers call: `window.dispatch_keystroke` (key event, then the registered platform input handler)
    //! and the `InputHandler` trait that GPUI's `PlatformInputHandler` wraps.

    use super::*;
    use gpui::{InputHandler, Keystroke, TestAppContext, VisualTestContext};

    struct Host {
        single: Entity<TextField>,
        multi: Entity<TextField>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(self.single.clone())
                .child(self.multi.clone())
        }
    }

    fn setup(cx: &mut TestAppContext) -> (Entity<Host>, &mut VisualTestContext) {
        cx.update(bind_keys);
        let (host, vcx) = cx.add_window_view(|_, cx| Host {
            single: cx.new(|cx| TextField::new("single", "Name", false, cx)),
            multi: cx.new(|cx| TextField::new("multi", "Notes", true, cx)),
        });
        (host, vcx)
    }

    /// Focus one field, draw a frame (so its handler is registered), and return an input handler over it.
    fn focus(
        field: &Entity<TextField>,
        vcx: &mut VisualTestContext,
    ) -> ElementInputHandler<TextField> {
        let f = field.clone();
        vcx.update(|window, cx| window.focus(&f.read(cx).focus.clone(), cx));
        vcx.run_until_parked();
        let bounds = field.read_with(vcx, |f, _| f.last_bounds().expect("painted"));
        ElementInputHandler::new(bounds, field.clone())
    }

    fn state(
        field: &Entity<TextField>,
        vcx: &mut VisualTestContext,
    ) -> (String, Option<Range<usize>>, Range<usize>) {
        field.read_with(vcx, |f, _| {
            (f.text.clone(), f.marked.clone(), f.selection.clone())
        })
    }

    fn compose_sequence(field: &Entity<TextField>, vcx: &mut VisualTestContext) {
        // Committed text through the real key path: the keystroke is dispatched, nothing binds it, so GPUI hands
        // its character to the platform window's registered input handler.
        let mut handler = focus(field, vcx);
        for key in ["a", "b"] {
            vcx.update(|window, cx| {
                assert!(window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx));
            });
        }
        assert_eq!(state(field, vcx).0, "ab");

        // Composition, as an input method sends it: begin, update twice, commit.
        vcx.update(|window, cx| {
            handler.replace_and_mark_text_in_range(None, "k", Some(1..1), window, cx);
        });
        assert_eq!(state(field, vcx), ("abk".into(), Some(2..3), 3..3));
        vcx.update(|window, cx| {
            handler.replace_and_mark_text_in_range(None, "か", Some(1..1), window, cx);
        });
        assert_eq!(state(field, vcx), ("abか".into(), Some(2..5), 5..5));
        vcx.update(|window, cx| {
            handler.replace_and_mark_text_in_range(None, "かん", Some(2..2), window, cx);
        });
        assert_eq!(state(field, vcx), ("abかん".into(), Some(2..8), 8..8));
        // The candidate window is placed from the marked range's bounds.
        let bounds = vcx.update(|window, cx| {
            let marked = handler.marked_text_range(window, cx).expect("marked");
            assert_eq!(marked, 2..4);
            handler.bounds_for_range(marked, window, cx)
        });
        let bounds = bounds.expect("bounds for the marked range");
        assert!(bounds.size.width > px(0.), "marked text has width");
        vcx.update(|window, cx| handler.replace_text_in_range(None, "漢", window, cx));
        assert_eq!(state(field, vcx), ("ab漢".into(), None, 5..5));

        // A dead key: the accent is shown as marked text, then the next key commits the composed letter.
        vcx.update(|window, cx| {
            handler.replace_and_mark_text_in_range(None, "´", Some(1..1), window, cx);
        });
        assert_eq!(state(field, vcx), ("ab漢´".into(), Some(5..7), 7..7));
        vcx.update(|window, cx| handler.replace_text_in_range(None, "é", window, cx));
        assert_eq!(state(field, vcx), ("ab漢é".into(), None, 7..7));

        // A cancelled composition: the marked text is replaced by nothing and the composition ends.
        vcx.update(|window, cx| {
            handler.replace_and_mark_text_in_range(None, "にほ", Some(2..2), window, cx);
        });
        assert_eq!(state(field, vcx).1, Some(7..13));
        vcx.update(|window, cx| {
            handler.replace_and_mark_text_in_range(None, "", Some(0..0), window, cx);
            handler.unmark_text(window, cx);
        });
        assert_eq!(state(field, vcx), ("ab漢é".into(), None, 7..7));

        // Key events versus text: backspace is bound to an action, so it edits and inserts nothing.
        vcx.update(|window, cx| {
            assert!(window.dispatch_keystroke(Keystroke::parse("backspace").unwrap(), cx));
        });
        assert_eq!(state(field, vcx).0, "ab漢");
    }

    #[gpui::test]
    fn single_line_composition_dead_key_and_cancel(cx: &mut TestAppContext) {
        let (host, vcx) = setup(cx);
        let single = host.read_with(vcx, |h, _| h.single.clone());
        compose_sequence(&single, vcx);
        // Enter is not bound in the single-line field; its newline character is turned into a space.
        vcx.update(|window, cx| {
            window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
        });
        assert_eq!(state(&single, vcx).0, "ab漢 ");
    }

    #[gpui::test]
    fn multi_line_composition_and_newline(cx: &mut TestAppContext) {
        let (host, vcx) = setup(cx);
        let multi = host.read_with(vcx, |h, _| h.multi.clone());
        compose_sequence(&multi, vcx);
        vcx.update(|window, cx| {
            assert!(window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx));
        });
        vcx.update(|window, cx| {
            window.dispatch_keystroke(Keystroke::parse("x").unwrap(), cx);
        });
        vcx.run_until_parked();
        assert_eq!(state(&multi, vcx).0, "ab漢\nx");
        // The second line is laid out below the first: the candidate window follows the caret to it.
        let mut handler = focus(&multi, vcx);
        let (first, second) = vcx.update(|window, cx| {
            (
                handler.bounds_for_range(0..1, window, cx).unwrap(),
                handler.bounds_for_range(4..5, window, cx).unwrap(),
            )
        });
        assert!(second.top() > first.top());
    }
}
