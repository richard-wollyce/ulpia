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

Four Cloudflare settings decide what a visitor actually receives. Each is one toggle,
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

**Browser Cache TTL: Respect Existing Headers.** Caching, then Configuration. Any other
value makes Cloudflare rewrite the `Cache-Control` this repository sets, on the way out,
where no build can see it. It was four hours until 2026-09-03, which meant a returning
visitor held `theme.js` and six other unhashed files for four hours after every deploy,
reachable by nothing: not a deploy, not a purge. Section 5b has the measurement and the
check.

Re-check all four after launch. A default that flips silently would falsify a claim we
made in writing, which is worse than never having made it, and two of them already did:
Web Analytics was on while the footer said "no analytics", and Browser Cache TTL was
overriding the repository's own caching policy. Both found on 2026-09-03, both by
measuring the apex rather than the build.

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

## 5b. Cache, and what happens when a URL is withdrawn

Everything in this section was measured against this account on 2026-09-03, and the
figures are the reason it exists: the runbook had no cache section until a withdrawn URL
kept answering 200 for hours and nobody could say which layer was holding it.

### Three classes, and only one of them can go stale

| What | What the visitor is told | Where it can go stale |
|---|---|---|
| HTML | `max-age=0, must-revalidate`, `cf-cache-status: DYNAMIC` | Nowhere. It is never edge cached here and it revalidates every load |
| `/assets/*` | `max-age=31536000, immutable` | Nowhere, and the filename is why: the hash changes with the bytes, so the old name is simply never requested again |
| The unhashed root files | `max-age=0, must-revalidate` | Nowhere, since 2026-09-03. It used to be four hours, and that is worth reading |

**The setting that decides the third row is Browser Cache TTL, and it is the one dial here
that can silently override this repository.** Until 2026-09-03 it was set to four hours, and
the effect was invisible from the build: `theme.js`, `anim.js`, `nav.js`, `field.js`,
`subscribe.js`, `favicon.svg` and `blog/feed.css` came back from the apex as
`public, max-age=14400, must-revalidate` while the same files on a `*.pages.dev` URL came
back `max-age=0`. The zone was rewriting them on the way out, so only the apex figure ever
reached a browser and only the apex could reveal it.

**Why that mattered more than it looks.** A returning visitor held those seven files for
four hours without asking again. No deploy evicted them and no purge could have, because the
browser makes no request to be answered. It is the only layer on this site that neither a
deploy nor a purge reaches.

It is now **Respect Existing Headers**, so Cloudflare stops overriding and the file that
already states the intent, `public/_headers`, is what reaches the visitor. That is safe here
for a measured reason rather than a hopeful one: every response this origin serves already
carries an explicit `Cache-Control`, nine of nine checked, so nothing falls back to a
browser's own heuristics. `/assets/*` keeps its year because we set that ourselves, and a
year is safe there and only there, since the filename carries a content hash.

The cost, named: a returning visitor now makes one conditional request per unhashed file per
page load. Seven small files, answered `304` with no body. That is the price of a deploy
actually reaching people who have already visited.

**Check it after any change to caching in the dashboard**, because this is exactly the kind
of setting that gets flipped and forgotten:

```bash
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
curl -sI -H "User-Agent: $UA" https://ulpia.io/theme.js | grep -i cache-control
# expect: public, max-age=0, must-revalidate
# a max-age of 14400 means Browser Cache TTL is overriding us again
```

### Withdrawing a URL is two steps, and the second is not optional

A slug change withdraws a URL. `tools/build-posts.mjs` wipes `blog/` on every run, so the
old page leaves the build the moment the file is renamed, and the origin starts answering
404 there immediately. That is not the end of it.

**Add the redirect in the same commit that renames the file, and write two lines, not
one.** Pages matches a `_redirects` source literally, so a rule ending in a slash does not
cover the same URL without one. That gap shipped once and left the slash-less form
answering 404 for half a day after the fix was called done.

```
/blog/<old-slug>/  /blog/<new-slug>/  301
/blog/<old-slug>   /blog/<new-slug>/  301
```

`_redirects` comes out of `public/` untouched, like `_headers`, so confirm it survived the
build in the same shell as section 2:

```bash
test -f dist/_redirects && echo "redirects present"
```

**Do not reach for a purge.** Three reasons, in the order they matter. A purge turns the
URL into a 404, which breaks every link already in a chat, a feed reader or a history,
while the actual worry, two URLs serving one article, is what 301 exists to consolidate
and deletion does not. A purge cannot be run from a developer machine here anyway:
`npx wrangler purge --help` exits 1, wrangler has no cache purge command, and the stored
OAuth credential carries no purge scope, so it is a dashboard action or nothing. And on
the one occasion it would have been reached for, measurement said the tier a purge
addresses was not holding the object.

### Diagnosing a URL that answers when it should not

Point these at an HTML URL. `/assets/*` and the root `.js` and `.svg` files are cached on
purpose and look nothing like the shapes below.

```bash
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
SUS=https://ulpia.io/blog/<the-withdrawn-slug>/

curl -sI -H "User-Agent: $UA" "$SUS" | grep -iE '^(HTTP|cache-control|age|x-robots-tag|cf-cache-status|cf-ray)'
curl -sI -H "User-Agent: $UA" https://ulpia.io/ | grep -iE '^(cache-control|age|cf-cache-status)'   # the control
```

Read them together, because no single header decides it:

- **A live page here** is `200`, `max-age=0, must-revalidate`, `DYNAMIC`, **no `Age`**.
- **An honest 404** is `404`, `no-store`, `DYNAMIC`, **no `Age`**.
- **An `Age` that climbs across polls** means a stored copy answered you, and on this
  account no HTML URL carries one at all. That header appearing on HTML is the anomaly.

**Do not confirm a fix by the absence of `Age`.** A 404 has no `Age` either, so an empty
grep proves nothing on its own. Assert on the status line and the `Location`, and poll six
times, because one poll is one Cloudflare location: read the colo off `CF-RAY` and say
which one you measured when you report a URL fixed.

**Rule out propagation before anything cache shaped.** They look alike from the apex and
they are not alike. Propagation is the apex still serving the previous build while the new
deployment already serves the new one at its own URL; it clears itself in minutes. Ask the
build directly, where there is no zone and no custom domain in the way:

```bash
DEP=$(npx wrangler pages deployment list --project-name ulpia | grep -o 'https://[0-9a-f]*\.ulpia\.pages\.dev' | head -1)
curl -sI -H "User-Agent: $UA" "$DEP/blog/<the-withdrawn-slug>/" | head -1
```

If that is a 404 and the apex is a 200, the origin has nothing there and something in
front of it is answering. If it is a 301, the redirect shipped and the apex will follow.

### What the incident recorded, and what it did not establish

The withdrawn URL carried `Cache-Control: public, s-maxage=604800`, an `Age` past 2400 and
climbing, and `x-robots-tag: noindex`, none of which this repository emits and none of
which any live page carries. Three consecutive production builds answered a hard 404 at
that path on their own URLs throughout. Six deploys did not evict it. A 301 ended it in
one deploy, with no purge.

**The cause was never established, and the first answer written down was wrong.** It was
read as Cloudflare Always Online, and Cloudflare documents Always Online as serving an
archive when it cannot reach the origin at all, a 520 to 527. A 404 is a successful
connection. So that reading is contradicted by the vendor's own documentation and is not
recorded here as fact. Settling it needs a dashboard read of Caching, Configuration, which
is a login and therefore Richard's hands. **The redirect works without a theory of the
cause, which is the reason to prefer it.**

### Two things this section does not cover

**A rollback deletes redirects.** `_redirects` ships inside the build artifact, so
promoting a deployment older than the commit that added a rule removes that rule, and the
withdrawn URL becomes an origin 404 again. Check for it before promoting.

**Section 6's VPS path has no redirects at all.** `site/server/src/main.rs` is a `/health`
route, a static service with a 404 fallback, a cache middleware and four header layers, and
that is the whole router. `_redirects` is read by Cloudflare Pages and by nothing else, so
every line of it has to be reimplemented the day the site moves.

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
