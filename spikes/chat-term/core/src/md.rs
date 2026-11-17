//! A small markdown model for chat messages, and incremental parsing of a message that is still streaming.
//!
//! Parsing uses `pulldown-cmark`. The model is flat on purpose: a message is a list of blocks, and lists are flattened
//! into one block per item with its depth and marker, which is what a chat renderer needs.
//!
//! Incremental parsing ([`StreamDoc`]): the source is split at the last blank line outside a fenced code block. Everything
//! before that point is parsed once into "stable" blocks and never again; only the open tail is parsed again when text is
//! appended. The cost of a token is therefore the cost of parsing the current block, not the whole message. The split
//! is exact for the content this trial generates (paragraphs, tight lists, fenced code, tables, headings); a list item
//! whose continuation paragraph follows a blank line, or an indented code block containing blank lines, would be split
//! differently from a full parse. `headless-check` compares the two after every token and counts mismatches.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::time::Instant;

pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;
pub const CODE: u8 = 4;
pub const LINK: u8 = 8;
pub const STRIKE: u8 = 16;

/// A styled byte range of an [`Inline`]'s text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub style: u8,
    /// Index into [`Inline::links`] when `style` has [`LINK`].
    pub link: Option<u32>,
}

/// Inline text with style spans.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<Span>,
    pub links: Vec<String>,
}

impl Inline {
    fn push(&mut self, text: &str, style: u8, link: Option<&str>) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        if style == 0 {
            return;
        }
        let link_ix = link.map(|url| {
            if self.links.last().map(String::as_str) != Some(url) {
                self.links.push(url.to_string());
            }
            (self.links.len() - 1) as u32
        });
        if let Some(last) = self.spans.last_mut()
            && last.end == start
            && last.style == style
            && last.link == link_ix
        {
            last.end = end;
            return;
        }
        self.spans.push(Span {
            start,
            end,
            style,
            link: link_ix,
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading(u8, Inline),
    Paragraph(Inline),
    /// One list item. `marker` is "-" or "3." and so on.
    Item {
        depth: u8,
        marker: String,
        body: Inline,
    },
    Code {
        lang: String,
        text: String,
    },
    /// Rows of cells; the first row is the header.
    Table(Vec<Vec<Inline>>),
    Quote(Inline),
    Rule,
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    inline: Option<Inline>,
    bold: u32,
    italic: u32,
    strike: u32,
    link: Option<String>,
    heading: Option<u8>,
    lists: Vec<Option<u64>>,
    item_open: bool,
    item_depth: u8,
    item_marker: String,
    code: Option<(String, String)>,
    table: Option<Vec<Vec<Inline>>>,
    row: Option<Vec<Inline>>,
    quote: u32,
}

impl Builder {
    fn style(&self) -> u8 {
        let mut s = 0;
        if self.bold > 0 {
            s |= BOLD;
        }
        if self.italic > 0 {
            s |= ITALIC;
        }
        if self.strike > 0 {
            s |= STRIKE;
        }
        if self.link.is_some() {
            s |= LINK;
        }
        s
    }

    fn text(&mut self, text: &str, extra: u8) {
        if let Some((_, code)) = self.code.as_mut() {
            code.push_str(text);
            return;
        }
        let style = self.style() | extra;
        let link = self.link.clone();
        self.inline
            .get_or_insert_with(Inline::default)
            .push(text, style, link.as_deref());
    }

    fn flush_item(&mut self) {
        if self.item_open {
            let body = self.inline.take().unwrap_or_default();
            self.blocks.push(Block::Item {
                depth: self.item_depth,
                marker: std::mem::take(&mut self.item_marker),
                body,
            });
            self.item_open = false;
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    if self.item_open {
                        if let Some(inline) = self.inline.as_mut()
                            && !inline.text.is_empty()
                        {
                            inline.push(" ", 0, None);
                        }
                    } else {
                        self.inline = Some(Inline::default());
                    }
                }
                Tag::Heading { level, .. } => {
                    self.heading = Some(level as u8);
                    self.inline = Some(Inline::default());
                }
                Tag::List(start) => {
                    self.flush_item();
                    self.lists.push(start);
                }
                Tag::Item => {
                    self.flush_item();
                    let depth = self.lists.len().saturating_sub(1) as u8;
                    let marker = match self.lists.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}.");
                            *n += 1;
                            m
                        }
                        _ => "-".to_string(),
                    };
                    self.item_open = true;
                    self.item_depth = depth;
                    self.item_marker = marker;
                    self.inline = Some(Inline::default());
                }
                Tag::CodeBlock(kind) => {
                    let lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    self.code = Some((lang, String::new()));
                }
                Tag::Table(_) => self.table = Some(Vec::new()),
                Tag::TableHead | Tag::TableRow => self.row = Some(Vec::new()),
                Tag::TableCell => self.inline = Some(Inline::default()),
                Tag::Emphasis => self.italic += 1,
                Tag::Strong => self.bold += 1,
                Tag::Strikethrough => self.strike += 1,
                Tag::Link { dest_url, .. } => self.link = Some(dest_url.to_string()),
                Tag::BlockQuote(_) => self.quote += 1,
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    if !self.item_open {
                        let inline = self.inline.take().unwrap_or_default();
                        if self.quote > 0 {
                            self.blocks.push(Block::Quote(inline));
                        } else {
                            self.blocks.push(Block::Paragraph(inline));
                        }
                    }
                }
                TagEnd::Heading(_) => {
                    let inline = self.inline.take().unwrap_or_default();
                    self.blocks
                        .push(Block::Heading(self.heading.take().unwrap_or(1), inline));
                }
                TagEnd::Item => self.flush_item(),
                TagEnd::List(_) => {
                    self.flush_item();
                    self.lists.pop();
                }
                TagEnd::CodeBlock => {
                    if let Some((lang, mut text)) = self.code.take() {
                        if text.ends_with('\n') {
                            text.pop();
                        }
                        self.blocks.push(Block::Code { lang, text });
                    }
                }
                TagEnd::TableCell => {
                    let cell = self.inline.take().unwrap_or_default();
                    if let Some(row) = self.row.as_mut() {
                        row.push(cell);
                    }
                }
                TagEnd::TableHead | TagEnd::TableRow => {
                    if let (Some(row), Some(table)) = (self.row.take(), self.table.as_mut()) {
                        table.push(row);
                    }
                }
                TagEnd::Table => {
                    if let Some(table) = self.table.take() {
                        self.blocks.push(Block::Table(table));
                    }
                }
                TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
                TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
                TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
                TagEnd::Link => self.link = None,
                TagEnd::BlockQuote(_) => self.quote = self.quote.saturating_sub(1),
                _ => {}
            },
            Event::Text(t) => self.text(&t, 0),
            Event::Code(t) => self.text(&t, CODE),
            Event::Html(t) | Event::InlineHtml(t) => self.text(&t, 0),
            Event::SoftBreak => self.text(" ", 0),
            Event::HardBreak => self.text("\n", 0),
            Event::Rule => self.blocks.push(Block::Rule),
            _ => {}
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush_item();
        if let Some((lang, text)) = self.code.take() {
            self.blocks.push(Block::Code { lang, text });
        }
        self.blocks
    }
}

/// Parse a whole message from scratch.
pub fn parse(source: &str) -> Vec<Block> {
    let mut builder = Builder::default();
    for event in Parser::new_ext(source, options()) {
        builder.event(event);
    }
    builder.finish()
}

/// A message that is still growing, parsed incrementally.
#[derive(Default)]
pub struct StreamDoc {
    source: String,
    stable: Vec<Block>,
    stable_end: usize,
    tail: Vec<Block>,
    scan_pos: usize,
    fence: Option<(char, usize)>,
    last_boundary: usize,
    /// Bumped whenever `stable` grows, so a renderer can tell which blocks are new.
    pub stable_generation: u64,
    /// Nanoseconds the last `append` spent parsing.
    pub last_parse_ns: u64,
}

impl StreamDoc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn stable(&self) -> &[Block] {
        &self.stable
    }

    pub fn tail(&self) -> &[Block] {
        &self.tail
    }

    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.stable.iter().chain(self.tail.iter())
    }

    pub fn block_count(&self) -> usize {
        self.stable.len() + self.tail.len()
    }

    /// Append streamed text and update the parse. Only complete lines are scanned for fences and blank lines.
    pub fn append(&mut self, text: &str) {
        let started = Instant::now();
        self.source.push_str(text);
        while let Some(nl) = self.source[self.scan_pos..].find('\n') {
            let line_start = self.scan_pos;
            self.scan_pos += nl + 1;
            let line = &self.source[line_start..line_start + nl];
            let trimmed = line.trim_start();
            let fence_char = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            let fence_len = fence_char
                .map(|c| trimmed.chars().take_while(|x| *x == c).count())
                .unwrap_or(0);
            match (self.fence, fence_char) {
                (None, Some(c)) if fence_len >= 3 => self.fence = Some((c, fence_len)),
                (Some((c, len)), Some(c2))
                    if c == c2 && fence_len >= len && trimmed.trim_end().len() == fence_len =>
                {
                    self.fence = None
                }
                (None, _) if line.trim().is_empty() => self.last_boundary = self.scan_pos,
                _ => {}
            }
        }
        if self.last_boundary > self.stable_end {
            let blocks = parse(&self.source[self.stable_end..self.last_boundary]);
            self.stable.extend(blocks);
            self.stable_end = self.last_boundary;
            self.stable_generation += 1;
        }
        self.tail = parse(&self.source[self.stable_end..]);
        self.last_parse_ns = started.elapsed().as_nanos() as u64;
    }
}
