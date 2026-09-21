use img_parts::riff::{RiffChunk, RiffContent};
use img_parts::webp::{
    WebP, CHUNK_ALPH, CHUNK_ANIM, CHUNK_ANMF, CHUNK_EXIF, CHUNK_ICCP, CHUNK_VP8, CHUNK_VP8L,
    CHUNK_VP8X, CHUNK_XMP,
};
use img_parts::Bytes;

use crate::exif_rewrite::{rewrite_exif_tiff, tiff_from_exif_payload};
use crate::{
    cleaned_output_name, drop_unkept, maybe_makernote_warning, parse_exif_raw, xmp_dropped_warning,
    FileKind, RemovedTag, StripError, StripReport, TagFamily,
};

/// VP8X feature flags for EXIF (bit 3) and XMP (bit 2).
const VP8X_FLAG_EXIF: u8 = 0b0000_1000;
const VP8X_FLAG_XMP: u8 = 0b0000_0100;

pub(crate) fn strip_webp(
    name: &str,
    data: &[u8],
    keep: &[TagFamily],
) -> Result<(Vec<u8>, StripReport), StripError> {
    let webp = WebP::from_bytes(Bytes::copy_from_slice(data))
        .map_err(|e| StripError::InvalidImage(e.to_string()))?;

    let mut removed = Vec::new();
    let mut warnings = Vec::new();
    let mut kept = Vec::new();

    let rewritten_exif = rewrite_webp_exif(webp.chunks(), keep, &mut removed, &mut warnings);

    for chunk in webp.chunks() {
        let id = chunk.id();
        if id == CHUNK_VP8
            || id == CHUNK_VP8L
            || id == CHUNK_ALPH
            || id == CHUNK_ANIM
            || id == CHUNK_ANMF
            || id == CHUNK_ICCP
        {
            kept.push(chunk.clone());
        } else if id == CHUNK_VP8X {
            kept.push(vp8x_set_metadata_flags(
                chunk,
                rewritten_exif.is_some(),
                false,
            ));
        } else if id == CHUNK_EXIF {
            if let Some(ref payload) = rewritten_exif {
                kept.push(RiffChunk::new(
                    CHUNK_EXIF,
                    RiffContent::Data(Bytes::copy_from_slice(payload)),
                ));
            }
        } else if id == CHUNK_XMP {
            removed.push(RemovedTag {
                family: TagFamily::Xmp,
                label: "XMP".to_string(),
            });
            if !keep.is_empty() {
                warnings.push(xmp_dropped_warning());
            }
        } else {
            removed.push(RemovedTag {
                family: TagFamily::Other,
                label: String::from_utf8_lossy(&id).into_owned(),
            });
        }
    }

    let mut rebuilt = webp;
    *rebuilt.chunks_mut() = kept;
    let output = rebuilt.encoder().bytes().to_vec();

    if output.len() < 12 || output[..4] != *b"RIFF" || output[8..12] != *b"WEBP" {
        return Err(StripError::Encode(
            "rewritten WebP missing RIFF/WEBP signature".into(),
        ));
    }

    let report = StripReport {
        original_name: name.to_string(),
        kind: FileKind::WebP,
        output_name: cleaned_output_name(name, FileKind::WebP),
        removed,
        warnings,
        input_bytes: data.len(),
        output_bytes: output.len(),
    };
    Ok((output, report))
}

fn rewrite_webp_exif(
    chunks: &[RiffChunk],
    keep: &[TagFamily],
    removed: &mut Vec<RemovedTag>,
    warnings: &mut Vec<String>,
) -> Option<Vec<u8>> {
    let chunk = chunks.iter().find(|chunk| chunk.id() == CHUNK_EXIF)?;
    let tags = tags_for_exif_chunk(chunk);
    maybe_makernote_warning(&tags, keep, warnings);
    let payload = chunk.content().data().map(|d| d.as_ref())?;
    let tiff = tiff_from_exif_payload(payload);
    match rewrite_exif_tiff(tiff, keep) {
        Some(rewritten) => {
            removed.extend(drop_unkept(tags, keep));
            if payload.starts_with(b"Exif\0\0") {
                let mut wrapped = b"Exif\0\0".to_vec();
                wrapped.extend_from_slice(&rewritten);
                Some(wrapped)
            } else {
                Some(rewritten)
            }
        }
        None => {
            if tags.is_empty() {
                removed.push(RemovedTag {
                    family: TagFamily::Other,
                    label: "EXIF".to_string(),
                });
            } else {
                removed.extend(drop_unkept(tags, keep));
            }
            None
        }
    }
}

fn vp8x_set_metadata_flags(chunk: &RiffChunk, exif: bool, xmp: bool) -> RiffChunk {
    let Some(data) = chunk.content().data() else {
        return chunk.clone();
    };
    if data.is_empty() {
        return chunk.clone();
    }
    let mut buf = data.to_vec();
    buf[0] &= !(VP8X_FLAG_EXIF | VP8X_FLAG_XMP);
    if exif {
        buf[0] |= VP8X_FLAG_EXIF;
    }
    if xmp {
        buf[0] |= VP8X_FLAG_XMP;
    }
    RiffChunk::new(CHUNK_VP8X, RiffContent::Data(Bytes::from(buf)))
}

fn tags_for_exif_chunk(chunk: &RiffChunk) -> Vec<RemovedTag> {
    let Some(data) = chunk.content().data() else {
        return vec![RemovedTag {
            family: TagFamily::Other,
            label: "EXIF".to_string(),
        }];
    };
    let tiff = if data.starts_with(b"Exif\0\0") {
        &data[6..]
    } else {
        data.as_ref()
    };
    let mut tags = parse_exif_raw(tiff);
    if tags.is_empty() {
        tags.push(RemovedTag {
            family: TagFamily::Other,
            label: "EXIF".to_string(),
        });
    }
    tags
}
