# Fixtures for the Electron baseline

`long-thread.mjs` writes the 5,000-message thread that `docs/baseline/electron-2.7.0.md` scrolls and opens, so the Rust
app can be given the same content.

Run it with a standard Node runtime (no packages needed), giving the output file:

```sh
node docs/baseline/fixtures/long-thread.mjs <scratch>/long-thread.jsonl
```

It is deterministic: seed 15015, and fixed timestamps, so every run writes the same bytes.

What it produces:

- 5,000 messages, as 2,500 turns of one prompt and one reply, in 10,002 JSON lines (a header line, a prompt, the agent's
  reply, a usage line and a done line per turn, and one session line in the first turn), 2,140,999 bytes in all.
- Prompts are 8 to 30 words (median 19). Replies are 1 to 3 paragraphs, 6 to 146 words (median 53) and 32 to 901
  characters (median 323, 95th percentile 628). About 30% of replies end with a three-item list, 15% name a file in inline
  code and 4% to 5% end with a fenced code block of 3 to 6 lines.
- All text is invented from a list of about 90 made-up placeholder words.
- The format is the Electron application's thread storage: one JSON object per line, with a `kind` field naming the line
  (`meta`, `prompt`, `session`, `text`, `usage`, `done`). The Rust app will need to convert it if its own thread storage
  differs.
