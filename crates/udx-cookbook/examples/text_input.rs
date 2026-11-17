//! Two text fields that implement GPUI's input handler: typed text, input-method composition with marked text, dead
//! keys, the candidate window placed at the marked text, Tab between fields, and what a screen reader needs
//! (decision 0010).
//!
//! Run it with `cargo run -p udx-cookbook --example text_input`. To see a composition, switch to a Japanese or Chinese
//! input source and type; for a dead key, press Option-E then E on a U.S. layout. The bottom line shows the Name
//! field's text and marked range as the platform edits them.
//!
//! The contract, as the platform trial found it:
//! - The platform never edits a field. On every OS it calls the field's `EntityInputHandler` with committed text,
//!   composition updates and the end of a composition, and asks about the text, in UTF-16 ranges.
//!   `udx_cookbook::text_model` does that editing and converting, and is unit-tested.
//! - While painting, the field registers its handler with `window.handle_input`; only the focused field's handler
//!   reaches the platform window.
//! - `bounds_for_range` places the candidate window, answering from the last painted layout. So after every edit the
//!   field calls `window.invalidate_character_coordinates()` and the platform asks again after the next frame. Asked
//!   before that frame, the trial got a zero-width rectangle for the old text (E3).
//! - A keystroke bound to an action (backspace, arrows, Enter in the multi-line field) never reaches the input
//!   handler; an unbound printable key does, as text.
//! - Tab moves only between tab stops: the handle the field tracks is made one with `.tab_stop(true)` (E10);
//!   `div().tab_stop(true)` affects only the handle a `focusable()` div makes for itself.
//! - Accessibility: an id, a role, a label and the value, plus one text run per line so a screen reader can count
//!   characters and follow the caret (E21).
//!
//! A real field also needs mouse selection, word movement, undo, wrapping and bidirectional text, and
//! grapheme-cluster movement and deletion (here backspace and the arrows work by scalar value, so a letter with a
//! combining accent takes two backspaces; that needs a segmentation library or the platform's text services).
//! Decision 0010 recommends the widget kit's input for these.

use std::ops::Range;

use gpui::{
    A11ySubtreeBuilder, App, Bounds, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, KeyBinding, Pixels, Point, Role, ShapedLine,
    SharedString, TextAlign, TextRun, TextStyle, TitlebarOptions, UTF16Selection, UnderlineStyle,
    Window, WindowBounds, WindowOptions, accesskit, actions, canvas, div, fill, point, prelude::*,
    px, rgb, size, text,
};
use udx_cookbook::text_model::{TextModel, TextRuns};

actions!(
    text_input,
    [Backspace, Left, Right, Newline, FocusNext, FocusPrev]
);

const LINE_HEIGHT: f32 = 22.;

struct TextField {
    focus: FocusHandle,
    label: SharedString,
    model: TextModel,
    /// Each line's start offset and shaped layout from the last paint: `bounds_for_range` answers from it.
    layout: Vec<(usize, ShapedLine)>,
    bounds: Option<Bounds<Pixels>>,
}

impl TextField {
    fn new(label: &'static str, multiline: bool, cx: &mut Context<Self>) -> Self {
        TextField {
            focus: cx.focus_handle().tab_stop(true),
            label: label.into(),
            model: TextModel::new(multiline),
            layout: Vec::new(),
            bounds: None,
        }
    }

    /// After every edit: the text changed, and character positions must be asked for again after the next frame.
    fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.notify_text_input_changed(&self.focus);
        window.invalidate_character_coordinates();
        cx.notify();
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
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (text, used) = self.model.text_for_range_utf16(range);
        *actual = Some(used);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let range = self.model.selection_utf16();
        Some(UTF16Selection {
            range,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.model.marked_utf16()
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.model.unmark_text();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model.replace_text_in_range(range, text);
        self.edited(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model
            .replace_and_mark_text_in_range(range, text, selected);
        self.edited(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        element: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // `range_from_utf16` puts the ends in order, so `end - start` below cannot underflow.
        let range = self.model.range_from_utf16(&range);
        let (index, (start, line)) =
            (self.layout.iter().enumerate()).rfind(|(_, (start, _))| *start <= range.start)?;
        let end = range.end.min(start + line.text.len());
        let top = element.top() + px(LINE_HEIGHT) * index as f32;
        let left = |offset: usize| element.left() + line.x_for_index(offset - start);
        Some(Bounds::from_corners(
            point(left(range.start), top),
            point(left(end), top + px(LINE_HEIGHT)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let local = self.bounds?.localize(&position)?;
        let index = ((local.y / px(LINE_HEIGHT)) as usize).min(self.layout.len().checked_sub(1)?);
        let (start, line) = &self.layout[index];
        Some(
            self.model
                .to_utf16(start + line.closest_index_for_x(local.x)),
        )
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model.set_selection_utf16(range);
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.model.len_utf16())
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let multiline = self.model.is_multiline();
        let focused = self.focus.is_focused(window);
        div()
            .id(ElementId::Name(self.label.clone()))
            .role(if multiline {
                Role::MultilineTextInput
            } else {
                Role::TextInput
            })
            .aria_label(self.label.clone())
            .aria_value(SharedString::from(self.model.text().to_string()))
            // GPUI builds the accessibility tree only after assistive technology has asked for it.
            .when(window.is_a11y_active(), |this| {
                let runs = self.model.text_runs();
                this.a11y_synthetic_children(move |builder| push_text_runs(builder, runs))
            })
            .track_focus(&self.focus)
            .key_context(if multiline {
                "TextField multiline"
            } else {
                "TextField"
            })
            .on_action(cx.listener(|this, _: &Backspace, window, cx| {
                this.model.backspace();
                this.edited(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Newline, window, cx| {
                this.model.replace_text_in_range(None, "\n");
                this.edited(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Left, _, cx| {
                this.model.move_left();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Right, _, cx| {
                this.model.move_right();
                cx.notify();
            }))
            .on_click(cx.listener(|this, _, window, cx| window.focus(&this.focus, cx)))
            .cursor(CursorStyle::IBeam)
            .w_full()
            .h(px(LINE_HEIGHT * if multiline { 4. } else { 1. } + 10.))
            .p(px(4.))
            .border_1()
            .border_color(rgb(if focused { 0x2563eb } else { 0x9ca3af }))
            .rounded_md()
            .bg(rgb(0xffffff))
            .text_size(px(16.))
            .child(text_canvas(cx.entity()))
    }
}

/// Shapes the lines in prepaint; in paint registers the input handler and draws the text and the caret.
fn text_canvas(field: Entity<TextField>) -> impl IntoElement {
    let painter = field.clone();
    canvas(
        move |_bounds, window, cx| {
            let model = &field.read(cx).model;
            let style = window.text_style();
            let font_size = style.font_size.to_pixels(window.rem_size());
            let shape = |(start, line): (usize, &str)| {
                let runs = runs_for_line(model, start, line, &style);
                let text = SharedString::from(line.to_string());
                (
                    start,
                    window
                        .text_system()
                        .shape_line(text, font_size, &runs, None),
                )
            };
            model.lines().into_iter().map(shape).collect::<Vec<_>>()
        },
        move |bounds, lines, window, cx| {
            let field = painter.read(cx);
            let (focus, caret) = (field.focus.clone(), field.model.caret());
            window.handle_input(
                &focus,
                ElementInputHandler::new(bounds, painter.clone()),
                cx,
            );
            for (index, (start, line)) in lines.iter().enumerate() {
                let origin = point(bounds.left(), bounds.top() + px(LINE_HEIGHT) * index as f32);
                line.paint(origin, px(LINE_HEIGHT), TextAlign::Left, None, window, cx)
                    .ok();
                if focus.is_focused(window) && (*start..=start + line.text.len()).contains(&caret) {
                    let at = point(origin.x + line.x_for_index(caret - start), origin.y);
                    let caret_box = Bounds::new(at, size(px(2.), px(LINE_HEIGHT)));
                    window.paint_quad(fill(caret_box, rgb(0x2563eb)));
                }
            }
            painter.update(cx, |field, _| {
                field.layout = lines;
                field.bounds = Some(bounds);
            });
        },
    )
    .size_full()
}

/// One line's runs, with the marked (composing) part underlined.
fn runs_for_line(model: &TextModel, start: usize, line: &str, style: &TextStyle) -> Vec<TextRun> {
    let underline = UnderlineStyle {
        color: Some(style.color),
        thickness: px(1.),
        wavy: false,
    };
    let run = |(len, marked): (usize, bool)| TextRun {
        len,
        font: style.font(),
        color: style.color,
        background_color: None,
        underline: marked.then_some(underline),
        strikethrough: None,
    };
    model
        .marked_segments(start, line.len())
        .into_iter()
        .map(run)
        .collect()
}

/// The field's text as AccessKit text runs and the caret as a text selection. The widget kit's `a11y_text.rs` does
/// the same more fully.
fn push_text_runs(builder: &mut A11ySubtreeBuilder, runs: TextRuns) {
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
        let selection = accesskit::TextSelection {
            anchor: caret,
            focus: caret,
        };
        builder.parent_node().set_text_selection(selection);
    }
}

struct Form {
    name: Entity<TextField>,
    notes: Entity<TextField>,
}

impl Render for Form {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = &self.name.read(cx).model;
        let state = format!("Name: {:?}, marked {:?}", name.text(), name.marked());
        div()
            .key_context("Form")
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(0xf3f4f6))
            .text_color(rgb(0x111827))
            .text_size(px(14.))
            .child(text!("Name (one line)"))
            .child(self.name.clone())
            .child(text!(
                "Notes (Enter starts a new line; Tab moves between the fields)"
            ))
            .child(self.notes.clone())
            .child(text!(state))
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, Some("TextField")),
            KeyBinding::new("left", Left, Some("TextField")),
            KeyBinding::new("right", Right, Some("TextField")),
            // Only the multi-line field binds Enter. In the other, Enter's line break reaches the input handler as
            // text, and the model turns it into a space.
            KeyBinding::new("enter", Newline, Some("TextField && multiline")),
            KeyBinding::new("tab", FocusNext, Some("Form")),
            KeyBinding::new("shift-tab", FocusPrev, Some("Form")),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(520.), px(360.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("UniDeX cookbook: text input".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            let form = cx.new(|cx| {
                let name = cx.new(|cx| TextField::new("Name", false, cx));
                let notes = cx.new(|cx| TextField::new("Notes", true, cx));
                // The bottom line shows the Name field's state, so the form re-renders when it changes.
                cx.observe(&name, |_, _, cx| cx.notify()).detach();
                Form { name, notes }
            });
            let focus = form.read(cx).name.read(cx).focus.clone();
            window.focus(&focus, cx);
            form
        });
        if let Err(error) = opened {
            eprintln!("text_input: could not open the window: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}
