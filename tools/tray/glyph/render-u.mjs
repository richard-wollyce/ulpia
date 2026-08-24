// Rasterizes the Ulpia U once, with the real EB Garamond, into an alpha mask that
// make-icons.py bakes into every icon. Run when the glyph changes, never at build:
//   node glyph/render-u.mjs
// The eye's playwright install is reused so this repo needs no second browser.
import { chromium } from "../../eye/node_modules/playwright/index.mjs";
import { readFileSync, writeFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const SIZE = 512;

// The kb binary already embeds EB Garamond; the same woff2 is the one source of the
// letterform, so the tray and the reading room cannot drift apart.
const font = readFileSync(join(here, "../../kb/src/fonts/eb-garamond-latin-600-normal.woff2"));
const fontB64 = font.toString("base64");

const html = `<!doctype html><meta charset="utf-8"><style>
  @font-face { font-family: G; src: url(data:font/woff2;base64,${fontB64}) format("woff2"); }
  html,body { margin:0; background:transparent; }
  canvas { display:block; }
</style><canvas id="c" width="${SIZE}" height="${SIZE}"></canvas>`;

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: SIZE, height: SIZE } });
await page.setContent(html);
await page.evaluate(() => document.fonts.load('400px G').then(() => document.fonts.ready));

const alpha = await page.evaluate((SIZE) => {
  const ctx = document.getElementById("c").getContext("2d");
  ctx.clearRect(0, 0, SIZE, SIZE);
  ctx.fillStyle = "#fff";
  // Sized so the U fills the badge's inner field the way the index bars used to:
  // the glyph box runs roughly from y=7 to y=25 in the 32-unit grid the badge uses.
  ctx.font = "370px G";
  ctx.textAlign = "center";
  ctx.textBaseline = "alphabetic";
  ctx.fillText("U", SIZE / 2, SIZE * 0.78);
  const d = ctx.getImageData(0, 0, SIZE, SIZE).data;
  const a = new Array(SIZE * SIZE);
  for (let i = 0; i < SIZE * SIZE; i++) a[i] = d[i * 4 + 3];
  return a;
}, SIZE);

await browser.close();
writeFileSync(join(here, "u-glyph.alpha.gz"), gzipSync(Buffer.from(alpha)));
console.log("u-glyph.alpha.gz:", alpha.filter(v => v > 0).length, "covered pixels of", SIZE * SIZE);
