//! The server behind ulpia.io.
//!
//! It has one job today: serve the built landing page, small and fast. It is a Rust
//! binary rather than a static host because the site will grow server work, a blog,
//! subdomains, maybe a demo of the router itself, and this route table is where that
//! lands as a handler instead of a rehost.
//!
//! Three choices worth recording:
//!
//! 1. **Axum over a hand rolled std server.** The kb crate holds the line at one
//!    dependency because a linter parses brackets. This binary parses HTTP from the
//!    open internet, and a homemade HTTP parser on a public port is a vulnerability
//!    class, not a purity win. Same criterion as kb's SQLite comment, opposite verdict,
//!    because the input is hostile here.
//! 2. **The page is compressed on the fly, not precompressed.** The whole site is a
//!    few files measured in kilobytes; gzip at request time costs microseconds and
//!    keeps the build free of a second artifact set. Revisit if the asset set grows
//!    past what fits in the page cache.
//! 3. **TLS is not here.** Certificates belong to the reverse proxy (Caddy on the
//!    VPS), which renews them without this binary restarting. This process binds a
//!    loopback port and never sees a handshake.

use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, Response};
use axum::routing::get;
use axum::Router;
use std::net::{IpAddr, SocketAddr};
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

#[tokio::main]
async fn main() {
    // Where the built frontend lives. The default resolves when run from
    // `site/server/` in the repository; the VPS unit sets STATIC_DIR explicitly.
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/dist".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    if !std::path::Path::new(&static_dir).join("index.html").is_file() {
        eprintln!("no index.html under {static_dir}. Build the frontend first (npm run build in site/frontend), or point STATIC_DIR at a build.");
        std::process::exit(1);
    }

    // Unknown paths fall through to the 404 page, not to index.html. This is a site,
    // not an SPA: a wrong URL should say so instead of pretending to be the homepage,
    // and a real 404 status is what lets crawlers and link checkers do their job.
    // ServeFile is deliberately not used here: it would answer 200 with the 404
    // body, and honesty holds at the HTTP layer or the page is a decoration. The
    // body is read once at startup; it changes only on deploy, and a deploy
    // restarts the process.
    let page_404: &'static str = Box::leak(
        std::fs::read_to_string(format!("{static_dir}/404.html"))
            .unwrap_or_else(|_| "404 · nothing is shelved at this address".into())
            .into_boxed_str(),
    );
    let not_found = get(move || async move { (StatusCode::NOT_FOUND, Html(page_404)) });
    let files = ServeDir::new(&static_dir).not_found_service(not_found);

    let app = Router::new()
        // For the uptime monitor and the deploy script. Answers before the file
        // system is consulted, so it measures the process, not the disk.
        .route("/health", get(|| async { "ok" }))
        .fallback_service(files)
        .layer(middleware::from_fn(cache_policy))
        .layer(CompressionLayer::new())
        // Every header here is one attack surface closed by default; loosen them the
        // day a feature actually needs it, and name the feature. The named feature is
        // Cloudflare Web Analytics: Pages injects its beacon at the edge, so the two
        // cloudflareinsights.com entries are what let it run rather than be refused.
        // This string is the one in site/frontend/public/_headers, and the two are
        // meant to stay identical, so the page behaves the same whether this binary
        // or Pages is in front of it. The binary never sees the injection, since that
        // happens at Cloudflare's edge and not here, but a policy that differs by
        // deployment target is a policy nobody can test in one place.
        .layer(header_layer(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; style-src 'self'; script-src 'self' https://static.cloudflareinsights.com; img-src 'self'; font-src 'self'; connect-src 'self' https://cloudflareinsights.com; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
        ))
        .layer(header_layer(header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .layer(header_layer(header::REFERRER_POLICY, "no-referrer"))
        .layer(header_layer(
            // Not a named constant in the http crate, so it is spelled out.
            header::HeaderName::from_static("permissions-policy"),
            "camera=(), microphone=(), geolocation=()",
        ));

    // Loopback by default: in production Caddy is the only thing that should reach
    // this process, and in development the browser is on the same machine. Exposing
    // the naked port is a decision, so it takes an explicit HOST=0.0.0.0.
    let host: IpAddr = std::env::var("HOST")
        .ok()
        .and_then(|h| h.parse().ok())
        .unwrap_or_else(|| IpAddr::from([127, 0, 0, 1]));
    let addr = SocketAddr::from((host, port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("cannot bind {addr}: {e}"));
    eprintln!("serving {static_dir} on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            eprintln!("shutting down");
        })
        .await
        .expect("server error");
}

/// Cache what cannot change, revalidate what can.
///
/// Vite writes content hashes into every filename under `/assets/`, so those files
/// are immutable by construction: a change produces a new name, never a new body
/// under an old name. That is what makes `immutable` honest rather than optimistic.
/// The HTML keeps `no-cache`, which still allows conditional revalidation (a 304 on
/// an unchanged ETag), so repeat visits cost one small request and deploys are
/// visible on the next load instead of whenever a cache expires.
async fn cache_policy(req: Request<axum::body::Body>, next: Next) -> Response {
    let immutable = req.uri().path().starts_with("/assets/");
    let mut res = next.run(req).await;
    let value = if immutable {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    res
}

fn header_layer(
    name: header::HeaderName,
    value: &'static str,
) -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(name, HeaderValue::from_static(value))
}
