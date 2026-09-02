// Turn content/posts/*.md into pages, an index and a feed.
//
// Publishing is writing a file and pushing it. That is the product's own thesis
// applied to its own website: the markdown in git is the source of truth, and
// everything under blog/ is derived and disposable, which is why it is
// gitignored and rebuilt from scratch on every run.
//
// Runs before vite, which then picks the generated HTML up as build inputs.
import { readdirSync, readFileSync, writeFileSync, mkdirSync, rmSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { marked } from "marked";

const ROOT = resolve(import.meta.dirname, "..");
const POSTS = join(ROOT, "content", "posts");
const OUT = join(ROOT, "blog");
// The feed is an asset rather than a page, and vite only copies what is a build
// input or lives in public/, so it is written there.
const FEED_DIR = join(ROOT, "public", "blog");
const SITE = "https://ulpia.io";

const esc = (s) =>
  String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

// The band, repeated on every page because it is the library's chrome. Kept as
// one constant so the lamp cannot drift between the hand-written pages and the
// generated ones.
const band = (current) => `    <header class="band">
      <div class="band-inner">
        <p class="wordmark"><a href="/">Ulpia</a></p>
        <nav class="nav" aria-label="Sections">
          <ul>
            <li><a href="/docs/">Docs</a></li>
            <li><a href="/blog/" aria-current="CURRENT">Writing</a></li>
          </ul>
        </nav>
        <button type="button" class="lamp-toggle" aria-label="Color scheme: light, press to change">
          <svg viewBox="0 0 28 28" width="28" height="28" aria-hidden="true" focusable="false">
            <g class="glow">
              <path class="ray ray-c" d="M 14 1.2 L 14 3.6" />
              <path class="ray ray-l" d="M 8.6 2.6 L 10.1 4.3" />
              <path class="ray ray-r" d="M 19.4 2.6 L 17.9 4.3" />
              <path class="shade-fill" d="M 6 13.5 L 10.5 5.5 L 17.5 5.5 L 22 13.5 Z" />
            </g>
            <g class="lamp">
              <path class="shade-line" d="M 6 13.5 L 10.5 5.5 L 17.5 5.5 L 22 13.5 Z" />
              <path class="stem" d="M 12.6 13.5 C 11.9 14.3, 11.1 15.0, 11.1 16.1 C 11.1 17.6, 12.9 18.2, 12.9 19.5 C 12.9 20.6, 10.6 20.9, 10.6 22.2 L 17.4 22.2 C 17.4 20.9, 15.1 20.6, 15.1 19.5 C 15.1 18.2, 16.9 17.6, 16.9 16.1 C 16.9 15.0, 16.1 14.3, 15.4 13.5 Z" />
              <path class="base" d="M 9 24.25 L 19 24.25" />
              <g class="chain-group">
                <path class="chain" d="M 19 14.35 L 19 16.35" />
                <circle class="chain-pull" cx="19" cy="17.15" r="1.05" />
              </g>
            </g>
          </svg>
        </button>
      </div>
    </header>`.replace("CURRENT", current);

const FOOTER = `    <footer>
      <p class="address-line">
        <a href="mailto:hello@ulpia.io">hello@ulpia.io</a>
      </p>
    </footer>`;

// `lang` comes from the post's front matter, because a Portuguese post served
// under lang="en" mishyphenates and misreads aloud; "en" stays the default so
// the hand-written pages and old posts change nothing.
const page = ({ title, description, canonical, body, ogType = "article", current = "true", lang = "en" }) => `<!doctype html>
<html lang="${lang}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${esc(title)}</title>
    <meta name="description" content="${esc(description)}" />
    <meta name="color-scheme" content="light dark" />
    <meta name="theme-color" media="(prefers-color-scheme: light)" content="#F6F1E7" />
    <meta name="theme-color" media="(prefers-color-scheme: dark)" content="#12100D" />
    <link rel="canonical" href="${esc(canonical)}" />
    <link rel="alternate" type="application/atom+xml" href="/blog/feed.xml" title="Ulpia" />
    <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
    <meta property="og:title" content="${esc(title)}" />
    <meta property="og:description" content="${esc(description)}" />
    <meta property="og:url" content="${esc(canonical)}" />
    <meta property="og:type" content="${esc(ogType)}" />
    <script src="/theme.js"></script>
    <link rel="stylesheet" href="/src/styles.css" />
  </head>
  <body>
    <a class="skip" href="#main">Skip to content</a>
${band(current)}
${body}
${FOOTER}
  </body>
</html>
`;

// Frontmatter, deliberately tiny: key: value pairs until the closing fence.
// A YAML dependency would buy nesting we do not use and a parser we do not read.
function parse(raw) {
  const m = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  if (!m) return { meta: {}, body: raw };
  const meta = {};
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_-]+):\s*(.*)$/);
    if (kv) meta[kv[1]] = kv[2].replace(/^["']|["']$/g, "").trim();
  }
  return { meta, body: m[2] };
}

const longDate = (iso) => {
  const [y, mo, d] = iso.split("-").map(Number);
  const months = ["January","February","March","April","May","June","July","August","September","October","November","December"];
  return `${d} ${months[mo - 1]} ${y}`;
};

if (existsSync(OUT)) rmSync(OUT, { recursive: true });
mkdirSync(OUT, { recursive: true });
if (existsSync(FEED_DIR)) rmSync(FEED_DIR, { recursive: true });
mkdirSync(FEED_DIR, { recursive: true });

const files = existsSync(POSTS) ? readdirSync(POSTS).filter((f) => f.endsWith(".md")) : [];

const posts = files
  .map((file) => {
    const { meta, body } = parse(readFileSync(join(POSTS, file), "utf8"));
    const slug = meta.slug || file.replace(/^\d{4}-\d{2}-\d{2}-/, "").replace(/\.md$/, "");
    if (!meta.title) throw new Error(`${file}: needs a title in its frontmatter`);
    if (!meta.date) throw new Error(`${file}: needs a date in its frontmatter (YYYY-MM-DD)`);
    return {
      slug,
      title: meta.title,
      date: meta.date,
      description: meta.description || "",
      author: meta.author || "Richard Wollyce",
      lang: meta.lang,
      html: marked.parse(body),
    };
  })
  // Newest first, which is what a reader arriving at an index expects.
  .sort((a, b) => (a.date < b.date ? 1 : -1));

for (const p of posts) {
  const dir = join(OUT, p.slug);
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "index.html"),
    page({
      title: `${p.title} · Ulpia`,
      description: p.description,
      canonical: `${SITE}/blog/${p.slug}/`,
      lang: p.lang,
      body: `    <main class="doc" id="main" tabindex="-1">
      <article>
        <h1>${esc(p.title)}</h1>
        <p class="doc-date">
          <time datetime="${esc(p.date)}">${longDate(p.date)}</time> · ${esc(p.author)}
        </p>
${p.html.split("\n").map((l) => (l ? "        " + l : l)).join("\n")}
      </article>
      <p class="doc-back"><a href="/blog/">All writing</a> · <a href="/">Back to the library</a></p>
    </main>`,
    }),
  );
}

const list = posts.length
  ? `      <ul class="post-list">
${posts
  .map(
    (p) => `        <li>
          <a href="/blog/${p.slug}/">${esc(p.title)}</a>
          <p class="post-meta"><time datetime="${esc(p.date)}">${longDate(p.date)}</time></p>
          ${p.description ? `<p class="post-desc">${esc(p.description)}</p>` : ""}
        </li>`,
  )
  .join("\n")}
      </ul>`
  : `      <p>Nothing published yet.</p>`;

writeFileSync(
  join(OUT, "index.html"),
  page({
    title: "Writing · Ulpia",
    description: "Richard Wollyce, on building Ulpia: what was decided, what it cost, and what is still wrong.",
    ogType: "website",
    current: "page",
    canonical: `${SITE}/blog/`,
    body: `    <main class="doc" id="main" tabindex="-1">
      <h1>Writing</h1>
      <p class="doc-date">Richard Wollyce, on building Ulpia: what was decided, what it cost, and what is still wrong.</p>
${list}
      <p class="doc-back"><a href="/blog/feed.xml">Feed</a> · <a href="/">Back to the library</a></p>
    </main>`,
  }),
);

// Atom rather than RSS: dates are unambiguous and the spec is one document.
const updated = posts[0] ? `${posts[0].date}T00:00:00Z` : "1970-01-01T00:00:00Z";
writeFileSync(
  join(FEED_DIR, "feed.xml"),
  `<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Ulpia</title>
  <link href="${SITE}/blog/feed.xml" rel="self" />
  <link href="${SITE}/blog/" />
  <id>${SITE}/blog/</id>
  <updated>${updated}</updated>
${posts
  .map(
    (p) => `  <entry>
    <title>${esc(p.title)}</title>
    <link href="${SITE}/blog/${p.slug}/" />
    <id>${SITE}/blog/${p.slug}/</id>
    <updated>${p.date}T00:00:00Z</updated>
    <author><name>${esc(p.author)}</name></author>
    <summary>${esc(p.description)}</summary>
  </entry>`,
  )
  .join("\n")}
</feed>
`,
);

console.log(`posts: ${posts.length} generated into blog/`);
