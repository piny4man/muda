#![recursion_limit = "512"]

//! Writes the prerendered pages into the site root produced by
//! `cargo leptos build`. The result is a static site (HTML + /pkg + /fonts +
//! `_headers`) deployed to Cloudflare Pages. There is no runtime server.

#[cfg(feature = "ssr")]
#[tokio::main(flavor = "current_thread")]
async fn main() {
    use leptos::config::get_configuration;
    use muda_web::prerender::render_page;
    use std::path::PathBuf;

    let conf = get_configuration(None).expect("leptos configuration");
    let options = conf.leptos_options;
    let site_root = PathBuf::from(options.site_root.as_ref());
    std::fs::create_dir_all(&site_root)
        .unwrap_or_else(|e| panic!("create {}: {e}", site_root.display()));

    for (path, file) in [("/", "index.html"), ("/404", "404.html")] {
        let html = render_page(options.clone(), path).await;
        let out = site_root.join(file);
        std::fs::write(&out, html).unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
        println!("prerendered {path} -> {}", out.display());
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
