use image::GenericImageView;
use img_parts::jpeg::{markers, Jpeg};
use img_parts::png::Png;
use img_parts::webp::{
    WebP, CHUNK_ANIM, CHUNK_ANMF, CHUNK_EXIF, CHUNK_ICCP, CHUNK_VP8L, CHUNK_XMP,
};
use img_parts::Bytes;
use muda_core::{sniff_kind, strip_image, FileKind, StripError, TagFamily};

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
    assert_eq!(sniff_kind(&input), Some(FileKind::Jpeg));
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

    assert_eq!(report.kind, FileKind::Jpeg);
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
    assert_eq!(report.kind, FileKind::Jpeg);
    let cleaned = assert_decodes(&output);
    assert_eq!(original.dimensions(), cleaned.dimensions());
    Jpeg::from_bytes(Bytes::copy_from_slice(&output)).expect("cleaned plain jpeg parses");
}

#[test]
fn png_text_and_exif_chunks_removed() {
    let input = fixture("png_text.png");
    assert_eq!(sniff_kind(&input), Some(FileKind::Png));
    assert!(contains_ascii(&input, b"SECRET_TAG_XYZ"));
    assert!(contains_ascii(&input, b"tEXt") || contains_ascii(&input, b"eXIf"));

    let original = assert_decodes(&input);
    let (output, report) = strip_image("shot.png", &input).expect("strip png");

    assert_eq!(report.kind, FileKind::Png);
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
    assert_eq!(sniff_kind(b"RIFF\x24\x00\x00\x00WAVE"), None);
    assert_eq!(
        sniff_kind(b"\0\0\0\x18ftypheic"),
        None,
        "HEIC must stay unsupported"
    );
    match strip_image("x.bin", &[0, 1, 2, 3]) {
        Err(StripError::Unsupported) => {}
        other => panic!("expected Unsupported, got {other:?}"),
    }
    let truncated = b"RIFF\x00\x00\x00\x00WEBP";
    assert_eq!(sniff_kind(truncated), Some(FileKind::WebP));
    match strip_image("broken.webp", truncated) {
        Err(StripError::InvalidImage(_)) => {}
        other => panic!("truncated WebP must be invalid, got {other:?}"),
    }
}

#[test]
fn input_buffer_is_not_returned_mutated() {
    let input = fixture("jpeg_gps.jpg");
    let before = input.clone();
    let _ = strip_image("a.jpg", &input).unwrap();
    assert_eq!(input, before);
}

fn parse_webp(bytes: &[u8]) -> WebP {
    WebP::from_bytes(Bytes::copy_from_slice(bytes)).expect("parse webp")
}

fn chunk_bytes(webp: &WebP, id: [u8; 4]) -> Option<Bytes> {
    webp.chunk_by_id(id)?.content().data().cloned()
}

#[test]
fn webp_exif_xmp_is_stripped() {
    let input = fixture("webp_exif.webp");
    assert_eq!(sniff_kind(&input), Some(FileKind::WebP));
    assert!(contains_ascii(&input, b"EXIF"));
    assert!(contains_ascii(&input, b"GPS"));
    assert!(contains_ascii(&input, b"SECRET_TAG_XYZ"));

    let original = parse_webp(&input);
    let vp8l = chunk_bytes(&original, CHUNK_VP8L).expect("fixture VP8L");
    let (output, report) = strip_image("chat.webp", &input).expect("strip webp");

    assert_eq!(report.kind, FileKind::WebP);
    assert_eq!(report.output_name, "chat.cleaned.webp");
    assert_eq!(report.input_bytes, input.len());
    assert_eq!(report.output_bytes, output.len());
    assert!(
        report.removed.iter().any(|t| t.family == TagFamily::Gps),
        "report must include Gps, got {:?}",
        report.removed
    );
    assert!(
        report.removed.iter().any(|t| t.family == TagFamily::Xmp),
        "report must include XMP, got {:?}",
        report.removed
    );
    assert!(
        report.removed.iter().all(|t| t.family != TagFamily::Icc),
        "kept ICC must not be listed as removed: {:?}",
        report.removed
    );

    let cleaned = parse_webp(&output);
    assert!(cleaned.chunk_by_id(CHUNK_EXIF).is_none());
    assert!(cleaned.chunk_by_id(CHUNK_XMP).is_none());
    assert_eq!(chunk_bytes(&cleaned, CHUNK_VP8L).as_ref(), Some(&vp8l));
    assert!(!contains_ascii(&output, b"SECRET_TAG_XYZ"));
    assert!(!contains_ascii(&output, b"GPS"));
}

#[test]
fn webp_without_metadata_still_valid() {
    let input = fixture("webp_plain.webp");
    assert_eq!(sniff_kind(&input), Some(FileKind::WebP));
    assert!(!contains_ascii(&input, b"EXIF"));
    let original = parse_webp(&input);
    let vp8l = chunk_bytes(&original, CHUNK_VP8L).expect("plain VP8L");
    let (output, report) = strip_image("plain.webp", &input).expect("strip plain webp");
    assert_eq!(report.kind, FileKind::WebP);
    assert_eq!(report.output_name, "plain.cleaned.webp");
    let cleaned = parse_webp(&output);
    assert_eq!(chunk_bytes(&cleaned, CHUNK_VP8L).as_ref(), Some(&vp8l));
}

#[test]
fn webp_icc_is_kept() {
    let input = fixture("webp_icc.webp");
    assert!(contains_ascii(&input, b"ICC_PROFILE_MUDA_KEEP"));
    let original = parse_webp(&input);
    let icc = chunk_bytes(&original, CHUNK_ICCP).expect("fixture ICCP");
    let (output, report) = strip_image("profile.webp", &input).expect("strip icc webp");
    assert!(
        report.removed.iter().all(|t| t.family != TagFamily::Icc),
        "ICC must stay: {:?}",
        report.removed
    );
    let cleaned = parse_webp(&output);
    assert_eq!(chunk_bytes(&cleaned, CHUNK_ICCP).as_ref(), Some(&icc));
    assert!(contains_ascii(&output, b"ICC_PROFILE_MUDA_KEEP"));
}

#[test]
fn webp_animation_payload_is_kept() {
    let input = fixture("webp_anim.webp");
    assert!(contains_ascii(&input, b"ANIM"));
    assert!(contains_ascii(&input, b"EXIF"));
    let original = parse_webp(&input);
    let anim = chunk_bytes(&original, CHUNK_ANIM).expect("ANIM");
    let frames: Vec<Bytes> = original
        .chunks_by_id(CHUNK_ANMF)
        .filter_map(|chunk| chunk.content().data().cloned())
        .collect();
    assert_eq!(frames.len(), 2);
    let (output, report) = strip_image("loop.webp", &input).expect("strip anim webp");
    assert!(
        report.removed.iter().any(|t| t.family == TagFamily::Gps)
            || report.removed.iter().any(|t| t.label.contains("EXIF")),
        "anim EXIF must be reported: {:?}",
        report.removed
    );
    let cleaned = parse_webp(&output);
    assert!(cleaned.chunk_by_id(CHUNK_EXIF).is_none());
    assert_eq!(chunk_bytes(&cleaned, CHUNK_ANIM).as_ref(), Some(&anim));
    let cleaned_frames: Vec<Bytes> = cleaned
        .chunks_by_id(CHUNK_ANMF)
        .filter_map(|chunk| chunk.content().data().cloned())
        .collect();
    assert_eq!(cleaned_frames, frames);
}
