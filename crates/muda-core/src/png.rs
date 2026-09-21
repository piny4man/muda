use img_parts::png::{Png, PngChunk};
use img_parts::Bytes;

use crate::exif_rewrite::rewrite_exif_tiff;
use crate::{
    cleaned_output_name, drop_unkept, maybe_makernote_warning, parse_exif_raw, truncate, FileKind,
    RemovedTag, StripError, StripReport, TagFamily,
};

/// Chunks required for a correct, displayable PNG (color, not identity).
const KEEP_TYPES: &[[u8; 4]] = &[
    *b"IHDR", *b"PLTE", *b"IDAT", *b"IEND", *b"tRNS", *b"gAMA", *b"cHRM", *b"sRGB", *b"iCCP",
    *b"pHYs", *b"bKGD", *b"sBIT",
];

pub(crate) fn strip_png(
    name: &str,
    data: &[u8],
    keep: &[TagFamily],
) -> Result<(Vec<u8>, StripReport), StripError> {
    let png = Png::from_bytes(Bytes::copy_from_slice(data))
        .map_err(|e| StripError::InvalidImage(e.to_string()))?;

    let mut removed = Vec::new();
    let mut warnings = Vec::new();
    let mut kept = Vec::new();

    for chunk in png.chunks() {
        let kind = chunk.kind();
        if KEEP_TYPES.contains(&kind) {
            kept.push(chunk.clone());
            continue;
        }
        if &kind == b"eXIf" {
            let tags = parse_exif_raw(chunk.contents());
            maybe_makernote_warning(&tags, keep, &mut warnings);
            removed.extend(drop_unkept(tags.clone(), keep));
            if let Some(tiff) = rewrite_exif_tiff(chunk.contents(), keep) {
                kept.push(PngChunk::new(*b"eXIf", Bytes::from(tiff)));
            } else if tags.is_empty() {
                removed.extend(tags_for_dropped_chunk(kind, chunk.contents()));
            }
            continue;
        }
        let tags = tags_for_dropped_chunk(kind, chunk.contents());
        if tags.iter().any(|tag| keep.contains(&tag.family)) {
            kept.push(chunk.clone());
        } else {
            removed.extend(tags);
        }
    }

    let mut rebuilt = png;
    *rebuilt.chunks_mut() = kept;
    let output = rebuilt.encoder().bytes().to_vec();

    if !output.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Err(StripError::Encode("rewritten PNG missing signature".into()));
    }

    let report = StripReport {
        original_name: name.to_string(),
        kind: FileKind::Png,
        output_name: cleaned_output_name(name, FileKind::Png),
        removed,
        warnings,
        input_bytes: data.len(),
        output_bytes: output.len(),
    };
    Ok((output, report))
}

fn tags_for_dropped_chunk(kind: [u8; 4], contents: &Bytes) -> Vec<RemovedTag> {
    let type_name = String::from_utf8_lossy(&kind).into_owned();
    match &kind {
        b"eXIf" => {
            let mut tags = parse_exif_raw(contents);
            if tags.is_empty() {
                tags.push(RemovedTag {
                    family: TagFamily::Other,
                    label: "eXIf".to_string(),
                });
            }
            tags
        }
        b"tEXt" => vec![text_tag("tEXt", contents)],
        b"zTXt" => vec![compressed_text_tag("zTXt", contents)],
        b"iTXt" => vec![itxt_tag(contents)],
        b"tIME" => vec![RemovedTag {
            family: TagFamily::Other,
            label: "tIME".to_string(),
        }],
        _ => vec![RemovedTag {
            family: TagFamily::Other,
            label: type_name,
        }],
    }
}

fn text_tag(prefix: &str, contents: &[u8]) -> RemovedTag {
    let (keyword, value) = split_keyword(contents);
    family_for_png_text(prefix, &keyword, value)
}

fn compressed_text_tag(prefix: &str, contents: &[u8]) -> RemovedTag {
    let (keyword, _) = split_keyword(contents);
    family_for_png_text(prefix, &keyword, "")
}

fn itxt_tag(contents: &[u8]) -> RemovedTag {
    // iTXt: keyword\0 compression_flag compression_method language\0 translated\0 text
    let (keyword, rest) = split_nul(contents);
    let text = if rest.len() >= 2 && rest[0] == 0 {
        let after_flags = &rest[2..];
        let (_, after_lang) = split_nul(after_flags);
        let (_, value) = split_nul(after_lang);
        std::str::from_utf8(value).unwrap_or("")
    } else {
        ""
    };
    family_for_png_text("iTXt", &keyword, text)
}

fn split_nul(contents: &[u8]) -> (String, &[u8]) {
    match contents.iter().position(|&b| b == 0) {
        Some(i) => (
            String::from_utf8_lossy(&contents[..i]).into_owned(),
            &contents[i + 1..],
        ),
        None => (String::from_utf8_lossy(contents).into_owned(), &[]),
    }
}

fn split_keyword(contents: &[u8]) -> (String, &str) {
    let (keyword, rest) = split_nul(contents);
    (keyword, std::str::from_utf8(rest).unwrap_or(""))
}

fn family_for_png_text(prefix: &str, keyword: &str, value: &str) -> RemovedTag {
    let family = match keyword {
        "Software" => TagFamily::Software,
        "Comment" | "Description" | "Title" | "Author" | "Copyright" | "Disclaimer" | "Warning"
        | "Source" => TagFamily::Comment,
        _ if keyword.eq_ignore_ascii_case("gps") => TagFamily::Gps,
        _ => TagFamily::Other,
    };
    let label = if value.is_empty() {
        format!("{prefix}:{keyword}")
    } else {
        format!("{prefix}:{keyword}={}", truncate(value, 80))
    };
    RemovedTag { family, label }
}
