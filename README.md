# muda

Privacy-oriented **metadata eraser** for JPEG, PNG, and WebP. Full-stack Rust (Leptos + Axum + WASM).

**JPEG, PNG, and WebP are processed in your browser. Files are not uploaded.**

v1 never sends image bytes to the server. `muda-core::strip_image` runs in the hydrated WASM client. The Axum process only serves HTML/WASM/CSS and two stubs (`health`, a 501 for a future unsupported-format upload).

## Workspace

```
muda/
  crates/
    muda-core/    # format sniff + lossless JPEG/PNG/WebP rewrite
    muda-web/     # Leptos 0.8 SSR + Axum 0.8 shell
```

License: MIT OR Apache-2.0.

## Formats

| Format | v1 |
| --- | --- |
| JPEG | yes, in the browser |
| PNG | yes, in the browser |
| WebP | yes, in the browser |
| HEIC, RAW, TIFF, PDF, video, audio | no |

Color profiles (`iCCP` / `sRGB` / JPEG ICC APP2 / Adobe APP14 / WebP `ICCP`) are **kept** so pixels do not shift. GPS, camera, software, comments, XMP, Photoshop IRB, and embedded thumbnails are removed.

## JPEG / PNG / WebP strategy

Lossless **container rewrite** with [`img-parts`](https://crates.io/crates/img-parts) `0.4`. Compressed scans / `IDAT` / VP8 payloads are copied, not decoded to RGB and re-encoded. [`kamadak-exif`](https://crates.io/crates/kamadak-exif) `0.6` is read-only, for the per-file report.

- JPEG: drop APP1 (Exif / XMP), COM, Photoshop APP13, and other metadata APPn. Keep a JFIF APP0 (or write a minimal one), ICC APP2, and Adobe APP14.
- PNG: rebuild the file with `IHDR`, `PLTE`, `IDAT`, `IEND`, plus correctness/color chunks (`tRNS`, `gAMA`, `cHRM`, `sRGB`, `iCCP`, `pHYs`, …). Drop `eXIf`, `tEXt`, `zTXt`, `iTXt`, `tIME`, and other ancillary metadata. CRCs are those of the kept chunks.
- WebP: drop `EXIF` and `XMP ` chunks and clear those VP8X flags. Keep `VP8` / `VP8L` / `VP8X` / `ALPH` / `ANIM` / `ANMF` / `ICCP`.

## Crate versions

Resolved from this workspace (see `Cargo.lock`):

| Crate | Version |
| --- | --- |
| leptos | 0.8.20 |
| leptos_axum | 0.8.10 |
| leptos_meta | 0.8.6 |
| leptos_router | 0.8.15 |
| axum | 0.8.9 |
| tokio | 1.x |
| tower-http | 0.6 |
| img-parts | 0.4.0 |
| kamadak-exif | 0.6.1 |
| wasm-bindgen | 0.2.x |

Rust: **stable** (`rust-toolchain.toml`), target `wasm32-unknown-unknown`.

## Run

Install the Leptos CLI once:

```sh
cargo install cargo-leptos --locked
rustup target add wasm32-unknown-unknown
```

From the web crate:

```sh
cd crates/muda-web
cargo leptos watch
```

Then open http://127.0.0.1:3000/.

### Use the page

1. Drop JPEG/PNG/WebP files (or click the dropzone). Multiple files are fine. A `.txt` or HEIC stays in the list as **unsupported** — it is not uploaded.
2. Previews are local object URLs. Nothing is sent to the server.
3. **Strip** one row, or **Strip all**. Work runs in the browser (two at a time).
4. **Download** a cleaned `{stem}.cleaned.jpg` / `.png` / `.webp`, or **Download all (ZIP)**.
5. **Clear** / **Remove** revokes the object URLs.

Color profile is kept; GPS/camera/comments/XMP go away. Large files stay in the queue with a memory warning; there is no hard size cap.

Release binary + `target/site` assets:

```sh
cd crates/muda-web
cargo leptos build --release
```

## Deploy (Railway SSR)

v1 is **SSR Axum + a WASM client**. JPEG/PNG/WebP bytes never leave the browser. Railway runs the Axum process from the root `Dockerfile` (auto-detected). Do not put this binary on Vercel Functions or Cloudflare Workers.

### Railway

First build is slow (Rust + `wasm32`). Railway injects `PORT`; the server binds `0.0.0.0:$PORT`.

```sh
# from the repo root, after `railway login`
railway up
```

Or connect the GitHub repo in the Railway dashboard. You should see `Using detected Dockerfile!` in the build log.

Generate a `*.up.railway.app` URL, then attach the subdomain (DNS stays at your current registrar — no Cloudflare nameserver move):

```sh
railway domain
railway domain muda.pinya.dev
```

The second command prints the CNAME to create at your DNS host, for example:

| Type | Name | Target |
| --- | --- | --- |
| CNAME | `muda` | the hostname Railway prints |

Set a health check path of `/` in the service settings if Railway does not pick it up. No extra env vars are required beyond `PORT`.

Local image (same as Railway):

```sh
docker build -t muda-web .
docker run --rm -p 8080:8080 muda-web
```

Then open http://127.0.0.1:8080/.

### Other hosts

| Host | Fits this repo today? |
| --- | --- |
| **Railway / Fly / any Docker host** | Yes — this Dockerfile |
| **Cloudflare Pages / Vercel static** | Only after a separate CSR/Trunk build |
| **Vercel Functions / Cloudflare Workers** | No — this is a tokio Axum binary |

Do **not** enable an image-upload API in v1. That would break the product claim.

## Tests

```sh
cargo test -p muda-core
cargo test -p muda-core --target wasm32-unknown-unknown --no-run
```

Fixtures live in `crates/muda-core/tests/fixtures/` (tiny JPEG with EXIF GPS + planted `SECRET_TAG_XYZ`, plain JPEG, PNG with `tEXt`/`eXIf`, WebP with EXIF/XMP, ICC, and animation). Regenerate with:

```sh
python3 crates/muda-core/tests/generate_fixtures.py
```

## Privacy model

1. Drop or pick files. Previews use `URL.createObjectURL` on the original locally.
2. Strip runs in WASM via `muda-core` (concurrency 2, cooperative yield). No `#[server]` function takes image `Vec<u8>`.
3. Download one file as a blob, or all as a ZIP built in WASM (store method, no extra ZIP crate).
4. Object URLs are revoked when a row is removed or the list is cleared.

There is no hard file-size cap. Files at or above 50 MiB, and queues at or above 256 MiB of originals, show a memory warning; Strip still runs.
