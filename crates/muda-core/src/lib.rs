//! Lossless JPEG/PNG container rewrite that strips identity metadata.
//!
//! Color profiles (`iCCP` / `sRGB` / JPEG ICC APP2 / Adobe APP14) are kept.
//! Compressed image scans are not re-encoded.

mod jpeg;
mod png;

use std::fmt;
use std::io::Cursor;

pub use jpeg::strip_jpeg;
pub use png::strip_png;

const JPEG_SOI: &[u8] = &[0xFF, 0xD8];
const PNG_SIGNATURE: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Jpeg,
    Png,
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
    pub kind: ImageKind,
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
            StripError::Unsupported => f.write_str("unsupported format (only JPEG and PNG in v1)"),
            StripError::InvalidImage(msg) => write!(f, "invalid image: {msg}"),
            StripError::Encode(msg) => write!(f, "encode failed: {msg}"),
        }
    }
}

impl std::error::Error for StripError {}

/// Detect JPEG (`FF D8`) or PNG signature. Truncated or unknown bytes return `None`.
pub fn sniff_kind(bytes: &[u8]) -> Option<ImageKind> {
    if bytes.starts_with(JPEG_SOI) {
        Some(ImageKind::Jpeg)
    } else if bytes.starts_with(PNG_SIGNATURE) {
        Some(ImageKind::Png)
    } else {
        None
    }
}

/// Strip identity metadata and return **new** bytes plus a report.
/// The input buffer is never mutated.
pub fn strip_image(name: &str, data: &[u8]) -> Result<(Vec<u8>, StripReport), StripError> {
    match sniff_kind(data) {
        Some(ImageKind::Jpeg) => strip_jpeg(name, data),
        Some(ImageKind::Png) => strip_png(name, data),
        None => Err(StripError::Unsupported),
    }
}

/// Reserved for a future UI that lets the user keep selected tag families.
/// v1 still strips all identity metadata and keeps color profiles.
pub fn strip_image_selective(
    name: &str,
    data: &[u8],
    _keep_families: &[TagFamily],
) -> Result<(Vec<u8>, StripReport), StripError> {
    strip_image(name, data)
}

pub(crate) fn cleaned_output_name(original: &str, kind: ImageKind) -> String {
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
        ImageKind::Jpeg => "jpg",
        ImageKind::Png => "png",
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
        exif::Tag::Make | exif::Tag::Model | exif::Tag::MakerNote => TagFamily::Camera,
        _ => TagFamily::Other,
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
        assert_eq!(sniff_kind(&[0xFF, 0xD8, 0xFF]), Some(ImageKind::Jpeg));
        assert_eq!(sniff_kind(PNG_SIGNATURE), Some(ImageKind::Png));
        assert_eq!(sniff_kind(&[]), None);
        assert_eq!(sniff_kind(&[0xFF]), None);
        assert_eq!(sniff_kind(b"not an image"), None);
        assert_eq!(sniff_kind(&[0x89, 0x50]), None);
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
            cleaned_output_name("photo.jpg", ImageKind::Jpeg),
            "photo.cleaned.jpg"
        );
        assert_eq!(
            cleaned_output_name("/tmp/vacation.PNG", ImageKind::Png),
            "vacation.cleaned.png"
        );
        assert_eq!(
            cleaned_output_name("noext", ImageKind::Jpeg),
            "noext.cleaned.jpg"
        );
    }
}
