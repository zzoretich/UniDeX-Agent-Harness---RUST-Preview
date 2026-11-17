//! Deterministic generation of a chat thread and of a streamed reply.
//!
//! Everything comes from a seeded SplitMix64 generator and a made-up vocabulary, so a run with the same seed always
//! produces the same 5,000 messages and the same token stream. Content mixes what a coding agent's thread holds:
//! paragraphs with inline code, emphasis and links, bullet and numbered lists, fenced code blocks of 5 to 80 lines, a few
//! tables, headings, and a sprinkling of emoji, CJK and right-to-left text and very long unbroken lines.

/// SplitMix64: small, fast and good enough for test content.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `lo..=hi`.
    pub fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next_u64() % (hi - lo + 1) as u64) as usize
    }

    /// True with probability `p` (0 to 1).
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0, items.len() - 1)]
    }
}

const WORDS: &[&str] = &[
    "agent",
    "branch",
    "buffer",
    "cache",
    "channel",
    "commit",
    "config",
    "crate",
    "cursor",
    "daemon",
    "delta",
    "diff",
    "engine",
    "event",
    "fixture",
    "frame",
    "handle",
    "index",
    "kernel",
    "layout",
    "ledger",
    "lock",
    "loop",
    "marker",
    "memory",
    "module",
    "parser",
    "patch",
    "pipeline",
    "probe",
    "queue",
    "render",
    "replay",
    "router",
    "runtime",
    "sandbox",
    "schema",
    "scope",
    "session",
    "signal",
    "socket",
    "stream",
    "struct",
    "task",
    "thread",
    "token",
    "trait",
    "vault",
    "vector",
    "widget",
    "window",
    "worker",
    "quietly",
    "rapidly",
    "carefully",
    "simply",
    "first",
    "then",
    "after",
    "before",
    "because",
    "while",
    "every",
    "each",
    "some",
    "the",
    "a",
    "an",
    "of",
    "to",
    "in",
    "on",
    "with",
    "for",
    "from",
    "into",
    "over",
    "under",
    "is",
    "was",
    "will",
    "can",
    "should",
    "must",
    "returns",
    "reads",
    "writes",
    "keeps",
    "drops",
    "moves",
    "checks",
    "builds",
    "tests",
    "draws",
    "waits",
    "sends",
    "parses",
    "measures",
    "splits",
];

const IDENTS: &[&str] = &[
    "ListState",
    "spawn_blocking",
    "remeasure_items",
    "Cargo.toml",
    "src/main.rs",
    "on_next_frame",
    "TokenStream",
    "parse_block",
    "FairMutex",
    "Term::resize",
    "cx.notify()",
    "WindowSize",
    "--release",
    "udx-core",
    "mpsc::channel",
    "Instant::now()",
    "README.md",
    "build.rs",
    "Vec<Block>",
    "Option<usize>",
];

const SPICE: &[&str] = &[
    "\u{1F680}",
    "\u{2705}",
    "\u{1F525}",
    "\u{1F468}\u{200D}\u{1F4BB}",
    "\u{4F60}\u{597D}\u{4E16}\u{754C}",
    "\u{65E5}\u{672C}\u{8A9E}\u{306E}\u{6587}",
    "\u{D55C}\u{AD6D}\u{C5B4}",
    "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627} \u{0628}\u{0627}\u{0644}\u{0639}\u{0627}\u{0644}\u{0645}",
    "\u{05E9}\u{05DC}\u{05D5}\u{05DD} \u{05E2}\u{05D5}\u{05DC}\u{05DD}",
    "caf\u{0065}\u{0301}",
];

const LANGS: &[&str] = &["rust", "python", "sh", "toml", "json", "typescript"];

/// One message of the thread.
#[derive(Clone, Debug)]
pub struct Message {
    pub from_user: bool,
    pub text: String,
}

fn sentence(rng: &mut Rng) -> String {
    let n = rng.range(6, 18);
    let mut out = String::new();
    for i in 0..n {
        if i > 0 {
            out.push(' ');
        }
        let roll = rng.range(0, 99);
        if roll < 6 {
            out.push('`');
            out.push_str(rng.pick(IDENTS));
            out.push('`');
        } else if roll < 9 {
            out.push_str("**");
            out.push_str(rng.pick(WORDS));
            out.push_str("**");
        } else if roll < 12 {
            out.push('*');
            out.push_str(rng.pick(WORDS));
            out.push('*');
        } else if roll < 13 {
            out.push_str("[the ");
            out.push_str(rng.pick(WORDS));
            out.push_str(" notes](https://example.com/docs/");
            out.push_str(rng.pick(WORDS));
            out.push(')');
        } else if roll < 14 {
            out.push_str(rng.pick(SPICE));
        } else {
            out.push_str(rng.pick(WORDS));
        }
    }
    let mut chars = out.chars();
    let mut capitalised: String = match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    };
    capitalised.push('.');
    capitalised
}

fn paragraph(rng: &mut Rng, sentences: usize) -> String {
    (0..sentences)
        .map(|_| sentence(rng))
        .collect::<Vec<_>>()
        .join(" ")
}

fn code_line(rng: &mut Rng, lang: &str, depth: usize) -> String {
    let indent = "    ".repeat(depth);
    let a = rng.pick(WORDS);
    let b = rng.pick(WORDS);
    let n = rng.range(0, 999);
    match lang {
        "rust" => match rng.range(0, 4) {
            0 => format!("{indent}let {a}_{b} = {a}.{b}({n});"),
            1 => format!("{indent}if {a}.is_empty() {{ return Err(Error::{b}); }}"),
            2 => format!(
                "{indent}// {} {} {}",
                rng.pick(WORDS),
                rng.pick(WORDS),
                rng.pick(WORDS)
            ),
            3 => format!("{indent}self.{a}.push({b}_{n}.clone());"),
            _ => format!("{indent}{a}::{b}(&mut cx, {n}).await?;"),
        },
        "python" => match rng.range(0, 3) {
            0 => format!("{indent}{a}_{b} = {a}.{b}({n})"),
            1 => format!("{indent}if not {a}: raise ValueError(\"{b}\")"),
            2 => format!("{indent}# {} {}", rng.pick(WORDS), rng.pick(WORDS)),
            _ => format!("{indent}return [{a} for {a} in {b} if {a} > {n}]"),
        },
        "sh" => match rng.range(0, 2) {
            0 => format!("cargo {a} --{b} -j {n}"),
            1 => format!("echo \"{a} {b}\" | grep -c {n}"),
            _ => format!("export {}={n}", a.to_uppercase()),
        },
        "toml" => format!(
            "{a}_{b} = {{ version = \"={}.{}.{n}\" }}",
            rng.range(0, 3),
            rng.range(0, 40)
        ),
        "json" => format!("{indent}\"{a}_{b}\": {n},"),
        _ => format!("{indent}const {a}{n} = await {b}(\"{a}\");"),
    }
}

fn code_block(rng: &mut Rng, lines: usize) -> String {
    let lang = *rng.pick(LANGS);
    let mut out = format!("```{lang}\n");
    let mut depth = 0usize;
    for i in 0..lines {
        if i % 7 == 3 && depth < 3 {
            depth += 1;
        } else if i % 5 == 0 && depth > 0 {
            depth -= 1;
        }
        let mut line = code_line(rng, lang, depth);
        if rng.chance(0.02) {
            // A very long line, to see how wrapping behaves.
            line.push_str(" // ");
            for _ in 0..40 {
                line.push_str(rng.pick(WORDS));
                line.push('_');
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("```");
    out
}

fn list(rng: &mut Rng) -> String {
    let ordered = rng.chance(0.4);
    let n = rng.range(2, 6);
    (0..n)
        .map(|i| {
            let marker = if ordered {
                format!("{}.", i + 1)
            } else {
                "-".to_string()
            };
            format!("{marker} {}", sentence(rng))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn table(rng: &mut Rng) -> String {
    let cols = rng.range(2, 5);
    let rows = rng.range(2, 8);
    let mut out = String::new();
    let header: Vec<String> = (0..cols).map(|_| rng.pick(WORDS).to_string()).collect();
    out.push_str(&format!("| {} |\n", header.join(" | ")));
    out.push_str(&format!("|{}\n", "---|".repeat(cols)));
    for _ in 0..rows {
        let cells: Vec<String> = (0..cols)
            .map(|c| {
                if c == 0 {
                    format!("`{}`", rng.pick(IDENTS))
                } else {
                    format!("{}", rng.range(0, 99_999))
                }
            })
            .collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out.pop();
    out
}

fn reply(rng: &mut Rng) -> String {
    let mut blocks = Vec::new();
    if rng.chance(0.15) {
        blocks.push(format!("## {}", sentence(rng).trim_end_matches('.')));
    }
    let n = rng.range(1, 4);
    for _ in 0..n {
        let n = rng.range(1, 4);
        blocks.push(paragraph(rng, n));
        let roll = rng.range(0, 99);
        if roll < 30 {
            blocks.push(list(rng));
        } else if roll < 52 {
            let lines = if rng.chance(0.15) {
                rng.range(30, 80)
            } else {
                rng.range(5, 20)
            };
            blocks.push(code_block(rng, lines));
        } else if roll < 55 {
            blocks.push(table(rng));
        }
    }
    if rng.chance(0.03) {
        // A long unbroken token: a URL with no spaces.
        let mut url = String::from("https://example.com/");
        for _ in 0..30 {
            url.push_str(rng.pick(WORDS));
            url.push('/');
        }
        blocks.push(url);
    }
    blocks.join("\n\n")
}

/// The whole thread: `count` messages alternating a user prompt and an agent reply.
pub fn thread(seed: u64, count: usize) -> Vec<Message> {
    let mut rng = Rng::new(seed);
    (0..count)
        .map(|i| {
            if i % 2 == 0 {
                let n = rng.range(1, 2);
                Message {
                    from_user: true,
                    text: paragraph(&mut rng, n),
                }
            } else {
                Message {
                    from_user: false,
                    text: reply(&mut rng),
                }
            }
        })
        .collect()
}

/// A long reply split into tokens of 2 to 6 characters (on character boundaries), for the stream.
/// Each round of the reply is a few paragraphs, a code block, a list and sometimes a table.
pub fn stream_tokens(seed: u64, min_chars: usize) -> Vec<String> {
    let mut rng = Rng::new(seed ^ 0x5eed_5eed);
    let mut text = String::new();
    while text.len() < min_chars {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&paragraph(&mut rng, 3));
        text.push_str("\n\n");
        let lines = rng.range(8, 40);
        text.push_str(&code_block(&mut rng, lines));
        text.push_str("\n\n");
        text.push_str(&list(&mut rng));
        if rng.chance(0.3) {
            text.push_str("\n\n");
            text.push_str(&table(&mut rng));
        }
    }
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let n = rng.range(2, 6).min(chars.len() - i);
        tokens.push(chars[i..i + n].iter().collect());
        i += n;
    }
    tokens
}
