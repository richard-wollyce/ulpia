# Deploying ulpia.io

The site is two artifacts: a static build (`frontend/dist/`) and one binary
(`server/target/release/ulpia-site`) that serves it. TLS and the public port belong to
Caddy; the binary listens on loopback and never sees a certificate.

Why this shape: Caddy provisions and renews Let's Encrypt certificates on its own and
reloads without dropping connections, so certificate lifecycle never requires touching
or restarting our process. The alternative, TLS inside the binary with rustls, saves
one component and costs a restart on every renewal plus certificate code we would own.
Caddy is a single binary from a repository package. No Docker anywhere in this path.

## 1. Build on the VPS

Rust cross-compilation from Windows to Linux is possible and not worth its setup cost
for one binary. Build where it runs:

```bash
git clone <this repository> ulpia && cd ulpia/site
cd frontend && npm ci && npm run build && cd ..
cd server && cargo build --release && cd ..
```

Then place the artifacts (adjust paths to taste, the unit file below assumes these):

```bash
sudo mkdir -p /srv/ulpia
sudo cp server/target/release/ulpia-site /srv/ulpia/
sudo cp -r frontend/dist /srv/ulpia/dist
```

## 2. The service

`/etc/systemd/system/ulpia-site.service`:

```ini
[Unit]
Description=ulpia.io site
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

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now ulpia-site
curl -s http://127.0.0.1:8080/health   # expect: ok
```

## 3. Caddy

Append to `/etc/caddy/Caddyfile`:

```
ulpia.io {
    reverse_proxy 127.0.0.1:8080
}

www.ulpia.io {
    redir https://ulpia.io{uri} permanent
}
```

```bash
sudo systemctl reload caddy
```

Caddy obtains certificates for both names on first request. The apex, not `www`, is
canonical: one canonical origin means one cache, one set of search results, and no
cookie-scope surprises later.

## 4. DNS

At the registrar for `ulpia.io`:

| Type | Name | Value |
|------|------|-------|
| A    | `@`   | the VPS IPv4 |
| A    | `www` | the VPS IPv4 |
| AAAA | `@` and `www` | the VPS IPv6, if it has one |

Leave the existing MX records alone; `hello@ulpia.io` mail routing is independent of
where the website points.

Propagation is minutes to hours depending on prior TTLs. Verify with
`curl -sI https://ulpia.io/health` once the A record resolves.

## 5. Redeploying

A deploy is: rebuild, copy, restart.

```bash
cd ulpia && git pull
cd site/frontend && npm ci && npm run build && cd ..
cd server && cargo build --release && cd ..
sudo cp server/target/release/ulpia-site /srv/ulpia/
sudo rm -rf /srv/ulpia/dist && sudo cp -r frontend/dist /srv/ulpia/dist
sudo systemctl restart ulpia-site
```

Worth scripting the day it is run a third time, not before.
