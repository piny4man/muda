#![recursion_limit = "512"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::extract::Request;
    use axum::http::{header, HeaderValue};
    use axum::middleware::{self, Next};
    use axum::response::Response;
    use axum::Router;
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use muda_web::app::*;
    use tower_http::trace::TraceLayer;
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let mut conf = get_configuration(None).expect("leptos configuration");
    conf.leptos_options.site_addr = bind_addr(conf.leptos_options.site_addr);
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    async fn security_headers(request: Request, next: Next) -> Response {
        let path = request.uri().path().to_string();
        let mut response = next.run(request).await;
        let headers = response.headers_mut();
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        headers.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );
        // HydrationScripts and font preload use CORS (`crossorigin`). Safari
        // (including iOS) refuses the WASM/JS/font fetch without ACAO, so the
        // page SSR-renders but never hydrates and cannot strip files.
        if path.starts_with("/pkg/") || path.starts_with("/fonts/") {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
        }
        if path.ends_with(".wasm") {
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            );
        } else if path.ends_with(".js") || path.ends_with(".css") {
            // Unhashed /pkg/*.css and /pkg/*.js; do not pin a stale sheet for an hour.
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-cache"),
            );
        }
        response
    }

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .layer(middleware::from_fn(security_headers))
        .layer(TraceLayer::new_for_http())
        .with_state(leptos_options);

    log!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    axum::serve(listener, app.into_make_service())
        .await
        .expect("server");
}

/// Railway (and most PaaS) inject `PORT` and route to `0.0.0.0`. Local
/// `cargo leptos watch` leaves `PORT` unset and keeps `LEPTOS_SITE_ADDR`.
#[cfg(feature = "ssr")]
fn bind_addr(configured: std::net::SocketAddr) -> std::net::SocketAddr {
    match std::env::var("PORT") {
        Ok(port) => match port.parse::<u16>() {
            Ok(port) => std::net::SocketAddr::from(([0, 0, 0, 0], port)),
            Err(_) => {
                tracing::warn!("PORT={port} is not a valid u16; using {configured}");
                configured
            }
        },
        Err(_) => configured,
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
