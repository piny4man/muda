//! Identity-metadata strip for JPEG, PNG, WebP, and PDF.
//!
//! Image color profiles (`iCCP` / `sRGB` / JPEG ICC APP2 / Adobe APP14 / WebP ICCP) are kept.
//! Compressed image scans are not re-encoded.
//! A PDF is rebuilt as one generation so older revisions are not appended. Page content is not rendered.

mod exif_rewrite;
mod jpeg;
mod pdf;
mod png;
mod webp;

use std::fmt;
use std::io::Cursor;

const JPEG_SOI: &[u8] = &[0xFF, 0xD8];
const PNG_SIGNATURE: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Jpeg,
    Png,
    WebP,
    Pdf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagFamily {
    Gps,
    Camera,
    Software,
    Thumbnail,
    Xmp,
    Icc,
    Comment,
    Other,
}

impl TagFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            TagFamily::Gps => "GPS",
            TagFamily::Camera => "Camera",
            TagFamily::Software => "Software",
            TagFamily::Thumbnail => "Thumbnail",
            TagFamily::Xmp => "XMP",
            TagFamily::Icc => "ICC",
            TagFamily::Comment => "Comment",
            TagFamily::Other => "Other",
        }
    }
}

impl fmt::Display for TagFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedTag {
    pub family: TagFamily,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripReport {
    pub original_name: String,
    pub kind: FileKind,
    pub output_name: String,
    pub removed: Vec<RemovedTag>,
    pub warnings: Vec<String>,
    pub input_bytes: usize,
    pub output_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StripError {
    Unsupported,
    InvalidImage(String),
    Encode(String),
}

impl fmt::Display for StripError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StripError::Unsupported => {
                f.write_str("unsupported format (only JPEG, PNG, WebP, and PDF)")
            }
            StripError::InvalidImage(msg) => write!(f, "invalid image: {msg}"),
            StripError::Encode(msg) => write!(f, "encode failed: {msg}"),
        }
    }
}

impl std::error::Error for StripError {}

/// Detect JPEG (`FF D8`), PNG, WebP (`RIFF….WEBP`), or PDF (`%PDF-` in the first 1024 bytes).
/// Other RIFF is unsupported.
pub fn sniff_kind(bytes: &[u8]) -> Option<FileKind> {
    if bytes.starts_with(JPEG_SOI) {
        Some(FileKind::Jpeg)
    } else if bytes.starts_with(PNG_SIGNATURE) {
        Some(FileKind::Png)
    } else if is_webp(bytes) {
        Some(FileKind::WebP)
    } else if is_pdf(bytes) {
        Some(FileKind::Pdf)
    } else {
        None
    }
}

fn is_pdf(bytes: &[u8]) -> bool {
    let window = &bytes[..bytes.len().min(1024)];
    window.windows(5).any(|candidate| candidate == b"%PDF-")
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
}

/// Strip identity metadata and return **new** bytes plus a report.
/// The input buffer is never mutated.
pub fn strip_image(name: &str, data: &[u8]) -> Result<(Vec<u8>, StripReport), StripError> {
    strip_image_selective(name, data, &[])
}

/// Strip identity metadata, keeping only the requested tag families.
///
/// Keep is a whitelist. XMP, Photoshop IRB, thumbnails, and other EXIF fields
/// (dates, orientation, exposure) are always removed. Empty `keep_families`
/// matches [`strip_image`].
pub fn strip_image_selective(
    name: &str,
    data: &[u8],
    keep_families: &[TagFamily],
) -> Result<(Vec<u8>, StripReport), StripError> {
    match sniff_kind(data) {
        Some(FileKind::Jpeg) => jpeg::strip_jpeg(name, data, keep_families),
        Some(FileKind::Png) => png::strip_png(name, data, keep_families),
        Some(FileKind::WebP) => webp::strip_webp(name, data, keep_families),
        Some(FileKind::Pdf) => pdf::strip_pdf(name, data, keep_families),
        None => Err(StripError::Unsupported),
    }
}

/// Full strip (no families kept). Public wrapper around the format decoder.
pub fn strip_jpeg(name: &str, data: &[u8]) -> Result<(Vec<u8>, StripReport), StripError> {
    jpeg::strip_jpeg(name, data, &[])
}

pub fn strip_png(name: &str, data: &[u8]) -> Result<(Vec<u8>, StripReport), StripError> {
    png::strip_png(name, data, &[])
}

pub fn strip_webp(name: &str, data: &[u8]) -> Result<(Vec<u8>, StripReport), StripError> {
    webp::strip_webp(name, data, &[])
}

pub(crate) fn cleaned_output_name(original: &str, kind: FileKind) -> String {
    let file_name = original
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("image");
    let stem = match file_name.rfind('.') {
        Some(i) if i > 0 => &file_name[..i],
        _ => file_name,
    };
    let ext = match kind {
        FileKind::Jpeg => "jpg",
        FileKind::Png => "png",
        FileKind::WebP => "webp",
        FileKind::Pdf => "pdf",
    };
    format!("{stem}.cleaned.{ext}")
}

pub(crate) fn parse_exif_fields(data: &[u8]) -> Vec<RemovedTag> {
    let mut cursor = Cursor::new(data);
    let Ok(exifreader) = exif::Reader::new().read_from_container(&mut cursor) else {
        return Vec::new();
    };
    collect_exif_tags(&exifreader)
}

pub(crate) fn parse_exif_raw(tiff: &[u8]) -> Vec<RemovedTag> {
    let Ok(exifreader) = exif::Reader::new().read_raw(tiff.to_vec()) else {
        return Vec::new();
    };
    collect_exif_tags(&exifreader)
}

fn collect_exif_tags(exifreader: &exif::Exif) -> Vec<RemovedTag> {
    exifreader
        .fields()
        .map(|field| {
            let family = classify_exif_field(field);
            let value = field.display_value().to_string();
            let value = truncate(&value, 80);
            let label = if value.is_empty() {
                field.tag.to_string()
            } else {
                format!("{}={value}", field.tag)
            };
            RemovedTag { family, label }
        })
        .collect()
}

fn classify_exif_field(field: &exif::Field) -> TagFamily {
    if field.ifd_num == exif::In::THUMBNAIL {
        return TagFamily::Thumbnail;
    }
    let name = field.tag.to_string();
    if name.starts_with("GPS") {
        return TagFamily::Gps;
    }
    match field.tag {
        exif::Tag::Software => TagFamily::Software,
        exif::Tag::UserComment
        | exif::Tag::ImageDescription
        | exif::Tag::Artist
        | exif::Tag::Copyright => TagFamily::Comment,
        exif::Tag::Make
        | exif::Tag::Model
        | exif::Tag::MakerNote
        | exif::Tag::BodySerialNumber
        | exif::Tag::CameraOwnerName
        | exif::Tag::LensMake
        | exif::Tag::LensModel
        | exif::Tag::LensSerialNumber => TagFamily::Camera,
        _ => TagFamily::Other,
    }
}

pub(crate) fn drop_unkept(tags: Vec<RemovedTag>, keep: &[TagFamily]) -> Vec<RemovedTag> {
    tags.into_iter()
        .filter(|tag| !keep.contains(&tag.family))
        .collect()
}

pub(crate) fn xmp_dropped_warning() -> String {
    "XMP was dropped; duplicated tags in XMP were not kept".to_string()
}

pub(crate) fn maybe_makernote_warning(
    original: &[RemovedTag],
    keep: &[TagFamily],
    warnings: &mut Vec<String>,
) {
    if keep.contains(&TagFamily::Camera) && !keep.contains(&TagFamily::Gps) {
        let has_maker_note = original
            .iter()
            .any(|tag| tag.family == TagFamily::Camera && tag.label.contains("MakerNote"));
        if has_maker_note {
            warnings.push(
                "MakerNote kept with Camera; it may still contain location or serial data"
                    .to_string(),
            );
        }
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let clipped: String = s.chars().take(max).collect();
        format!("{clipped}…")
    }
}

pub(crate) fn latin1_lossy(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_jpeg_png_and_garbage() {
        assert_eq!(sniff_kind(&[0xFF, 0xD8, 0xFF]), Some(FileKind::Jpeg));
        assert_eq!(sniff_kind(PNG_SIGNATURE), Some(FileKind::Png));
        let mut webp = [0u8; 12];
        webp[..4].copy_from_slice(b"RIFF");
        webp[8..12].copy_from_slice(b"WEBP");
        assert_eq!(sniff_kind(&webp), Some(FileKind::WebP));
        assert_eq!(sniff_kind(b"RIFF\x00\x00\x00\x00WAVE"), None);
        assert_eq!(sniff_kind(&[]), None);
        assert_eq!(sniff_kind(&[0xFF]), None);
        assert_eq!(sniff_kind(b"not an image"), None);
        assert_eq!(sniff_kind(&[0x89, 0x50]), None);
        assert_eq!(sniff_kind(b"%PDF-1.7\n"), Some(FileKind::Pdf));
        let mut preamble = vec![b' '; 32];
        preamble.extend_from_slice(b"%PDF-1.4\n");
        assert_eq!(sniff_kind(&preamble), Some(FileKind::Pdf));
        let mut late = vec![0u8; 1020];
        late.extend_from_slice(b"%PDF-");
        assert_eq!(sniff_kind(&late), None);
    }

    #[test]
    fn strip_rejects_unsupported() {
        match strip_image("notes.txt", b"hello") {
            Err(StripError::Unsupported) => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn cleaned_names() {
        assert_eq!(
            cleaned_output_name("photo.jpg", FileKind::Jpeg),
            "photo.cleaned.jpg"
        );
        assert_eq!(
            cleaned_output_name("/tmp/vacation.PNG", FileKind::Png),
            "vacation.cleaned.png"
        );
        assert_eq!(
            cleaned_output_name("noext", FileKind::Jpeg),
            "noext.cleaned.jpg"
        );
        assert_eq!(
            cleaned_output_name("chat.webp", FileKind::WebP),
            "chat.cleaned.webp"
        );
        assert_eq!(
            cleaned_output_name("notes.pdf", FileKind::Pdf),
            "notes.cleaned.pdf"
        );
    }
}
