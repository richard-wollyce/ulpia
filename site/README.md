# site

The public face of Ulpia at [ulpia.io](https://ulpia.io). Two parts, one page.

| Part | What it is |
|---|---|
| `frontend/` | The page. Hand written HTML and CSS, TypeScript compiled and hashed by Vite. No framework: the page is text, and a framework would be weight with nothing to carry |
| `server/` | One Rust binary (Axum) that serves the build. It exists because the site will grow server work, a blog, subdomains, and a route table is where that lands without a rehost |

## Run it locally

Frontend alone, with hot reload, for design work:

```bash
cd frontend && npm install && npm run dev
```

The real thing, exactly as production serves it:

```bash
cd frontend && npm run build
cd ../server && cargo run
# http://127.0.0.1:8080
```

## Deploy

[`DEPLOY.md`](DEPLOY.md). Short version, and it is the present tense: the page is on
Cloudflare Pages and the project is git-connected, so **a push to `main` publishes it**.
Nothing else is required and nothing else should be run.

The VPS, systemd and the reverse proxy described in `DEPLOY.md` section 6 are the future,
for when the server earns its place. This line used to summarise that section as though it
were today, which made two operational documents appear to disagree about where the site
runs.
