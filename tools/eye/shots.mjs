/* Screenshot every page of the built site, at two widths and both schemes.
 *
 * Why this exists beside eye.mjs: the eye diffs computed styles between states
 * of one component, which is the right instrument for a hover that changes
 * nothing. It is the wrong instrument for the question a page-level review
 * asks, which is "does this page read". That question is answered by looking,
 * so this takes the pictures and a person or a model reads them.
 *
 * The design spec's process law is that nothing ships unseen, and it means the
 * whole page, at the width most readers hold, in the scheme their phone is in.
 *
 *   node tools/eye/shots.mjs [outDir]
 *
 * It serves dist/ itself rather than asking for a running preview, because a
 * review that depends on somebody having started a server is a review that gets
 * skipped. Static files only: this is the same bytes Pages would send.
 */
import { chromium } from "playwright";
import { createServer } from "node:http";
import { readFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve, join, extname } from "node:path";

const ROOT = resolve(import.meta.dirname, "../../site/frontend/dist");
const OUT = resolve(process.argv[2] ?? join(import.meta.dirname, "shots"));

const PAGES = [
  ["landing", "/"],
  ["docs", "/docs/"],
  ["concepts", "/docs/concepts/"],
  ["how-it-works", "/docs/how-it-works/"],
  ["local", "/docs/local/"],
  ["cloud", "/docs/cloud/"],
  ["reference", "/docs/reference/"],
  ["decisions", "/docs/decisions/"],
  ["roadmap", "/docs/roadmap/"],
  ["benchmarks", "/benchmarks/"],
  ["blog", "/blog/"],
  ["post", "/blog/the-floor-scales-with-the-corpus/"],
  ["404", "/404.html"],
];

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".woff2": "font/woff2",
  ".svg": "image/svg+xml",
  ".xml": "application/xml",
  ".json": "application/json",
};

// Directory URLs map to index.html, which is what Pages does and what makes the
// screenshots the same page a visitor gets.
const server = createServer(async (req, res) => {
  const url = new URL(req.url, "http://localhost");
  let file = join(ROOT, decodeURIComponent(url.pathname));
  if (url.pathname.endsWith("/")) file = join(file, "index.html");
  if (!existsSync(file)) {
    res.writeHead(404, { "content-type": "text/plain" });
    res.end("not found: " + url.pathname);
    return;
  }
  try {
    const body = await readFile(file);
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
    res.end(body);
  } catch (e) {
    res.writeHead(500, { "content-type": "text/plain" });
    res.end(String(e));
  }
});

await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}`;
await mkdir(OUT, { recursive: true });

const browser = await chromium.launch();
const taken = [];
for (const [width, label] of [[390, "phone"], [1280, "laptop"]]) {
  for (const scheme of ["light", "dark"]) {
    const ctx = await browser.newContext({
      viewport: { width, height: width === 390 ? 844 : 900 },
      deviceScaleFactor: 1,
      colorScheme: scheme,
      // The reveal animation starts elements at opacity 0 and raises them on
      // intersection. A screenshot taken before that runs photographs a blank
      // page and reads as a broken layout, so motion is off for the camera.
      reducedMotion: "reduce",
    });
    const page = await ctx.newPage();
    for (const [name, path] of PAGES) {
      const resp = await page.goto(base + path, { waitUntil: "networkidle" });
      if (!resp.ok() && !path.endsWith("404.html")) {
        console.log(`  ${resp.status()}  ${path}`);
      }
      // Fonts settle after networkidle on the first paint of a family.
      await page.evaluate(() => document.fonts.ready);
      const file = join(OUT, `${name}-${label}-${scheme}.png`);
      await page.screenshot({ path: file, fullPage: true });
      taken.push(file);
    }
    await ctx.close();
  }
}
await browser.close();
server.close();
console.log(`${taken.length} screenshots in ${OUT}`);
