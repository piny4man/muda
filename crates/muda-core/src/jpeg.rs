use img_parts::jpeg::{markers, Jpeg, JpegSegment};
use img_parts::Bytes;

use crate::exif_rewrite::{jpeg_exif_payload, rewrite_exif_tiff, tiff_from_exif_payload};
use crate::{
    cleaned_output_name, drop_unkept, latin1_lossy, maybe_makernote_warning, parse_exif_fields,
    truncate, xmp_dropped_warning, FileKind, RemovedTag, StripError, StripReport, TagFamily,
};

const JFIF_IDENT: &[u8] = b"JFIF\0";
const JFXX_IDENT: &[u8] = b"JFXX\0";
const EXIF_IDENT: &[u8] = b"Exif\0\0";
const XMP_IDENT: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const XMP_EXT_IDENT: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";
const ICC_IDENT: &[u8] = b"ICC_PROFILE\0";
const PHOTOSHOP_IDENT: &[u8] = b"Photoshop 3.0";
const ADOBE_IDENT: &[u8] = b"Adobe";

/// Minimal JFIF APP0 (version 1.01, no thumbnail) for decoder compatibility.
const MINIMAL_JFIF: &[u8] = &[
    b'J', b'F', b'I', b'F', 0x00, // identifier
    0x01, 0x01, // version 1.01
    0x00, // density units (none)
    0x00, 0x01, 0x00, 0x01, // 1x1 density
    0x00, 0x00, // no thumbnail
];

pub(crate) fn strip_jpeg(
    name: &str,
    data: &[u8],
    keep: &[TagFamily],
) -> Result<(Vec<u8>, StripReport), StripError> {
    let jpeg = Jpeg::from_bytes(Bytes::copy_from_slice(data))
        .map_err(|e| StripError::InvalidImage(e.to_string()))?;

    let mut removed = Vec::new();
    let mut warnings = Vec::new();
    let mut kept: Vec<JpegSegment> = Vec::new();
    let mut has_jfif = false;

    let parsed_exif = parse_exif_fields(data);
    maybe_makernote_warning(&parsed_exif, keep, &mut warnings);
    removed.extend(drop_unkept(parsed_exif, keep));

    for segment in jpeg.segments() {
        match classify_segment(segment, keep) {
            SegmentAction::Keep => {
                if segment.marker() == markers::APP0 && segment.contents().starts_with(JFIF_IDENT) {
                    has_jfif = true;
                }
                kept.push(segment.clone());
            }
            SegmentAction::Replace(rewritten) => {
                kept.push(rewritten);
            }
            SegmentAction::Drop(tags) => {
                if tags.iter().any(|tag| tag.family == TagFamily::Xmp) && !keep.is_empty() {
                    warnings.push(xmp_dropped_warning());
                }
                for tag in tags {
                    let duplicate = removed.iter().any(|existing| {
                        existing.family == tag.family && existing.label == tag.label
                    });
                    if !duplicate {
                        removed.push(tag);
                    }
                }
            }
        }
    }

    if !has_jfif {
        kept.insert(
            0,
            JpegSegment::new_with_contents(markers::APP0, Bytes::copy_from_slice(MINIMAL_JFIF)),
        );
        warnings.push("wrote a fresh minimal JFIF APP0 for compatibility".to_string());
    }

    let mut rebuilt = jpeg;
    *rebuilt.segments_mut() = kept;
    let output = rebuilt.encoder().bytes().to_vec();

    if output.len() < 4 || output[0] != 0xFF || output[1] != 0xD8 {
        return Err(StripError::Encode(
            "rewritten JPEG missing SOI marker".into(),
        ));
    }

    let report = StripReport {
        original_name: name.to_string(),
        kind: FileKind::Jpeg,
        output_name: cleaned_output_name(name, FileKind::Jpeg),
        removed,
        warnings,
        input_bytes: data.len(),
        output_bytes: output.len(),
    };
    Ok((output, report))
}

enum SegmentAction {
    Keep,
    Replace(JpegSegment),
    Drop(Vec<RemovedTag>),
}

fn classify_segment(segment: &JpegSegment, keep: &[TagFamily]) -> SegmentAction {
    let marker = segment.marker();
    let contents = segment.contents();

    if marker == markers::COM {
        if keep.contains(&TagFamily::Comment) {
            return SegmentAction::Keep;
        }
        let text = latin1_lossy(contents).trim_end_matches('\0').to_string();
        let label = if text.is_empty() {
            "COM".to_string()
        } else {
            format!("COM={}", truncate(&text, 80))
        };
        return SegmentAction::Drop(vec![RemovedTag {
            family: TagFamily::Comment,
            label,
        }]);
    }

    if marker == markers::APP0 {
        if contents.starts_with(JFXX_IDENT) {
            return SegmentAction::Drop(vec![RemovedTag {
                family: TagFamily::Thumbnail,
                label: "JFXX thumbnail".to_string(),
            }]);
        }
        if contents.starts_with(JFIF_IDENT) {
            return SegmentAction::Keep;
        }
        return SegmentAction::Drop(vec![unknown_app(marker)]);
    }

    if marker == markers::APP1 {
        if contents.starts_with(EXIF_IDENT) {
            let tiff = tiff_from_exif_payload(contents);
            if let Some(rewritten) = rewrite_exif_tiff(tiff, keep) {
                let payload = jpeg_exif_payload(&rewritten);
                return SegmentAction::Replace(JpegSegment::new_with_contents(
                    markers::APP1,
                    Bytes::from(payload),
                ));
            }
            // Detailed tags are collected from the full file via kamadak-exif.
            // Ensure the APP1 family is represented even if parsing yields nothing.
            return SegmentAction::Drop(vec![RemovedTag {
                family: TagFamily::Other,
                label: "APP1".to_string(),
            }]);
        }
        if contents.starts_with(XMP_IDENT) || contents.starts_with(XMP_EXT_IDENT) {
            return SegmentAction::Drop(vec![RemovedTag {
                family: TagFamily::Xmp,
                label: "XMP".to_string(),
            }]);
        }
        return SegmentAction::Drop(vec![RemovedTag {
            family: TagFamily::Other,
            label: "APP1".to_string(),
        }]);
    }

    if marker == markers::APP2 {
        if contents.starts_with(ICC_IDENT) {
            return SegmentAction::Keep;
        }
        return SegmentAction::Drop(vec![unknown_app(marker)]);
    }

    if marker == markers::APP13 {
        let label = if contents.starts_with(PHOTOSHOP_IDENT) {
            "Photoshop IRB".to_string()
        } else {
            "APP13".to_string()
        };
        return SegmentAction::Drop(vec![RemovedTag {
            family: TagFamily::Other,
            label,
        }]);
    }

    if marker == markers::APP14 {
        if contents.starts_with(ADOBE_IDENT) {
            // Color transform, not identity metadata.
            return SegmentAction::Keep;
        }
        return SegmentAction::Drop(vec![unknown_app(marker)]);
    }

    if (markers::APP0..=markers::APP15).contains(&marker) {
        return SegmentAction::Drop(vec![unknown_app(marker)]);
    }

    SegmentAction::Keep
}

fn unknown_app(marker: u8) -> RemovedTag {
    RemovedTag {
        family: TagFamily::Other,
        label: format!("APP{}", marker.saturating_sub(markers::APP0)),
    }
}
