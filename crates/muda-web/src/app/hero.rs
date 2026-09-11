use leptos::prelude::*;

#[component]
pub(crate) fn LogoMark() -> impl IntoView {
    view! {
        <svg
            class="logo-mark"
            viewBox="0 0 32 32"
            fill="currentColor"
            aria-hidden="true"
            focusable="false"
        >
            <path fill-rule="evenodd" d="M0 0h32v32H0zm5 5v22h22V5z"></path>
            <rect x="-6.5" y="13.5" width="45" height="5" transform="rotate(45 16 16)"></rect>
        </svg>
    }
}

#[component]
pub(crate) fn Hero() -> impl IntoView {
    view! {
        <header class="hero">
            <p class="brand">
                <LogoMark/>
                <span class="brand-word">"MUDA"</span>
            </p>
            <h1>"Metadata eraser"</h1>
            <p class="claim">
                "JPEG and PNG are processed in your browser. Files are not uploaded."
            </p>
        </header>
    }
}
