use image::GenericImageView;
use img_parts::jpeg::{markers, Jpeg};
use img_parts::png::Png;
use img_parts::webp::{WebP, CHUNK_EXIF, CHUNK_XMP};
use img_parts::Bytes;
use muda_core::{strip_image, strip_image_selective, FileKind, TagFamily};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn contains_ascii(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn assert_decodes(bytes: &[u8]) -> image::DynamicImage {
    image::load_from_memory(bytes).expect("image crate should decode rewritten bytes")
}

fn jpeg_app1_tiff(bytes: &[u8]) -> Option<Vec<u8>> {
    let jpeg = Jpeg::from_bytes(Bytes::copy_from_slice(bytes)).expect("parse jpeg");
    for seg in jpeg.segments_by_marker(markers::APP1) {
        let contents = seg.contents();
        if contents.starts_with(b"Exif\0\0") {
            return Some(contents[6..].to_vec());
        }
    }
    None
}

fn jpeg_has_com(bytes: &[u8]) -> bool {
    let jpeg = Jpeg::from_bytes(Bytes::copy_from_slice(bytes)).expect("parse jpeg");
    let has = jpeg.segments_by_marker(markers::COM).next().is_some();
    has
}

fn png_exif_tiff(bytes: &[u8]) -> Option<Vec<u8>> {
    let png = Png::from_bytes(Bytes::copy_from_slice(bytes)).expect("parse png");
    png.chunks()
        .iter()
        .find(|c| &c.kind() == b"eXIf")
        .map(|c| c.contents().to_vec())
}

fn png_text_chunks(bytes: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    let png = Png::from_bytes(Bytes::copy_from_slice(bytes)).expect("parse png");
    png.chunks()
        .iter()
        .filter(|c| &c.kind() == b"tEXt" || &c.kind() == b"zTXt" || &c.kind() == b"iTXt")
        .map(|c| (c.kind(), c.contents().to_vec()))
        .collect()
}

fn webp_exif_tiff(bytes: &[u8]) -> Option<Vec<u8>> {
    let webp = WebP::from_bytes(Bytes::copy_from_slice(bytes)).expect("parse webp");
    let data = webp.chunk_by_id(CHUNK_EXIF)?.content().data()?.clone();
    if data.starts_with(b"Exif\0\0") {
        Some(data[6..].to_vec())
    } else {
        Some(data.to_vec())
    }
}

fn webp_has_xmp(bytes: &[u8]) -> bool {
    let webp = WebP::from_bytes(Bytes::copy_from_slice(bytes)).expect("parse webp");
    webp.chunk_by_id(CHUNK_XMP).is_some()
}

fn exif_tag_names(tiff: &[u8]) -> Vec<String> {
    let reader = exif::Reader::new()
        .read_raw(tiff.to_vec())
        .expect("parse rewritten EXIF");
    reader.fields().map(|f| f.tag.to_string()).collect()
}

fn has_tag(names: &[String], needle: &str) -> bool {
    names.iter().any(|n| n == needle || n.starts_with(needle))
}

fn removed_families(report: &muda_core::StripReport) -> Vec<TagFamily> {
    let mut families = Vec::new();
    for tag in &report.removed {
        if !families.contains(&tag.family) {
            families.push(tag.family);
        }
    }
    families
}

#[test]
fn empty_keep_matches_full_strip_jpeg() {
    let input = fixture("jpeg_gps.jpg");
    let (full, _) = strip_image("vacation.jpg", &input).expect("strip");
    let (selective, report) =
        strip_image_selective("vacation.jpg", &input, &[]).expect("selective");
    assert_eq!(full, selective, "empty keep must match strip_image");
    assert!(jpeg_app1_tiff(&selective).is_none());
    assert!(!jpeg_has_com(&selective));
    assert!(removed_families(&report).contains(&TagFamily::Gps));
    assert!(removed_families(&report).contains(&TagFamily::Comment));
}

#[test]
fn jpeg_keep_comment_preserves_com_drops_identity_exif() {
    let input = fixture("jpeg_gps.jpg");
    let original = assert_decodes(&input);
    let (output, report) =
        strip_image_selective("vacation.jpg", &input, &[TagFamily::Comment]).expect("keep comment");

    assert!(jpeg_has_com(&output), "JPEG COM must stay");
    assert!(contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(!contains_ascii(&output, b"Canon"));
    assert!(!contains_ascii(&output, b"Adobe"));
    assert!(!removed_families(&report).contains(&TagFamily::Comment));
    assert!(removed_families(&report).contains(&TagFamily::Gps));
    assert!(removed_families(&report).contains(&TagFamily::Camera));
    assert!(removed_families(&report).contains(&TagFamily::Software));

    let tiff = jpeg_app1_tiff(&output);
    if let Some(tiff) = tiff {
        let names = exif_tag_names(&tiff);
        assert!(
            !has_tag(&names, "GPSLatitude"),
            "GPS IFD must be gone: {names:?}"
        );
        assert!(!has_tag(&names, "Make"), "Make must be gone: {names:?}");
        assert!(
            !has_tag(&names, "Software"),
            "Software must be gone: {names:?}"
        );
        assert!(
            has_tag(&names, "ImageDescription"),
            "Comment EXIF ImageDescription must stay: {names:?}"
        );
    }

    let cleaned = assert_decodes(&output);
    assert_eq!(original.dimensions(), cleaned.dimensions());
}

#[test]
fn jpeg_keep_gps_preserves_gps_ifd_only() {
    let input = fixture("jpeg_gps.jpg");
    let (output, report) =
        strip_image_selective("vacation.jpg", &input, &[TagFamily::Gps]).expect("keep gps");

    assert!(!jpeg_has_com(&output), "COM must still be stripped");
    assert!(!contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(!contains_ascii(&output, b"Canon"));
    assert!(!contains_ascii(&output, b"Adobe"));
    assert!(!removed_families(&report).contains(&TagFamily::Gps));
    assert!(removed_families(&report).contains(&TagFamily::Comment));
    assert!(removed_families(&report).contains(&TagFamily::Camera));

    let tiff = jpeg_app1_tiff(&output).expect("kept GPS must leave an APP1 Exif");
    let names = exif_tag_names(&tiff);
    assert!(
        has_tag(&names, "GPSLatitude"),
        "GPS latitude must stay: {names:?}"
    );
    assert!(
        has_tag(&names, "GPSLongitude"),
        "GPS longitude must stay: {names:?}"
    );
    assert!(!has_tag(&names, "Make"), "Make must be gone: {names:?}");
    assert!(
        !has_tag(&names, "Software"),
        "Software must be gone: {names:?}"
    );
    assert!(
        !has_tag(&names, "ImageDescription"),
        "ImageDescription is Comment, must be gone: {names:?}"
    );
}

#[test]
fn jpeg_keep_camera_preserves_make() {
    let input = fixture("jpeg_gps.jpg");
    let (output, report) =
        strip_image_selective("vacation.jpg", &input, &[TagFamily::Camera]).expect("keep camera");

    assert!(contains_ascii(&output, b"Canon"));
    assert!(!contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(!jpeg_has_com(&output));
    assert!(!removed_families(&report).contains(&TagFamily::Camera));
    assert!(removed_families(&report).contains(&TagFamily::Gps));

    let tiff = jpeg_app1_tiff(&output).expect("kept Camera must leave APP1");
    let names = exif_tag_names(&tiff);
    assert!(has_tag(&names, "Make"), "Make must stay: {names:?}");
    assert!(!has_tag(&names, "GPSLatitude"), "{names:?}");
    assert!(!has_tag(&names, "Software"), "{names:?}");
}

#[test]
fn jpeg_keep_software_preserves_software() {
    let input = fixture("jpeg_gps.jpg");
    let (output, report) = strip_image_selective("vacation.jpg", &input, &[TagFamily::Software])
        .expect("keep software");

    assert!(contains_ascii(&output, b"Adobe"));
    assert!(!contains_ascii(&output, b"Canon"));
    assert!(!contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(!removed_families(&report).contains(&TagFamily::Software));
    assert!(removed_families(&report).contains(&TagFamily::Gps));

    let tiff = jpeg_app1_tiff(&output).expect("kept Software must leave APP1");
    let names = exif_tag_names(&tiff);
    assert!(has_tag(&names, "Software"), "Software must stay: {names:?}");
    assert!(!has_tag(&names, "Make"), "{names:?}");
    assert!(!has_tag(&names, "GPSLatitude"), "{names:?}");
}

#[test]
fn jpeg_keep_all_four_keeps_planted_exif_and_com() {
    let input = fixture("jpeg_gps.jpg");
    let original = assert_decodes(&input);
    let keep = [
        TagFamily::Gps,
        TagFamily::Camera,
        TagFamily::Software,
        TagFamily::Comment,
    ];
    let (output, report) = strip_image_selective("vacation.jpg", &input, &keep).expect("keep all");

    assert!(jpeg_has_com(&output));
    assert!(contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(contains_ascii(&output, b"Canon"));
    assert!(contains_ascii(&output, b"Adobe"));
    let tiff = jpeg_app1_tiff(&output).expect("APP1");
    let names = exif_tag_names(&tiff);
    assert!(has_tag(&names, "GPSLatitude"), "{names:?}");
    assert!(has_tag(&names, "Make"), "{names:?}");
    assert!(has_tag(&names, "Software"), "{names:?}");
    assert!(has_tag(&names, "ImageDescription"), "{names:?}");
    assert!(!removed_families(&report).contains(&TagFamily::Gps));
    assert!(!removed_families(&report).contains(&TagFamily::Camera));
    assert!(!removed_families(&report).contains(&TagFamily::Software));
    assert!(!removed_families(&report).contains(&TagFamily::Comment));
    let cleaned = assert_decodes(&output);
    assert_eq!(original.dimensions(), cleaned.dimensions());
}

#[test]
fn png_keep_comment_keeps_text_filters_exif() {
    let input = fixture("png_text.png");
    let original = assert_decodes(&input);
    let (output, report) =
        strip_image_selective("shot.png", &input, &[TagFamily::Comment]).expect("keep comment");

    assert!(contains_ascii(&output, b"SECRET_TAG_XYZ"));
    let texts = png_text_chunks(&output);
    assert!(
        texts
            .iter()
            .any(|(_, data)| data.windows(7).any(|w| w == b"Comment")),
        "Comment tEXt must stay: {texts:?}"
    );
    assert!(
        texts
            .iter()
            .all(|(_, data)| !data.windows(8).any(|w| w == b"Software")),
        "Software tEXt must go: {texts:?}"
    );
    assert!(!removed_families(&report).contains(&TagFamily::Comment));
    assert!(removed_families(&report).contains(&TagFamily::Gps));

    if let Some(tiff) = png_exif_tiff(&output) {
        let names = exif_tag_names(&tiff);
        assert!(!has_tag(&names, "GPSLatitude"), "{names:?}");
        assert!(!has_tag(&names, "Make"), "{names:?}");
        assert!(has_tag(&names, "ImageDescription"), "{names:?}");
    }

    let cleaned = assert_decodes(&output);
    assert_eq!(original.dimensions(), cleaned.dimensions());
}

#[test]
fn png_keep_software_keeps_software_text() {
    let input = fixture("png_text.png");
    let (output, report) =
        strip_image_selective("shot.png", &input, &[TagFamily::Software]).expect("keep software");

    let texts = png_text_chunks(&output);
    assert!(
        texts
            .iter()
            .any(|(_, data)| data.windows(8).any(|w| w == b"Software")),
        "Software tEXt must stay"
    );
    assert!(
        texts
            .iter()
            .all(|(_, data)| !contains_ascii(data, b"SECRET_TAG_XYZ")),
        "Comment tEXt must go"
    );
    assert!(!removed_families(&report).contains(&TagFamily::Software));
}

#[test]
fn webp_keep_gps_rewrites_exif_drops_xmp() {
    let input = fixture("webp_exif.webp");
    let (output, report) =
        strip_image_selective("chat.webp", &input, &[TagFamily::Gps]).expect("keep gps");

    assert!(!webp_has_xmp(&output), "XMP must always be dropped");
    assert!(!contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(report.removed.iter().any(|t| t.family == TagFamily::Xmp));
    assert!(!removed_families(&report).contains(&TagFamily::Gps));
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.to_ascii_lowercase().contains("xmp")),
        "must warn that XMP was dropped: {:?}",
        report.warnings
    );

    let tiff = webp_exif_tiff(&output).expect("EXIF chunk must remain for kept GPS");
    let names = exif_tag_names(&tiff);
    assert!(has_tag(&names, "GPSLatitude"), "{names:?}");
    assert!(!has_tag(&names, "Make"), "{names:?}");
    assert!(!has_tag(&names, "Software"), "{names:?}");
}

#[test]
fn webp_keep_all_four_still_drops_xmp() {
    let input = fixture("webp_exif.webp");
    let keep = [
        TagFamily::Gps,
        TagFamily::Camera,
        TagFamily::Software,
        TagFamily::Comment,
    ];
    let (output, _) = strip_image_selective("chat.webp", &input, &keep).expect("keep all");
    assert!(!webp_has_xmp(&output));
    assert!(!contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(contains_ascii(&output, b"Canon"));
    assert!(contains_ascii(&output, b"Adobe"));
    let tiff = webp_exif_tiff(&output).expect("EXIF");
    let names = exif_tag_names(&tiff);
    assert!(has_tag(&names, "GPSLatitude"), "{names:?}");
    assert!(has_tag(&names, "Make"), "{names:?}");
}

#[test]
fn selective_does_not_mutate_input() {
    let input = fixture("jpeg_gps.jpg");
    let before = input.clone();
    let _ = strip_image_selective("a.jpg", &input, &[TagFamily::Gps]).unwrap();
    assert_eq!(input, before);
}

#[test]
fn empty_keep_plain_jpeg_still_valid() {
    let input = fixture("jpeg_plain.jpg");
    let original = assert_decodes(&input);
    let (output, report) = strip_image_selective("plain.jpg", &input, &[]).expect("plain");
    assert_eq!(report.kind, FileKind::Jpeg);
    let cleaned = assert_decodes(&output);
    assert_eq!(original.dimensions(), cleaned.dimensions());
}
