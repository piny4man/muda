<p align="center">
  <img src="assets/logo.svg" alt="muda" width="96" height="96" />
</p>

<h1 align="center">muda · 無駄</h1>

<p align="center">
  <strong>Metadata eraser</strong> for JPEG, PNG, WebP, and PDF.<br />
  Files are processed in your browser. Nothing is uploaded.
</p>

**muda** (無駄, “waste”) strips identity metadata from images and PDFs — GPS, camera, software, comments, XMP, Photoshop IRB, embedded thumbnails, PDF info and trailer IDs — and leaves the pixels and page text alone. GPS, camera, software, and comments can be kept if you check them; XMP and thumbnails always go.

Color profiles stay (`iCCP` / `sRGB` / JPEG ICC / Adobe APP14 / WebP `ICCP`), so colors do not shift. The compressed image data is copied, not decoded and re-encoded.

## Formats

| Format | In the browser |
| --- | --- |
| JPEG | yes |
| PNG | yes |
| WebP | yes |
| PDF | yes. Text stays. Info, XMP, the trailer ID, and tags inside embedded JPEGs are stripped. Older revisions are dropped. Encrypted files are rejected. |
| HEIC, RAW, TIFF, video, audio | no |

## How it works

Drop files, optionally keep selected tag families, **Strip**, download. Work runs in WASM (`muda-core`) inside the page. There is no server: the site is static HTML, CSS, and WASM on Cloudflare Pages.

Lossless **container rewrite** with [`img-parts`](https://crates.io/crates/img-parts) for images. PDFs are rebuilt in one generation with [`lopdf`](https://crates.io/crates/lopdf). [`kamadak-exif`](https://crates.io/crates/kamadak-exif) is read-only, for the per-file report of what left. Kept EXIF families are rewritten at the TIFF field level so GPS, camera, software, and comments can stay independently.

- **JPEG** — drop XMP, Photoshop APP13, other metadata APPn, and unkept EXIF fields. Keep JFIF APP0, ICC APP2, Adobe APP14. Keep COM / selected EXIF families when asked.
- **PNG** — keep `IHDR`, `PLTE`, `IDAT`, `IEND`, and color/correctness chunks (`tRNS`, `gAMA`, `cHRM`, `sRGB`, `iCCP`, `pHYs`, …). Drop `tIME`. Keep `eXIf` / text chunks only for selected families.
- **WebP** — always drop `XMP `. Keep `VP8` / `VP8L` / `VP8X` / `ALPH` / `ANIM` / `ANMF` / `ICCP`. Rewrite or drop `EXIF` to match the keep list.
- **PDF** — rebuild one generation with [`lopdf`](https://crates.io/crates/lopdf). Drop info (except Title), XMP, trailer `/ID`, incremental history, embedded files, JavaScript, and signatures. Run the JPEG stripper on `/DCTDecode` images. Page content is not rendered. Encrypted files are rejected.

## Workspace

```
crates/
  muda-core/   format sniff + JPEG/PNG/WebP rewrite and PDF rebuild
  muda-web/    Leptos UI: WASM client + build-time prerender of index.html
```

License: MIT OR Apache-2.0.

## Run locally

```sh
cargo install cargo-leptos --locked
rustup target add wasm32-unknown-unknown
(cd crates/muda-web && cargo leptos build)
./target/debug/muda-web            # prerender index.html + 404.html into target/site
npx wrangler pages dev target/site # serve like Cloudflare Pages, with _headers
```

Re-run the first two commands after changes.

Open http://127.0.0.1:8788/. Drop JPEG/PNG/WebP/PDF, **Strip**, download a cleaned file or a ZIP.

## Build and deploy

```sh
cd crates/muda-web && cargo leptos build --release && cd ../..
./target/release/muda-web   # prerender into target/site
```

`target/site` is the whole site. Response headers (nosniff, no-referrer, CORS and `no-cache` on `/pkg`, CORS on `/fonts`) live in `crates/muda-web/public/_headers`.

`.github/workflows/deploy.yml` tests, builds, prerenders, and runs `wrangler pages deploy` to the `muda` Pages project: `main` to production, PR branches to previews. It needs repository secrets `CLOUDFLARE_API_TOKEN` (Cloudflare Pages: Edit) and `CLOUDFLARE_ACCOUNT_ID`, and the project created once with `npx wrangler pages project create muda --production-branch main`. The custom subdomain is a CNAME to the project's `*.pages.dev` host at the external DNS provider.

## Tests

```sh
cargo test -p muda-core
cargo test -p muda-web --features ssr   # prerendered HTML
```

Fixtures live in `crates/muda-core/tests/fixtures/`. Regenerate with:

```sh
python3 crates/muda-core/tests/generate_fixtures.py
```
