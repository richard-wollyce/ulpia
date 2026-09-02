/* Viewport crops, for pages too tall to read in one image.
 *
 * A full-page shot of a 26,000 pixel documentation page scales to a thumbnail
 * where nothing is legible, so it answers no question. This takes the screenful
 * a reader actually meets: the top, and whatever an element selector points at.
 *
 *   node tools/eye/crops.mjs <outDir>
 */
import { chromium } from "playwright";
import { createServer } from "node:http";
import { readFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve, join, extname } from "node:path";

const ROOT = resolve(import.meta.dirname, "../../site/frontend/dist");
const OUT = resolve(process.argv[2] ?? join(import.meta.dirname, "crops"));

// name, path, and the selectors worth a picture of their own. The selectors are
// the things a stylesheet gets wrong on a narrow screen: a table, a code block,
// the navigation that has to collapse.
const SHOTS = [
  ["reference-top", "/docs/reference/", null],
  ["reference-table", "/docs/reference/", "table"],
  ["reference-code", "/docs/reference/", "pre"],
  ["how-top", "/docs/how-it-works/", null],
  ["how-table", "/docs/how-it-works/", "table"],
  ["local-code", "/docs/local/", "pre"],
  ["cloud-code", "/docs/cloud/", "pre"],
  ["docs-index", "/docs/", null],
  ["roadmap-top", "/docs/roadmap/", null],
  ["decisions-table", "/docs/decisions/", "table"],
  ["concepts-top", "/docs/concepts/", null],
];

const TYPES = { ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".woff2": "font/woff2", ".svg": "image/svg+xml", ".xml": "application/xml" };
const server = createServer(async (req, res) => {
  const url = new URL(req.url, "http://localhost");
  let file = join(ROOT, decodeURIComponent(url.pathname));
  if (url.pathname.endsWith("/")) file = join(file, "index.html");
  if (!existsSync(file)) { res.writeHead(404); res.end("nf"); return; }
  const body = await readFile(file);
  res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
  res.end(body);
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}`;
await mkdir(OUT, { recursive: true });

const browser = await chromium.launch();
let n = 0;
for (const [width, label, scheme] of [[390, "phone", "dark"], [1280, "laptop", "light"]]) {
  const ctx = await browser.newContext({
    viewport: { width, height: width === 390 ? 844 : 900 },
    colorScheme: scheme,
    reducedMotion: "reduce",
  });
  const page = await ctx.newPage();
  for (const [name, path, sel] of SHOTS) {
    await page.goto(base + path, { waitUntil: "networkidle" });
    await page.evaluate(() => document.fonts.ready);
    const file = join(OUT, `${name}-${label}.png`);
    if (sel) {
      const el = page.locator(sel).first();
      if (await el.count()) {
        await el.scrollIntoViewIfNeeded();
        // The screenful around it, not the element alone: an element shot hides
        // whether it overflows the page it sits in, which is the actual question.
        await page.screenshot({ path: file });
      } else { console.log(`  no ${sel} on ${path}`); continue; }
    } else {
      await page.screenshot({ path: file });
    }
    n++;
  }
  await ctx.close();
}
await browser.close();
server.close();
console.log(`${n} crops in ${OUT}`);
