use image::GenericImageView;
use img_parts::jpeg::{markers, Jpeg};
use img_parts::png::Png;
use img_parts::Bytes;
use muda_core::{sniff_kind, strip_image, ImageKind, StripError, TagFamily};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn assert_decodes(bytes: &[u8]) -> image::DynamicImage {
    image::load_from_memory(bytes).expect("image crate should decode rewritten bytes")
}

fn contains_ascii(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn jpeg_with_exif_gps_is_stripped() {
    let input = fixture("jpeg_gps.jpg");
    assert_eq!(sniff_kind(&input), Some(ImageKind::Jpeg));
    assert!(contains_ascii(&input, b"Exif"), "fixture must contain Exif");
    assert!(
        contains_ascii(&input, b"SECRET_TAG_XYZ"),
        "fixture must contain planted comment"
    );
    assert!(
        contains_ascii(&input, b"GPS"),
        "fixture must contain GPS ascii"
    );

    let original = assert_decodes(&input);
    let (output, report) = strip_image("vacation.jpg", &input).expect("strip jpeg");

    assert_eq!(report.kind, ImageKind::Jpeg);
    assert_eq!(report.output_name, "vacation.cleaned.jpg");
    assert_eq!(report.input_bytes, input.len());
    assert_eq!(report.output_bytes, output.len());
    assert!(
        report.removed.iter().any(|t| t.family == TagFamily::Gps),
        "report must include Gps, got {:?}",
        report.removed
    );
    assert!(
        report
            .removed
            .iter()
            .any(|t| t.family == TagFamily::Comment),
        "planted COM must appear in report: {:?}",
        report.removed
    );

    let jpeg = Jpeg::from_bytes(Bytes::copy_from_slice(&output)).expect("parse cleaned jpeg");
    assert!(
        jpeg.segments_by_marker(markers::APP1).next().is_none(),
        "cleaned JPEG must not contain APP1"
    );
    assert!(
        jpeg.segments_by_marker(markers::COM).next().is_none(),
        "cleaned JPEG must not contain COM"
    );

    let cleaned = assert_decodes(&output);
    assert_eq!(original.dimensions(), cleaned.dimensions());

    assert!(!contains_ascii(&output, b"Exif"));
    assert!(!contains_ascii(&output, b"GPS"));
    assert!(!contains_ascii(&output, b"SECRET_TAG_XYZ"));
}

#[test]
fn jpeg_without_exif_still_valid() {
    let input = fixture("jpeg_plain.jpg");
    assert!(
        !contains_ascii(&input, b"Exif"),
        "plain fixture should not contain Exif"
    );
    let original = assert_decodes(&input);
    let (output, report) = strip_image("plain.jpg", &input).expect("strip plain jpeg");
    assert_eq!(output[0], 0xFF);
    assert_eq!(output[1], 0xD8);
    assert_eq!(report.kind, ImageKind::Jpeg);
    let cleaned = assert_decodes(&output);
    assert_eq!(original.dimensions(), cleaned.dimensions());
    Jpeg::from_bytes(Bytes::copy_from_slice(&output)).expect("cleaned plain jpeg parses");
}

#[test]
fn png_text_and_exif_chunks_removed() {
    let input = fixture("png_text.png");
    assert_eq!(sniff_kind(&input), Some(ImageKind::Png));
    assert!(contains_ascii(&input, b"SECRET_TAG_XYZ"));
    assert!(contains_ascii(&input, b"tEXt") || contains_ascii(&input, b"eXIf"));

    let original = assert_decodes(&input);
    let (output, report) = strip_image("shot.png", &input).expect("strip png");

    assert_eq!(report.kind, ImageKind::Png);
    assert_eq!(report.output_name, "shot.cleaned.png");
    assert!(
        report.removed.iter().any(|t| {
            t.family == TagFamily::Comment || t.label.contains("SECRET") || t.label.contains("tEXt")
        }),
        "text chunk must appear in report: {:?}",
        report.removed
    );

    let png = Png::from_bytes(Bytes::copy_from_slice(&output)).expect("parse cleaned png");
    for chunk in png.chunks() {
        let kind = chunk.kind();
        assert_ne!(&kind, b"tEXt", "tEXt must be gone");
        assert_ne!(&kind, b"zTXt", "zTXt must be gone");
        assert_ne!(&kind, b"iTXt", "iTXt must be gone");
        assert_ne!(&kind, b"eXIf", "eXIf must be gone");
        assert_ne!(&kind, b"tIME", "tIME must be gone");
    }

    let cleaned = assert_decodes(&output);
    assert_eq!(original.dimensions(), cleaned.dimensions());
    assert!(!contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(!contains_ascii(&output, b"Exif"));
    assert!(!contains_ascii(&output, b"GPS"));
}

#[test]
fn sniff_truncated_and_garbage() {
    assert_eq!(sniff_kind(&[]), None);
    assert_eq!(sniff_kind(&[0xFF]), None);
    assert_eq!(sniff_kind(&[0xFF, 0xD7]), None);
    assert_eq!(sniff_kind(b"PK\x03\x04"), None);
    assert_eq!(sniff_kind(&[0x89, 0x50, 0x4E]), None);
    match strip_image("x.bin", &[0, 1, 2, 3]) {
        Err(StripError::Unsupported) => {}
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn input_buffer_is_not_returned_mutated() {
    let input = fixture("jpeg_gps.jpg");
    let before = input.clone();
    let _ = strip_image("a.jpg", &input).unwrap();
    assert_eq!(input, before);
}
