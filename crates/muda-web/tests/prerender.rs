#![cfg(feature = "ssr")]

use leptos::config::LeptosOptions;
use muda_web::prerender::render_page;

fn options() -> LeptosOptions {
    LeptosOptions::builder()
        .output_name("muda-web")
        .site_pkg_dir("pkg")
        .build()
}

#[tokio::test]
async fn index_is_a_hydratable_document() {
    let html = render_page(options(), "/").await;

    assert!(html.starts_with("<!DOCTYPE html>"), "doctype: {html}");
    assert!(
        html.contains("<title>muda — metadata eraser</title>"),
        "title missing: {html}"
    );
    assert!(html.contains("/pkg/muda-web.js"), "hydration js missing");
    // cargo-leptos names the file muda-web.wasm; bare LeptosOptions default
    // to wasm-bindgen's muda-web_bg.wasm. Either way it lives under /pkg.
    assert!(
        html.contains("/pkg/muda-web.wasm") || html.contains("/pkg/muda-web_bg.wasm"),
        "hydration wasm missing"
    );
    assert!(html.contains("/pkg/muda-web.css"), "stylesheet missing");
    assert!(html.contains("id=\"file-input\""), "dropzone not rendered");
    assert!(
        !html.contains("WebSocket"),
        "dev auto-reload leaked into build"
    );
}

#[tokio::test]
async fn unknown_path_renders_not_found() {
    let html = render_page(options(), "/404").await;

    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("Page not found."), "fallback missing: {html}");
    assert!(html.contains("/pkg/muda-web.js"));
}
