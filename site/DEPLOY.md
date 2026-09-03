# Deploying ulpia.io

Three things ship, not one, and this file used to name only the first:

1. **The static site.** `frontend/dist/`, on **Cloudflare Pages**.
2. **One Pages Function.** `frontend/functions/api/subscribe.ts`, compiled by Pages into
   the route `/api/subscribe` by its filename. It is live today.
3. **One D1 database.** `ulpia-subscribers`, bound as `DB` in `frontend/wrangler.toml`,
   holding the table in `frontend/schema.sql`.

**Sections 2 through 5 covered only the first for a long time, and that is how a person
following this runbook end to end ships a site whose subscribe modal has no database
behind it while `/privacy/` promises the visitor their address is stored in D1.** The
database and the schema are steps now, in section 2b, because a step that lives only as
a comment inside the file it applies is a step nobody runs twice.

The Rust binary in `server/` stays in the repository. Every job it does for the static
page (security headers, a real 404 status, compression, immutable caching on hashed
assets) is native to Pages, so a server in front of files we already have would be a
component with no second use case. **The per-request moment named below in section 6
already arrived, on 2026-08-19, and was answered by a Pages Function rather than by
standing the binary up.** Section 6 is still the path for the day something needs a
process rather than a handler: a live `kb route` endpoint, an MCP endpoint, anything
that has to hold state or run longer than an edge function should. Until then the binary
serves the site locally, which is where it earns its keep today.

The trade accepted, named: Cloudflare terminates TLS and sees traffic. It also counts the
traffic, since Web Analytics is deliberately on (section 1). The footer and `/privacy/`
say so in their own words rather than claiming a purity the deployment does not have. If
we later want the transport out of Cloudflare's hands too, that is the VPS path in
section 6.

## 1. Settings to change before the domain goes live

Three Cloudflare settings decide what a visitor actually receives. Each is one toggle,
and **each is invisible to a byte comparison against a local build**, because Cloudflare
applies them at the edge after the build. Check them the way section 5 checks them, with
a browser's `User-Agent`, or you will confirm a page that nobody is served.

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

**Web Analytics: on, deliberately.** Pages injects
`static.cloudflareinsights.com/beacon.min.js` before `</body>` of every HTML response
whose request looks like a browser's. It is cookieless and does not fingerprint, and
the footer and `/privacy/` describe it rather than denying it.

**It needs the two `cloudflareinsights.com` entries in `frontend/public/_headers` to
work at all.** While `script-src` was `'self'` alone the browser refused the injected
script, so the setting read as on in the dashboard and was dead on the page. That is the
failure mode worth remembering here: a policy silently blocking a measurement you believe
you are taking is worse than not measuring, because the dashboard still shows a number
and the number is of nothing.

Re-check all three after launch. A default that flips silently would falsify a claim we
made in writing, which is worse than never having made it, and one of them already did:
Web Analytics was on while the footer said "no analytics", found on 2026-09-03.

## 2. Build

```bash
cd site/frontend && npm ci && npm run build
```

`dist/` is the artifact. It contains `_headers`, which Pages parses and applies (see
`frontend/public/_headers` for what it sets and why). Confirm it survived the build:

```bash
test -f dist/_headers && echo "headers present"
```

## 2b. The database, once per account

The subscribe endpoint writes to D1. **Skip this and the site deploys clean, the modal
submits, and every visitor gets a 500 while `/privacy/` promises their address was
stored.** The failure is invisible from the outside, because the endpoint answers
identically whether the row was written or the table was missing, by design: see the
enumeration-oracle comment in `functions/api/subscribe.ts`.

```bash
cd site/frontend
npx wrangler d1 create ulpia-subscribers          # prints database_id, paste into wrangler.toml
npx wrangler d1 execute ulpia-subscribers --remote --file=./schema.sql
```

The `database_id` is an identifier, not a secret, and it is useless without an
authenticated account. It already sits in `wrangler.toml` for this deployment.

Verify the table exists rather than assuming the command worked, and note that
`npx wrangler d1 list` reports `num_tables 0` for this database even when the table is
there, measured 2026-09-03. **Ask the database, not the listing:**

```bash
npx wrangler d1 execute ulpia-subscribers --remote \
  --command "SELECT name FROM sqlite_master WHERE type='table';"
```

## 3. Deploy

**A push to `main` deploys this page. That is the whole mechanism.** The Pages project is
git-connected, so Cloudflare builds and publishes on its own when the branch moves.

Measured on 2026-08-21, not read from a dashboard screenshot:
`npx wrangler pages project list` returns `Git Provider: Yes` for `ulpia`, and
`npx wrangler pages deployment list --project-name ulpia` shows one production deployment
per commit on `main`, including the one that carried `9af0d57`.

**This section previously said the opposite**, that the deploy was a direct upload and that
no git remote or GitHub connection was required. That was true when it was written and stopped
being true without anybody updating the file, which is how a person following the runbook ends
up publishing twice by two different routes.

The manual upload still works and is the fallback when the git path is broken or when
publishing a tree that is deliberately not on `main`:

```bash
npx wrangler pages deploy dist --project-name ulpia
```

**Do not use both for the same change.** A direct upload and a git build are two publishing
paths into one origin, and the last one to finish wins regardless of which one you meant.

The first run creates the project and prints a `*.pages.dev` URL. Verify there before
attaching the domain: a broken deploy behind the real name is a worse minute than a
broken deploy behind a temporary one.

## 3b. The mail records, before the domain moves

**This bit the first deployment and will bite again on any zone that moves.** Moving
nameservers to Cloudflare moves DNS, and DNS is where mail delivery is decided. The
mail records for `ulpia.io` live at Hostinger and did not come across, so the domain
resolved with no MX at all, which means `hello@ulpia.io` could not receive anything:
not the Cloudflare verification message, and not the mail the page's only call to
action promises.

Recreate these in Cloudflare DNS, all of them **DNS only** (grey cloud, not proxied):
mail records cannot be proxied, and a proxied `autoconfig` breaks mail client setup.

| Type | Name | Value | Priority |
|------|------|-------|----------|
| MX | `@` | `mx1.hostinger.com` | 5 |
| MX | `@` | `mx2.hostinger.com` | 10 |
| TXT | `@` | `v=spf1 include:_spf.mail.hostinger.com ~all` | |
| TXT | `_dmarc` | `v=DMARC1; p=none` | |
| CNAME | `autoconfig` | `autoconfig.mail.hostinger.com` | |
| CNAME | `autodiscover` | `autodiscover.mail.hostinger.com` | |
| CNAME | `hostingermail-a._domainkey` | `hostingermail-a.dkim.mail.hostinger.com` | |
| CNAME | `hostingermail-b._domainkey` | `hostingermail-b.dkim.mail.hostinger.com` | |
| CNAME | `hostingermail-c._domainkey` | `hostingermail-c.dkim.mail.hostinger.com` | |

**The three `_domainkey` records are DKIM and they are not optional.** SPF says which
server may send for the domain; DKIM signs each message so a receiver can verify it
was not altered in transit and really came from us. Large mailbox providers now expect
both, and SPF alone breaks the moment a message is forwarded, because the forwarding
server is not in our SPF record while the DKIM signature still verifies. A domain that
sends without DKIM lands in spam and teaches the receiving side to keep putting it
there, which is a reputation this project needs intact before it ever mails a list.

**Every one of these is DNS only.** Cloudflare proxies CNAMEs by default, and a
proxied `_domainkey` returns Cloudflare's own addresses instead of the DKIM target,
which silently breaks signing.

**Do not recreate the old `A @` record, and do not recreate `www` by hand.** It pointed the apex at the previous web
host, and `www` pointed at the apex. Both belong to Pages now, and Cloudflare writes
them itself when the custom domains are attached.

Verify from outside rather than from the dashboard, because the dashboard shows
intent and DNS shows reality:

```bash
nslookup -type=mx ulpia.io 1.1.1.1
```

Then send a message to `hello@ulpia.io` from an unrelated address and confirm it
arrives. The page promises that address answers; nothing else on this list matters
if it does not.

## 4. The domain

In the Pages project: Custom domains, add `ulpia.io`, then add `www.ulpia.io` and set
it to redirect to the apex. The apex, not `www`, is canonical: one canonical origin
means one cache, one set of search results, and no cookie-scope surprises later.

**Measured 2026-09-03: `www` does not redirect.** It answers 200 with a byte-identical
copy of the apex, under its own certificate. The `<link rel="canonical">` on every page
points at the apex, which is what keeps search results from splitting, so this is a
runbook that describes a configuration the account does not have rather than a live
defect. Either set the redirect or change this paragraph; leaving both is how the next
person believes a redirect exists.

Cloudflare creates the DNS records itself when the zone is on its nameservers. **Leave
existing MX records alone**: `hello@ulpia.io` mail routing is independent of where the
website points, and the page is useless if the address in it stops answering.

## 5. Verify, before calling it done

**Send a browser's `User-Agent` or these checks lie to you.** Cloudflare decides what to
inject from the request, so a bare `curl` is served a different page from the one a
person gets. On 2026-09-03 a byte comparison of all twelve pages against a local build
passed while every one of them was shipping an analytics beacon to real browsers: plain
`curl` returned 19810 bytes, a browser `User-Agent` returned 20169. Set it once:

```bash
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
B=(-H "User-Agent: $UA" -H "Accept: text/html,application/xhtml+xml")

curl -sI https://ulpia.io/ | grep -i "content-security-policy\|cache-control\|x-content-type"
curl -s -o /dev/null -w "%{http_code}\n" https://ulpia.io/no-such-page   # expect 404
curl -s "${B[@]}" https://ulpia.io/ | grep -c "cdn-cgi"                  # expect 0
curl -s "${B[@]}" https://ulpia.io/ | grep -c "cloudflareinsights"       # expect 1
curl -s https://ulpia.io/ | grep -o 'mailto:[^"]*'                       # expect the real address
curl -s -o /dev/null -w "%{http_code}\n" https://ulpia.io/api/subscribe  # expect 405
curl -sI https://ulpia.io/assets/$(curl -s https://ulpia.io/ | grep -o 'assets/styles-[^"]*css' | head -1 | cut -d/ -f2) | grep -i cache-control
```

In order: the policy arrived; a wrong URL is honestly a 404; Cloudflare injected no
email obfuscation; the analytics beacon **is** there, because it is meant to be and a 0
means either the setting flipped off or the CSP is refusing it again; the CTA is still a
real `mailto`; the subscribe endpoint exists and answers a GET honestly rather than
falling through to the static 404; and hashed assets are immutable while the HTML is not.

Two of those catch a silently changed provider default, in both directions: `cdn-cgi`
expects 0 and `cloudflareinsights` expects 1. A check that only ever expects absence
cannot tell you a feature you rely on has been turned off.

Then open the page and press the lamp, and submit the form with an address you own.
Automated checks do not have eyes, and the write path is the one thing here that cannot
be verified from outside without writing: the endpoint answers 201 identically whether
the row was stored or the table was missing, deliberately, so that it is not an email
enumeration oracle. The count is the proof:

```bash
npx wrangler d1 execute ulpia-subscribers --remote --command "SELECT COUNT(*) FROM subscribers;"
```

## 6. When the server earns its place

**That moment already came once and this section did not get it**: `/api/subscribe` has
been computing per request since 2026-08-19, answered by a Pages Function. What is left
here is the case a Function cannot take: something that must hold state between requests,
run longer than an edge invocation allows, or ship a binary rather than a handler, such
as a live `kb route` or MCP endpoint.

When that lands, the binary moves to a VPS on a
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
git push origin main
```

That is it. Build locally first to see what you are about to publish, because the local build
is the only place you get to look before strangers do:

```bash
cd site/frontend && npm run build
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
