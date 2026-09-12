<p align="center">
  <img src="assets/logo.svg" alt="muda" width="96" height="96" />
</p>

<h1 align="center">muda · 無駄</h1>

<p align="center">
  <strong>Metadata eraser</strong> for JPEG, PNG, and WebP.<br />
  Files are processed in your browser. Nothing is uploaded.
</p>

**muda** (無駄, “waste”) strips identity metadata from images — GPS, camera, software, comments, XMP, Photoshop IRB, embedded thumbnails — and leaves the pixels alone.

Color profiles stay (`iCCP` / `sRGB` / JPEG ICC / Adobe APP14 / WebP `ICCP`), so colors do not shift. The compressed image data is copied, not decoded and re-encoded.

## Formats

| Format | In the browser |
| --- | --- |
| JPEG | yes |
| PNG | yes |
| WebP | yes |
| HEIC, RAW, TIFF, PDF, video, audio | no |

## How it works

Drop files, **Strip**, download. Work runs in WASM (`muda-core`) inside the page. The server only ships HTML, CSS, and WASM.

Lossless **container rewrite** with [`img-parts`](https://crates.io/crates/img-parts). [`kamadak-exif`](https://crates.io/crates/kamadak-exif) is read-only, for the per-file report of what left.

- **JPEG** — drop APP1 (Exif / XMP), COM, Photoshop APP13, other metadata APPn. Keep JFIF APP0, ICC APP2, Adobe APP14.
- **PNG** — keep `IHDR`, `PLTE`, `IDAT`, `IEND`, and color/correctness chunks (`tRNS`, `gAMA`, `cHRM`, `sRGB`, `iCCP`, `pHYs`, …). Drop `eXIf`, `tEXt`, `zTXt`, `iTXt`, `tIME`.
- **WebP** — drop `EXIF` and `XMP ` chunks. Keep `VP8` / `VP8L` / `VP8X` / `ALPH` / `ANIM` / `ANMF` / `ICCP`.

## Workspace

```
crates/
  muda-core/   format sniff + lossless JPEG/PNG/WebP rewrite
  muda-web/    Leptos SSR + Axum shell, WASM client
```

License: MIT OR Apache-2.0.

## Run locally

```sh
cargo install cargo-leptos --locked
rustup target add wasm32-unknown-unknown
cd crates/muda-web
cargo leptos watch
```

Open http://127.0.0.1:3000/. Drop JPEG/PNG/WebP, **Strip**, download a cleaned file or a ZIP.

## Tests

```sh
cargo test -p muda-core
```

Fixtures live in `crates/muda-core/tests/fixtures/`. Regenerate with:

```sh
python3 crates/muda-core/tests/generate_fixtures.py
```
