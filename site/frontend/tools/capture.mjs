// Turn a real `kb route` run into the landing page's terminal markup.
//
// Why this exists: the block's claim is that a reader can run the command and
// see the same thing. That claim expires as the base grows, so the capture has
// to be refreshable, and the spans carry the tool's exact column padding
// (`{:<44}` in tools/kb/src/main.rs), which nobody can hand-count reliably
// twice. This reads the real bytes and emits the markup, so a refresh is one
// command and the pads cannot drift from the output they describe.
//
//   node tools/capture.mjs [hits]
//
// It prints the <pre> element. Paste it over the existing one in index.html,
// or diff it against what is there to see whether the capture has gone stale.
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

const REPO = resolve(import.meta.dirname, "../../..");
const KB = resolve(REPO, "tools/kb/target/release/kb.exe");
const QUESTION = "why is there no embedding model in the retrieval path";
const BASE = "decisions";
const HITS = Number(process.argv[2] ?? 3);

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const pad = (s) => `<span class="pad">${s}</span>`;

const raw = execFileSync(KB, ["route", QUESTION, BASE, "--hybrid"], { cwd: REPO, encoding: "utf8" });
// Drop the indented preview lines: whole lines only, never an edit within one.
const lines = raw.split(/\r?\n/).filter((l) => !l.startsWith("         "));

const meta = lines.filter((l) => /^(question|indexed):/.test(l));
// `  {:>5.3}  {:<8} {:<44} {}` : leading pad, score, pad, base, pad, file, pad, why.
const hitRe = /^(\s+)(\S+)(\s+)(\S+)(\s+)(\S+)(\s+)(.+)$/;
const hits = lines.filter((l) => hitRe.test(l) && l.includes("+")).slice(0, HITS);

const cmd =
  `<span class="line cmd-line"><span class="prompt">$ </span><span class="cmd">kb route "${esc(QUESTION)}" ${BASE} ` +
  `<span class="flag">--hybrid</span></span></span>`;

const metaHtml = meta.map((l) => `<span class="line meta">${esc(l)}</span>`).join("");

const hitsHtml = hits
  .map((l) => {
    const [, p0, score, p1, base, p2, file, p3, why] = l.match(hitRe);
    return (
      `<span class="line hit">${pad(p0)}<span class="score">${esc(score)}</span>${pad(p1)}` +
      `<span class="base">${esc(base)}</span>${pad(p2)}<span class="file">${esc(file)}</span>` +
      `${pad(p3)}<span class="agree">${esc(why)}</span></span>`
    );
  })
  .join("");

process.stdout.write(
  `<pre class="term">${cmd}<span class="blank"></span>${metaHtml}<span class="blank"></span>${hitsHtml}</pre>\n`,
);
