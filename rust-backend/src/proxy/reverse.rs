//! Reverse proxy to the Node/Next.js upstream.
//!
//! Ports Go's httputil.ReverseProxy: streams requests + responses (including SSE)
//! without buffering, propagates client cancellation. Used as the catch-all fallback
//! for any path Rust doesn't handle natively (dashboard pages, /api/*, static assets).

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use tracing::warn;

use crate::proxy::AppState;

/// Hop-by-hop headers that must not be forwarded (RFC 7230 §6.1).
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Catch-all reverse proxy handler. Forwards the entire request to the Node
/// upstream and streams the response back. SSE responses flush immediately
/// because we stream the body chunk-by-chunk.
pub async fn proxy_to_node(State(state): State<AppState>, req: Request<Body>) -> Response {
    let node_upstream = &state.node_upstream;
    let path_query = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    let target_url = format!("{node_upstream}{path_query}");

    let method = req.method().clone();
    let mut req_headers = req.headers().clone();
    // Strip hop-by-hop headers.
    for h in HOP_BY_HOP {
        req_headers.remove(*h);
    }
    // Force host to the upstream's host[:port].
    if let Ok(uri) = node_upstream.parse::<http::Uri>() {
        if let Some(host) = uri.host() {
            let host_val = match uri.port_u16() {
                Some(p) => format!("{host}:{p}"),
                None => host.to_string(),
            };
            if let Ok(hv) = HeaderValue::from_str(&host_val) {
                req_headers.insert("host", hv);
            }
        }
    }

    // Collect the request body (for non-streaming upstream calls this is fine;
    // dashboard API requests are small JSON bodies).
    let req_body = match axum::body::to_bytes(req.into_body(), 256 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            warn!("proxy: failed to read request body: {e}");
            return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
        }
    };

    // Build the upstream request.
    let req_method = Method::from_bytes(method.as_str().as_bytes()).unwrap_or(Method::GET);
    let upstream_req = state
        .client
        .request(req_method, &target_url)
        .headers(req_headers)
        .body(req_body);

    let upstream_resp = match upstream_req.send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(path = %path_query, "upstream proxy error: {e}");
            return (StatusCode::BAD_GATEWAY, "Node upstream unavailable").into_response();
        }
    };

    // Build the client response: copy status + headers, stream the body.
    let status = upstream_resp.status();
    let mut resp_headers = HeaderMap::new();
    for (name, value) in upstream_resp.headers() {
        if HOP_BY_HOP.contains(&name.as_str()) || name == "content-length" {
            continue;
        }
        // append (not insert): multiple Set-Cookie values must survive the hop.
        resp_headers.append(name.clone(), value.clone());
    }

    // Stream the response body chunk-by-chunk (critical for SSE).
    let stream = upstream_resp
        .bytes_stream()
        .map(|chunk| chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
    let body = Body::from_stream(stream);

    let mut resp = Response::new(body);
    *resp.status_mut() = status;
    *resp.headers_mut() = resp_headers;
    resp
}
