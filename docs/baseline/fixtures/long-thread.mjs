// Generates the 5,000-message thread used in the Electron 2.7.0 baseline (docs/baseline/electron-2.7.0.md).
// Usage: node long-thread.mjs <output-file>
// Deterministic: the same seed gives the same bytes. All text is invented from a small made-up word list.
import fs from "node:fs";
import path from "node:path";

const SEED = 15015;
const MESSAGES = 5000;
const THREAD = "5b0c0000-0000-4000-8000-000000005000";
const SESSION = "5b0c0000-0000-4000-8000-000000000001";
const WORKSPACE = "harbor-map";
const END = Date.UTC(2026, 10, 17, 0, 0, 0); // fixed, so the timestamps do not change between runs

const out = process.argv[2];
if (!out) { console.error("usage: node long-thread.mjs <output-file>"); process.exit(2); }

// mulberry32, a small seeded generator
function rng(seed) {
  return () => {
    seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const WORDS = ("tide chart harbour buoy kelp reef survey depth sounding current swell pier mooring channel beacon gauge reading " +
  "station log entry morning evening north south east west shallow deep water level rise fall table parser tile zoom map " +
  "layer cache sample record field note check update fix test merge branch file line value range report week day hour").split(" ");

const r = rng(SEED);
const pick = () => WORDS[Math.floor(r() * WORDS.length)];
const sentence = (min, max) => {
  const n = min + Math.floor(r() * (max - min + 1));
  const w = Array.from({ length: n }, pick);
  w[0] = w[0][0].toUpperCase() + w[0].slice(1);
  return `${w.join(" ")}.`;
};
const para = () => Array.from({ length: 1 + Math.floor(r() * 3) }, () => sentence(6, 18)).join(" ");
const reply = () => {
  const parts = Array.from({ length: 1 + Math.floor(r() * 3) }, para);
  if (r() < 0.3) parts.push(Array.from({ length: 3 }, () => `- ${sentence(3, 8)}`).join("\n"));
  if (r() < 0.15) parts[0] += ` See \`${pick()}_${pick()}.js\`.`;
  if (r() < 0.05) parts.push("```js\n" + Array.from({ length: 3 + Math.floor(r() * 4) }, () => `const ${pick()} = read("${pick()}");`).join("\n") + "\n```");
  return parts.join("\n\n");
};

const turns = MESSAGES / 2;
const iso = (k) => new Date(END - (turns - k) * 60_000).toISOString();
const lines = [JSON.stringify({ schema: 1, kind: "meta", thread: THREAD, workspace: WORKSPACE, host: "claude", model: null, effort: null, access: "edit", title: "Baseline: a 5,000-message thread", created: iso(0) })];
for (let n = 1; n <= turns; n++) {
  lines.push(JSON.stringify({ t: iso(n), kind: "prompt", text: sentence(8, 30), model: null, effort: null, access: "edit", turn: n }));
  if (n === 1) lines.push(JSON.stringify({ t: iso(n), kind: "session", id: SESSION, turn: n }));
  lines.push(JSON.stringify({ t: iso(n), kind: "text", text: reply(), turn: n }));
  lines.push(JSON.stringify({ t: iso(n), kind: "usage", in: 1200, out: 80, cached: 0, usd: 0.002, estimated: false, turn: n }));
  lines.push(JSON.stringify({ t: iso(n), kind: "done", ok: true, usd: 0.002, estimated: false, turn: n }));
}
fs.mkdirSync(path.dirname(path.resolve(out)), { recursive: true });
fs.writeFileSync(out, `${lines.join("\n")}\n`);
console.log(`${out}: ${lines.length} records, ${turns * 2} messages, ${fs.statSync(out).size} bytes`);
