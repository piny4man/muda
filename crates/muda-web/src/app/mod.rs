use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Link, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

mod chrome;
mod dropzone;
mod hero;
mod home;
mod io;
mod model;
mod queue;
mod server;
mod size;
mod strip;
mod toolbar;

pub use server::{health, upload_unsupported};

use home::HomePage;
use chrome::SiteFooter;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
                <link rel="icon" href="/favicon.svg" type="image/svg+xml"/>
                <meta name="theme-color" media="(prefers-color-scheme: light)" content="#F0F3EB"/>
                <meta name="theme-color" media="(prefers-color-scheme: dark)" content="#101309"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/muda-web.css"/>
        <Link
            rel="preload"
            href="/fonts/IBMPlexSans-Regular.woff2"
            as_="font"
            type_="font/woff2"
            crossorigin="anonymous"
        />
        <Title text="muda — metadata eraser"/>
        <Router>
            <main>
                <Routes fallback=|| view! { <p class="muted">"Page not found."</p> }.into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
        <SiteFooter/>
    }
}
