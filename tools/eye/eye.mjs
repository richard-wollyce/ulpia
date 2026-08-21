/* Aldus's eye: renders a surface and reports what it actually looks like.
 *
 * Why this exists, stated plainly because the gap it closes was expensive. The
 * constitution says "verify on the real thing" and "a design reviewed only at
 * desktop width in light mode was not reviewed". There was no way to obey it.
 * The in-app browser could not screenshot (it only composites while its pane is
 * visible) and its getComputedStyle returned stale values: setting an inline
 * colour with !important did not change what it reported. So every visual claim
 * was unverifiable, and a dark-mode hover that changed nothing shipped through
 * review because nobody could look at it. Headless Chromium composites with no
 * pane and reports real computed values, which is the whole mechanism here.
 *
 * The tool reads computed styles and DIFFS them between states. Pixels alone
 * cannot tell you a hover changed nothing; a diff can, and says so as an error.
 *
 * Built from tools/eye/SPEC.md. Implemented: the matrix, preflight, NO_OP,
 * TOUCH_DEAD, HOVER_ONLY, WEAK_SIGNAL, CONTRAST_TEXT. NOT implemented, and the
 * summary says so rather than letting a green run imply them: the reduce-motion
 * axis, MOTION_5S, CONTRAST_NONTEXT, the no-JS pass, waivers, contact sheets.
 *
 *   node tools/eye/eye.mjs <sheet.mjs> [--quick] [--shots off|fail|all]
 */

import { chromium } from "playwright";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { pathToFileURL } from "node:url";

/* The properties a state is allowed to speak through. Sampled on the element
 * and on both pseudo-elements, because this system deliberately carries the
 * nav's link marker in ::after and a diff blind to it would call that hover a
 * no-op. */
const PROPS = [
  "color", "background-color", "border-color", "border-bottom-color",
  "text-decoration-color", "text-decoration-line", "outline-color",
  "outline-width", "outline-style", "transform", "opacity", "box-shadow",
  "font-weight", "visibility",
];

const STATES = ["hover", "focus-visible", "active"];

const lum = (rgb) => {
  const c = rgb.map((v) => v / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
};
const ratio = (a, b) => {
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};
const parseRgb = (s) => {
  const n = (s || "").match(/[\d.]+/g);
  return n && n.length >= 3 ? n.slice(0, 3).map(Number) : null;
};
const isTransparent = (s) => !s || s === "transparent" || /rgba\(.*,\s*0\)$/.test(s);

/* Read the element, its two pseudo-elements, and the first opaque background
 * behind it. The backdrop walk is why contrast here is measured against what is
 * actually painted rather than against whatever token the author had in mind. */
function sample(el, props) {
  const read = (pseudo) => {
    const cs = getComputedStyle(el, pseudo);
    const out = {};
    for (const p of props) out[p] = cs.getPropertyValue(p);
    return out;
  };
  let node = el, backdrop = null;
  while (node && node !== document.documentElement.parentNode) {
    const bg = getComputedStyle(node).backgroundColor;
    if (bg && bg !== "transparent" && !/rgba\(.*,\s*0\)$/.test(bg)) { backdrop = bg; break; }
    node = node.parentElement;
  }
  const r = el.getBoundingClientRect();
  return {
    el: read(null), after: read("::after"), before: read("::before"),
    backdrop,
    box: { x: r.x, y: r.y, w: r.width, h: r.height },
    overflows: document.documentElement.scrollWidth > document.documentElement.clientWidth,
  };
}

const diff = (a, b) => {
  const out = [];
  for (const part of ["el", "after", "before"]) {
    for (const p of PROPS) {
      if (a[part][p] !== b[part][p]) out.push(part === "el" ? p : `${part}:${p}`);
    }
  }
  return out;
};

async function main() {
  const argv = process.argv.slice(2);
  if (!argv.length || argv[0].startsWith("-")) {
    console.error("usage: node tools/eye/eye.mjs <sheet.mjs> [--quick] [--shots off|fail|all]");
    process.exit(2);
  }
  const sheetPath = resolve(argv[0]);
  const flag = (n, d) => { const i = argv.indexOf(n); return i < 0 ? d : argv[i + 1]; };
  const quick = argv.includes("--quick");
  const shots = flag("--shots", "fail");
  const outDir = resolve(flag("--out", resolve(dirname(sheetPath), "shots")));

  const sheet = (await import(pathToFileURL(sheetPath).href)).default;
  const widths = quick ? [360, 1280] : (sheet.widths ?? [360, 768, 1280]);
  const schemes = sheet.schemes ?? ["light", "dark"];
  /* 16 is the browser default. 24 is not an extreme: it is a common setting for
   * a reader with low vision, and it is above the width at which this band was
   * measured to start overflowing. */
  const fontSizes = quick ? [16, 24] : (sheet.fontSizes ?? [16, 20, 24]);
  const partial = quick;

  const browser = await chromium.launch();
  const findings = [];
  const rows = [];
  let sampled = 0;
  const add = (sev, id, where, msg) => findings.push({ sev, id, where, msg });

  await mkdir(outDir, { recursive: true });

  for (const page of sheet.pages) {
    const url = new URL(page.path, sheet.base).href;
    for (const width of widths) {
      for (const scheme of schemes) {
       /* The root font-size axis. Every horizontal term in this system is
        * rem-derived and the viewport is not, so the two curves cross somewhere
        * and the band starts pushing the page wider than the screen. Without
        * this axis the tool reviews only the person who never changed a setting,
        * and the reader who most needs the band to hold together is the one who
        * enlarged the type. Setting it on documentElement is what a browser's
        * own font-size preference does to rem. */
       for (const rem of fontSizes) {
        /* Two profiles per cell. The coarse one is not decoration: (hover:hover)
         * is false there, which is the only way to see that a control's entire
         * feedback lives in a state a phone can never enter. */
        for (const pointer of ["fine", "coarse"]) {
          const ctx = await browser.newContext({
            colorScheme: scheme,
            viewport: { width, height: 900 },
            hasTouch: pointer === "coarse",
            isMobile: pointer === "coarse",
            deviceScaleFactor: 1,
          });
          const p = await ctx.newPage();
          let resp;
          try {
            resp = await p.goto(url, { waitUntil: "networkidle", timeout: 15000 });
          } catch {
            console.error(`eye: cannot reach ${url}. Is the dev server up?`);
            console.error(`     npm --prefix site/frontend run dev`);
            await browser.close();
            process.exit(2);
          }
          if (!resp || !resp.ok()) { await browser.close(); console.error(`eye: ${url} returned ${resp && resp.status()}`); process.exit(2); }

          if (rem !== 16) {
            await p.evaluate((n) => { document.documentElement.style.fontSize = n + "px"; }, rem);
            await p.waitForTimeout(200);
          }

          const ground = await p.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--ground").trim());
          if (!ground) { await browser.close(); console.error("eye: --ground is empty, the stylesheet did not load. Nothing below would mean anything."); process.exit(2); }

          const over = await p.evaluate((vw) => {
            const doc = document.documentElement;
            if (doc.scrollWidth <= doc.clientWidth) return null;
            let worst = null;
            for (const el of document.querySelectorAll("*")) {
              const right = el.getBoundingClientRect().right;
              if (right > vw + 1 && (!worst || right > worst.right)) {
                worst = { right, tag: el.tagName.toLowerCase(), cls: (el.className || "").toString().slice(0, 30) };
              }
            }
            return { scrollW: doc.scrollWidth, clientW: doc.clientWidth, worst };
          }, width);
          if (over) {
            const who = over.worst ? `<${over.worst.tag} class="${over.worst.cls}">` : "unknown";
            add("error", "OVERFLOW", `page @ ${page.name} ${width}px r${rem} ${scheme} ${pointer}`,
                `document is ${over.scrollW}px in a ${over.clientW}px viewport; widest offender ${who}`);
          }

          for (const t of sheet.targets) {
            const loc = p.locator(t.selector).first();
            if (!(await loc.count())) continue;

            const rest = await loc.evaluate(sample, PROPS);
            sampled++;
            const cell = `${page.name} ${width}px r${rem} ${scheme} ${pointer}`;
            const where = `${t.name} @ ${cell}`;

            /* Overflow is a property of the page, not of this target. Raising it
             * per target blamed whichever element happened to be sampled when
             * the flag was read: it reported the nav and the lamp for a document
             * that was being widened by the terminal plate, on a run where the
             * band was already innocent. Checked once per cell now, and it names
             * the widest offender so the finding points at the thing to fix. */

            const seen = {};
            for (const st of STATES) {
              if (pointer === "coarse" && st === "hover") continue;
              try {
                if (st === "hover") await loc.hover({ timeout: 3000 });
                else if (st === "focus-visible") {
                  /* Reached by real Tab presses, never by el.focus(). The
                   * :focus-visible heuristic asks how focus arrived: scripted
                   * focus on a button does not match it in Chromium, so the
                   * scripted route reported the lamp's focus ring as absent in
                   * every single cell. That was the tool inventing a defect in
                   * a control that is fine, which is worse than missing one.
                   * The walk is bounded, and a target it cannot reach is an
                   * UNMEASURED finding rather than a silent pass. */
                  await p.evaluate(() => document.body.focus());
                  await p.keyboard.press("Tab");
                  let reached = false;
                  for (let i = 0; i < 30 && !reached; i++) {
                    reached = await loc.evaluate((n) => n === document.activeElement).catch(() => false);
                    if (!reached) await p.keyboard.press("Tab");
                  }
                  if (!reached) throw new Error("target not reachable by Tab within 30 stops");
                }
                else {
                  /* :active is set by the browser's own input path. A synthetic
                   * MouseEvent does NOT set it, so dispatching one samples rest
                   * and reports every press in the system as a no-op. Found by
                   * running this tool against a press that demonstrably works. */
                  const b = await loc.boundingBox();
                  if (!b) throw new Error("no box");
                  await p.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
                  await p.mouse.down();
                }
                await p.waitForTimeout(320);
                const s = await loc.evaluate(sample, PROPS);
                if (st === "active") {
                  /* Release AWAY from the element, never on it. A press and a
                   * release over the same link is a click, and a click on the
                   * wordmark navigates home: every target sampled after it was
                   * being measured on a different page than the sheet names,
                   * silently. Moving out before releasing is also how a person
                   * cancels a click they changed their mind about. */
                  await p.mouse.move(2, 2);
                  await p.mouse.up();
                }
                seen[st] = diff(rest, s);
                if (st === "hover" || st === "active") {
                  /* Only where there is text to read. The lamp is a button whose
                   * content is an SVG, so its inherited `color` against a
                   * translucent wash is a number about nothing. */
                  const hasText = await loc.evaluate((n) => (n.innerText || "").trim().length > 0);
                  const c = parseRgb(s.el.color), bg = parseRgb(s.backdrop);
                  if (hasText && c && bg) {
                    const r = ratio(c, bg);
                    if (r < 4.5) add("error", "CONTRAST_TEXT", where, `${st} colour ${s.el.color} on ${s.backdrop} is ${r.toFixed(2)}:1`);
                  }
                  if (seen[st].length === 1 && seen[st][0] === "color") {
                    const a = parseRgb(rest.el.color), b = parseRgb(s.el.color);
                    if (a && b && ratio(a, b) < 1.5) add("warn", "WEAK_SIGNAL", where, `${st} is colour alone, ${ratio(a, b).toFixed(2)}:1 from rest, with no second signal`);
                  }
                }
              } catch (e) {
                /* A state that could not be entered is not a blank in a table,
                 * it is a hole in the review. Reporting it as "n/a" and exiting
                 * 0 tells the reader the control passed when nobody ever looked
                 * at it, which is the same lie as a green run on a page that
                 * failed to load. */
                seen[st] = null;
                add("error", "UNMEASURED", where, `could not enter :${st}: ${String(e.message || e).split("\n")[0].slice(0, 120)}`);
              }
              await p.mouse.up().catch(() => {});
              await p.mouse.move(0, 0);
              await loc.evaluate((n) => n.blur()).catch(() => {});
              await p.waitForTimeout(120);
            }

            for (const [st, d] of Object.entries(seen)) {
              if (d && d.length === 0) add("error", "NO_OP", where, `:${st} changed nothing at all`);
            }
            if (pointer === "coarse") {
              const dead = (seen.active ?? []).length === 0 && (seen["focus-visible"] ?? []).length === 0;
              if (dead) add("error", "TOUCH_DEAD", where, "no press and no focus feedback: a phone gets nothing from this control");
            }
            rows.push({ cell, target: t.name, ...Object.fromEntries(Object.entries(seen).map(([k, v]) => [k, v === null ? "n/a" : v.length ? v.join(",") : "NADA"])) });
          }

          const bad = findings.some((f) => f.where.includes(`${page.name} ${width}px r${rem} ${scheme} ${pointer}`));
          if (shots === "all" || (shots === "fail" && bad)) {
            await p.screenshot({ path: resolve(outDir, `${page.name}-${width}-r${rem}-${scheme}-${pointer}.png`), fullPage: false });
          }
          await ctx.close();
        }
       }
      }
    }
  }
  await browser.close();

  const errors = findings.filter((f) => f.sev === "error");
  const warns = findings.filter((f) => f.sev === "warn");
  await writeFile(resolve(outDir, "report.json"), JSON.stringify({ findings, rows }, null, 1), "utf-8");

  console.log(`\neye: ${sampled} amostras, ${widths.length} larguras x ${schemes.length} esquemas x 2 ponteiros`);
  for (const f of [...errors, ...warns]) console.log(`  ${f.sev === "error" ? "ERRO " : "aviso"} ${f.id.padEnd(14)} ${f.where}\n        ${f.msg}`);
  if (!findings.length) console.log("  nada encontrado nas checagens implementadas");
  console.log(`\n  relatorio: ${resolve(outDir, "report.json")}`);
  if (partial) console.log("  PARCIAL: --quick cortou larguras. Um verde aqui nao e um verde completo.");
  console.log("  NAO checado por este tool: reduce-motion, MOTION_5S, contraste nao-textual, passe sem JS.");
  process.exit(errors.length ? 1 : 0);
}

main().catch((e) => { console.error(e); process.exit(2); });
