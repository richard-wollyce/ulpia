# Deploying ulpia.io

The site is static: `frontend/dist/` is the whole product. It ships on **Cloudflare
Pages**, and the Rust binary in `server/` stays in the repository for the day
something has to be computed per request.

Why this shape. Every job the binary does for this page (security headers, a real 404
status, compression, immutable caching on hashed assets) is native to Pages, so a
server in front of files we already have would be a component with no second use case.
That is the same failure as a shortcut, pointing the other way. The binary is not
wasted: it is written, tested, and it is what stands up on a subdomain the day a live
`kb route` endpoint, an MCP endpoint, or anything else needs to run per request. Until
then it serves the site locally, which is where it earns its keep today.

The trade accepted, named: Cloudflare terminates TLS and sees traffic. The page's claim
in the footer is about what the page itself does, and it stays true, since the page sets
no cookies, runs no analytics and requests nothing from another origin. If we later want
that claim to cover the transport as well, that is the VPS path in section 6.

## 1. Settings to change before the domain goes live

Two Cloudflare defaults would break this page. Both are one toggle.

**Email Address Obfuscation: turn it off.** Cloudflare enables it automatically on
signup. It rewrites email addresses in the HTML and injects `email-decode.min.js` to
decode them. Our primary call to action is a `mailto:` link, so the default would turn
our one conversion element into markup that requires JavaScript, breaking the no-JS
floor the page is built to honor, and adding a script to a page that ships one. It is
not blocked by our CSP either, because it is served from our own origin under
`/cdn-cgi/`. Dashboard: Security, then Settings, then Email Address Obfuscation, off.

**Rocket Loader: leave it off.** It defers and reorders scripts. `theme.js` is
deliberately synchronous in `<head>` so the stored color scheme applies before first
paint; deferring it reintroduces exactly the flash that placement exists to prevent.

Re-check both after launch. A default that flips silently would falsify a claim we
made in writing, which is worse than never having made it.

## 2. Build

```bash
cd site/frontend && npm ci && npm run build
```

`dist/` is the artifact. It contains `_headers`, which Pages parses and applies (see
`frontend/public/_headers` for what it sets and why). Confirm it survived the build:

```bash
test -f dist/_headers && echo "headers present"
```

## 3. Deploy

Direct upload, so no git remote or GitHub connection is required:

```bash
npx wrangler pages deploy dist --project-name ulpia
```

The first run creates the project and prints a `*.pages.dev` URL. Verify there before
attaching the domain: a broken deploy behind the real name is a worse minute than a
broken deploy behind a temporary one.

## 4. The domain

In the Pages project: Custom domains, add `ulpia.io`, then add `www.ulpia.io` and set
it to redirect to the apex. The apex, not `www`, is canonical: one canonical origin
means one cache, one set of search results, and no cookie-scope surprises later.

Cloudflare creates the DNS records itself when the zone is on its nameservers. **Leave
existing MX records alone**: `hello@ulpia.io` mail routing is independent of where the
website points, and the page is useless if the address in it stops answering.

## 5. Verify, before calling it done

```bash
curl -sI https://ulpia.io/ | grep -i "content-security-policy\|cache-control\|x-content-type"
curl -s -o /dev/null -w "%{http_code}\n" https://ulpia.io/no-such-page   # expect 404
curl -s https://ulpia.io/ | grep -c "cdn-cgi"                            # expect 0
curl -s https://ulpia.io/ | grep -o 'mailto:[^"]*'                       # expect the real address
curl -sI https://ulpia.io/assets/$(curl -s https://ulpia.io/ | grep -o 'assets/styles-[^"]*css' | head -1 | cut -d/ -f2) | grep -i cache-control
```

In order, those check that the policy arrived, that a wrong URL is honestly a 404, that
Cloudflare injected nothing, that the CTA is still a real `mailto`, and that hashed
assets are immutable while the HTML is not. The third one is the one that catches a
silently re-enabled obfuscation setting.

Then open the page and press the lamp. Automated checks do not have eyes.

## 6. When the server earns its place

The moment something must be computed per request, the binary moves to a VPS on a
subdomain (`api.ulpia.io` or similar) while the page stays on Pages. That split keeps
the landing page immune to the server being down.

Build where it runs, because cross-compiling from Windows for one binary costs more
setup than it saves:

```bash
git clone <this repository> ulpia && cd ulpia/site/server && cargo build --release
sudo mkdir -p /srv/ulpia && sudo cp target/release/ulpia-site /srv/ulpia/
```

`/etc/systemd/system/ulpia-site.service`:

```ini
[Unit]
Description=ulpia.io service
After=network.target

[Service]
ExecStart=/srv/ulpia/ulpia-site
Environment=STATIC_DIR=/srv/ulpia/dist
Environment=PORT=8080
# Loopback is the binary's default; HOST is only set when that should change.
Restart=on-failure

# The process needs nothing but read access to its own directory. Saying so costs
# five lines and turns a compromise of the process into a compromise of nothing.
DynamicUser=yes
ProtectSystem=strict
ProtectHome=yes
NoNewPrivileges=yes
ReadOnlyPaths=/srv/ulpia

[Install]
WantedBy=multi-user.target
```

TLS and the public port belong to Caddy; the binary listens on loopback and never sees
a certificate. Caddy provisions and renews Let's Encrypt certificates on its own and
reloads without dropping connections, so certificate lifecycle never requires touching
our process. Append to `/etc/caddy/Caddyfile`:

```
api.ulpia.io {
    reverse_proxy 127.0.0.1:8080
}
```

```bash
sudo systemctl daemon-reload && sudo systemctl enable --now ulpia-site
sudo systemctl reload caddy
curl -s http://127.0.0.1:8080/health   # expect: ok
```

## 7. Redeploying the page

```bash
cd site/frontend && npm run build && npx wrangler pages deploy dist --project-name ulpia
```

Before a deploy that includes the terminal block, re-take the capture if the base it
queries has changed, because that block is the only claim on the page a reader can
reproduce:

```bash
node tools/capture.mjs 3        # paste the printed <pre> over the one in index.html
```

Rolling back is selecting a previous deployment in the Pages dashboard and promoting
it. That is the one operational advantage of this path worth naming out loud: the
previous version never stopped existing.
