use img_parts::riff::{RiffChunk, RiffContent};
use img_parts::webp::{
    WebP, CHUNK_ALPH, CHUNK_ANIM, CHUNK_ANMF, CHUNK_EXIF, CHUNK_ICCP, CHUNK_VP8, CHUNK_VP8L,
    CHUNK_VP8X, CHUNK_XMP,
};
use img_parts::Bytes;

use crate::{
    cleaned_output_name, parse_exif_raw, ImageKind, RemovedTag, StripError, StripReport, TagFamily,
};

/// VP8X feature flags for EXIF (bit 3) and XMP (bit 2).
const VP8X_FLAG_EXIF: u8 = 0b0000_1000;
const VP8X_FLAG_XMP: u8 = 0b0000_0100;

pub fn strip_webp(name: &str, data: &[u8]) -> Result<(Vec<u8>, StripReport), StripError> {
    let webp = WebP::from_bytes(Bytes::copy_from_slice(data))
        .map_err(|e| StripError::InvalidImage(e.to_string()))?;

    let mut removed = Vec::new();
    let mut kept = Vec::new();

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
            kept.push(vp8x_without_metadata_flags(chunk));
        } else if id == CHUNK_EXIF {
            removed.extend(tags_for_exif_chunk(chunk));
        } else if id == CHUNK_XMP {
            removed.push(RemovedTag {
                family: TagFamily::Xmp,
                label: "XMP".to_string(),
            });
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
        kind: ImageKind::WebP,
        output_name: cleaned_output_name(name, ImageKind::WebP),
        removed,
        warnings: Vec::new(),
        input_bytes: data.len(),
        output_bytes: output.len(),
    };
    Ok((output, report))
}

fn vp8x_without_metadata_flags(chunk: &RiffChunk) -> RiffChunk {
    let Some(data) = chunk.content().data() else {
        return chunk.clone();
    };
    if data.is_empty() {
        return chunk.clone();
    }
    let mut buf = data.to_vec();
    buf[0] &= !(VP8X_FLAG_EXIF | VP8X_FLAG_XMP);
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
