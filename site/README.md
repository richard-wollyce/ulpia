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

[`DEPLOY.md`](DEPLOY.md). Short version: build both on the VPS, systemd runs the
binary on loopback, Caddy owns the public port and the certificates, DNS points the
apex at the VPS.
