//! Build-time rendering of the static site.
//!
//! muda has no runtime server: every strip runs in the WASM client. The page
//! is rendered once at build time through the same leptos_axum pipeline that
//! used to serve it, so `<MetaTags/>`, `<Title/>`, and the hydration scripts
//! come out exactly as before. The request runs in-process; nothing binds a
//! port.

use axum::body::{to_bytes, Body};
use axum::http::Request;
use axum::Router;
use leptos::config::LeptosOptions;
use leptos_axum::{generate_route_list, LeptosRoutes};
use tower::ServiceExt;

use crate::app::{shell, App};

/// Render `path` to a full HTML document. Paths outside the router render the
/// "Page not found." fallback (used for `404.html`).
pub async fn render_page(options: LeptosOptions, path: &str) -> String {
    let routes = generate_route_list(App);
    let app = Router::new()
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(options);

    let request = Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("prerender request");
    let response = app.oneshot(request).await.expect("prerender response");
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("prerender body");
    String::from_utf8(bytes.to_vec()).expect("prerender html is utf-8")
}
