//! Rebuild a PDF as a single generation with identity metadata removed.
//!
//! Page content streams are not rendered. Save uses a traditional xref so the
//! writer does not recompress dictionaries into object streams.

use lopdf::{Dictionary, Document, LoadOptions, Object, ObjectId, SaveOptions};

use crate::jpeg;
use crate::{
    cleaned_output_name, truncate, xmp_dropped_warning, FileKind, RemovedTag, StripError,
    StripReport, TagFamily,
};

const ENCRYPTED: &str = "encrypted PDF; decryption is not supported";
const MAX_DECOMPRESSED: usize = 64 * 1024 * 1024;

const WARN_SIGNATURE: &str = "digital signature removed; it will not verify";
const WARN_EMBEDDED: &str = "embedded files removed";
const WARN_JAVASCRIPT: &str = "JavaScript removed";
const WARN_FORM: &str = "form values kept; they can contain names and addresses";
const WARN_JPEG: &str = "embedded JPEG left unchanged; strip failed";

struct InfoSpec {
    key: &'static [u8],
    label: &'static str,
    family: TagFamily,
    always: bool,
}

const INFO_SPECS: &[InfoSpec] = &[
    InfoSpec {
        key: b"Author",
        label: "Author",
        family: TagFamily::Comment,
        always: false,
    },
    InfoSpec {
        key: b"Subject",
        label: "Subject",
        family: TagFamily::Comment,
        always: false,
    },
    InfoSpec {
        key: b"Keywords",
        label: "Keywords",
        family: TagFamily::Comment,
        always: false,
    },
    InfoSpec {
        key: b"Creator",
        label: "Creator",
        family: TagFamily::Software,
        always: false,
    },
    InfoSpec {
        key: b"Producer",
        label: "Producer",
        family: TagFamily::Software,
        always: false,
    },
    InfoSpec {
        key: b"CreationDate",
        label: "CreationDate",
        family: TagFamily::Other,
        always: true,
    },
    InfoSpec {
        key: b"ModDate",
        label: "ModDate",
        family: TagFamily::Other,
        always: true,
    },
];

pub(crate) fn strip_pdf(
    name: &str,
    data: &[u8],
    keep: &[TagFamily],
) -> Result<(Vec<u8>, StripReport), StripError> {
    let mut doc = Document::load_mem_with_options(data, load_options()).map_err(map_load_error)?;
    if doc.is_encrypted() || doc.was_encrypted() || doc.trailer.has(b"Encrypt") {
        return Err(StripError::InvalidImage(ENCRYPTED.into()));
    }

    let mut removed = Vec::new();
    let mut warnings = Vec::new();

    strip_info(&mut doc, keep, &mut removed);
    strip_attached_keys(&mut doc, keep, &mut removed, &mut warnings);
    strip_catalog(&mut doc, &mut warnings)?;
    strip_page_actions(&mut doc, &mut warnings);
    strip_annots(&mut doc, keep, &mut removed, &mut warnings);
    strip_signatures(&mut doc, &mut warnings);
    strip_dct_images(&mut doc, keep, &mut removed, &mut warnings);
    doc.prune_objects();
    replace_id(&mut doc, &mut removed)?;

    let output = save_pdf(&mut doc)?;
    if !output.starts_with(b"%PDF-") {
        return Err(StripError::Encode("rewritten PDF missing header".into()));
    }
    Document::load_mem_with_options(&output, load_options())
        .map_err(|err| StripError::Encode(format!("rewritten PDF did not reload: {err}")))?;

    let report = StripReport {
        original_name: name.to_string(),
        kind: FileKind::Pdf,
        output_name: cleaned_output_name(name, FileKind::Pdf),
        removed,
        warnings,
        input_bytes: data.len(),
        output_bytes: output.len(),
    };
    Ok((output, report))
}

fn load_options() -> LoadOptions {
    LoadOptions::with_max_decompressed_size(MAX_DECOMPRESSED)
}

fn map_load_error(err: lopdf::Error) -> StripError {
    let msg = err.to_string();
    if msg.to_ascii_lowercase().contains("encrypt") {
        StripError::InvalidImage(ENCRYPTED.into())
    } else {
        StripError::InvalidImage(msg)
    }
}

fn save_pdf(doc: &mut Document) -> Result<Vec<u8>, StripError> {
    let options = SaveOptions::builder()
        .use_object_streams(false)
        .use_xref_streams(false)
        .linearize(false)
        .build();
    let mut output = Vec::new();
    doc.save_with_options(&mut output, options)
        .map_err(|err| StripError::Encode(err.to_string()))?;
    Ok(output)
}

fn replace_id(doc: &mut Document, removed: &mut Vec<RemovedTag>) -> Result<(), StripError> {
    if doc.trailer.has(b"ID") {
        removed.push(RemovedTag {
            family: TagFamily::Other,
            label: "ID".to_string(),
        });
    }
    let first = random_bytes()?;
    let second = random_bytes()?;
    doc.trailer.set(
        "ID",
        Object::Array(vec![hex_string(&first), hex_string(&second)]),
    );
    Ok(())
}

fn random_bytes() -> Result<[u8; 16], StripError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|err| StripError::Encode(format!("id: {err}")))?;
    Ok(bytes)
}

fn hex_string(bytes: &[u8]) -> Object {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(HEX[(*byte >> 4) as usize]));
        text.push(char::from(HEX[(*byte & 0x0f) as usize]));
    }
    Object::string_literal(text)
}

fn strip_info(doc: &mut Document, keep: &[TagFamily], removed: &mut Vec<RemovedTag>) {
    let info_id = match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) => Some(*id),
        Ok(Object::Dictionary(_)) => None,
        _ => return,
    };
    let mut dict = if let Some(id) = info_id {
        match doc.get_dictionary(id) {
            Ok(dict) => dict.clone(),
            Err(_) => return,
        }
    } else if let Ok(Object::Dictionary(dict)) = doc.trailer.get(b"Info") {
        dict.clone()
    } else {
        return;
    };

    for spec in INFO_SPECS {
        if spec.always || !keep.contains(&spec.family) {
            if let Some(value) = dict.remove(spec.key) {
                removed.push(RemovedTag {
                    family: spec.family,
                    label: labeled(spec.label, &pdf_text(&value)),
                });
            }
        }
    }

    let custom: Vec<Vec<u8>> = dict
        .iter()
        .map(|(key, _)| key.clone())
        .filter(|key| !known_info_key(key))
        .collect();
    for key in custom {
        if let Some(value) = dict.remove(&key) {
            let name = String::from_utf8_lossy(&key);
            removed.push(RemovedTag {
                family: TagFamily::Other,
                label: labeled(&format!("Info:{name}"), &pdf_text(&value)),
            });
        }
    }

    if dict.is_empty() {
        doc.trailer.remove(b"Info");
        if let Some(id) = info_id {
            doc.delete_object(id);
        }
        return;
    }

    if let Some(id) = info_id {
        doc.set_object(id, dict);
    } else {
        doc.trailer.set("Info", Object::Dictionary(dict));
    }
}

fn known_info_key(key: &[u8]) -> bool {
    matches!(
        key,
        b"Title"
            | b"Trapped"
            | b"Author"
            | b"Subject"
            | b"Keywords"
            | b"Creator"
            | b"Producer"
            | b"CreationDate"
            | b"ModDate"
    )
}

fn strip_attached_keys(
    doc: &mut Document,
    keep: &[TagFamily],
    removed: &mut Vec<RemovedTag>,
    warnings: &mut Vec<String>,
) {
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    let mut delete_ids = Vec::new();
    let mut reported_xmp = false;

    for id in ids {
        let dropped = detach_keys(doc, id);
        for (key, value) in dropped {
            match key.as_slice() {
                b"Metadata" => {
                    if !reported_xmp {
                        removed.push(RemovedTag {
                            family: TagFamily::Xmp,
                            label: "XMP".to_string(),
                        });
                        reported_xmp = true;
                        if !keep.is_empty() {
                            push_warning(warnings, &xmp_dropped_warning());
                        }
                    }
                    if let Object::Reference(ref_id) = value {
                        delete_ids.push(ref_id);
                    }
                }
                b"Thumb" => removed.push(RemovedTag {
                    family: TagFamily::Thumbnail,
                    label: "Thumb".to_string(),
                }),
                b"PieceInfo" => {
                    removed.push(RemovedTag {
                        family: TagFamily::Other,
                        label: "PieceInfo".to_string(),
                    });
                    if let Object::Reference(ref_id) = value {
                        delete_ids.push(ref_id);
                    }
                }
                b"LastModified" => removed.push(RemovedTag {
                    family: TagFamily::Other,
                    label: labeled("LastModified", &pdf_text(&value)),
                }),
                _ => {}
            }
        }
    }

    for id in delete_ids {
        doc.delete_object(id);
    }
}

fn detach_keys(doc: &mut Document, id: ObjectId) -> Vec<(Vec<u8>, Object)> {
    const KEYS: &[&[u8]] = &[b"Metadata", b"PieceInfo", b"LastModified", b"Thumb"];
    let Ok(object) = doc.get_object_mut(id) else {
        return Vec::new();
    };
    let dict = match object {
        Object::Dictionary(dict) => dict,
        Object::Stream(stream) => &mut stream.dict,
        _ => return Vec::new(),
    };
    let mut dropped = Vec::new();
    for key in KEYS {
        if let Some(value) = dict.remove(key) {
            dropped.push((key.to_vec(), value));
        }
    }
    dropped
}

fn strip_catalog(doc: &mut Document, warnings: &mut Vec<String>) -> Result<(), StripError> {
    let catalog_id = match doc.trailer.get(b"Root") {
        Ok(Object::Reference(id)) => *id,
        _ => return Err(StripError::InvalidImage("PDF catalog missing".into())),
    };
    if doc
        .get_dictionary(catalog_id)
        .ok()
        .is_some_and(|dict| dict.has(b"AcroForm"))
    {
        push_warning(warnings, WARN_FORM);
    }
    strip_associated_files(doc, catalog_id, warnings);
    strip_name_trees(doc, catalog_id, warnings);
    strip_action_key(doc, catalog_id, b"OpenAction", warnings);
    strip_aa(doc, catalog_id, warnings);
    strip_perms(doc, catalog_id, warnings);
    Ok(())
}

fn strip_page_actions(doc: &mut Document, warnings: &mut Vec<String>) {
    let pages: Vec<ObjectId> = doc.page_iter().collect();
    for page_id in pages {
        strip_aa(doc, page_id, warnings);
    }
}

fn strip_associated_files(doc: &mut Document, catalog_id: ObjectId, warnings: &mut Vec<String>) {
    let Some(af) = dict_value(doc, catalog_id, b"AF") else {
        return;
    };
    if let Ok(dict) = doc.get_dictionary_mut(catalog_id) {
        dict.remove(b"AF");
    }
    delete_graph(doc, &af);
    push_warning(warnings, WARN_EMBEDDED);
}

fn strip_name_trees(doc: &mut Document, catalog_id: ObjectId, warnings: &mut Vec<String>) {
    let Some(names_obj) = dict_value(doc, catalog_id, b"Names") else {
        return;
    };
    let Some(names_id) = ref_id(&names_obj) else {
        return;
    };
    let Ok(mut names) = doc.get_dictionary(names_id).cloned() else {
        return;
    };
    let javascript = names.remove(b"JavaScript");
    let embedded = names.remove(b"EmbeddedFiles");
    if names.is_empty() {
        if let Ok(catalog) = doc.get_dictionary_mut(catalog_id) {
            catalog.remove(b"Names");
        }
        doc.delete_object(names_id);
    } else if let Ok(dict) = doc.get_dictionary_mut(names_id) {
        *dict = names;
    }
    if let Some(javascript) = javascript {
        delete_graph(doc, &javascript);
        push_warning(warnings, WARN_JAVASCRIPT);
    }
    if let Some(embedded) = embedded {
        delete_graph(doc, &embedded);
        push_warning(warnings, WARN_EMBEDDED);
    }
}

fn strip_action_key(
    doc: &mut Document,
    owner_id: ObjectId,
    key: &[u8],
    warnings: &mut Vec<String>,
) {
    let Some(action) = dict_value(doc, owner_id, key) else {
        return;
    };
    if !action_is_js(doc, &action) {
        return;
    }
    if let Ok(dict) = doc.get_dictionary_mut(owner_id) {
        dict.remove(key);
    }
    delete_graph(doc, &action);
    push_warning(warnings, WARN_JAVASCRIPT);
}

fn strip_aa(doc: &mut Document, owner_id: ObjectId, warnings: &mut Vec<String>) {
    let Some(aa) = dict_value(doc, owner_id, b"AA") else {
        return;
    };
    if !aa_has_js(doc, &aa) {
        return;
    }
    if let Ok(dict) = doc.get_dictionary_mut(owner_id) {
        dict.remove(b"AA");
    }
    delete_graph(doc, &aa);
    push_warning(warnings, WARN_JAVASCRIPT);
}

fn strip_perms(doc: &mut Document, catalog_id: ObjectId, warnings: &mut Vec<String>) {
    let Some(perms) = dict_value(doc, catalog_id, b"Perms") else {
        return;
    };
    if let Ok(dict) = doc.get_dictionary_mut(catalog_id) {
        dict.remove(b"Perms");
    }
    delete_graph(doc, &perms);
    push_warning(warnings, WARN_SIGNATURE);
}

fn strip_annots(
    doc: &mut Document,
    keep: &[TagFamily],
    removed: &mut Vec<RemovedTag>,
    warnings: &mut Vec<String>,
) {
    let pages: Vec<ObjectId> = doc.page_iter().collect();
    let mut annots = Vec::new();
    for page_id in pages {
        annots.extend(page_annot_ids(doc, page_id));
    }
    for annot_id in annots {
        strip_annot(doc, annot_id, keep, removed, warnings);
    }
}

fn page_annot_ids(doc: &Document, page_id: ObjectId) -> Vec<ObjectId> {
    let Ok(page) = doc.get_dictionary(page_id) else {
        return Vec::new();
    };
    let Ok(annots) = page.get(b"Annots") else {
        return Vec::new();
    };
    let items = match annots {
        Object::Array(items) => items.clone(),
        Object::Reference(id) => match doc.get_object(*id) {
            Ok(Object::Array(items)) => items.clone(),
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    items
        .iter()
        .filter_map(|item| item.as_reference().ok())
        .collect()
}

fn strip_annot(
    doc: &mut Document,
    annot_id: ObjectId,
    keep: &[TagFamily],
    removed: &mut Vec<RemovedTag>,
    warnings: &mut Vec<String>,
) {
    let Ok(original) = doc.get_dictionary(annot_id).cloned() else {
        return;
    };
    let subtype = original.get(b"Subtype").and_then(Object::as_name).ok();
    if subtype == Some(b"FileAttachment") || original.has(b"FS") {
        let _ = doc.remove_annot(&annot_id);
        push_warning(warnings, WARN_EMBEDDED);
        return;
    }
    if original
        .get(b"V")
        .ok()
        .is_some_and(|value| object_is_sig(doc, value))
    {
        let _ = doc.remove_annot(&annot_id);
        push_warning(warnings, WARN_SIGNATURE);
        return;
    }

    let mut dict = original;
    if !keep.contains(&TagFamily::Comment) {
        if let Some(author) = dict.remove(b"T") {
            removed.push(RemovedTag {
                family: TagFamily::Comment,
                label: labeled("AnnotAuthor", &pdf_text(&author)),
            });
        }
    }
    if let Some(modified) = dict.remove(b"M") {
        removed.push(RemovedTag {
            family: TagFamily::Other,
            label: labeled("AnnotDate", &pdf_text(&modified)),
        });
    }

    let mut delete_ids = Vec::new();
    let mut saw_script = false;
    if let Ok(action) = dict.get(b"A").cloned() {
        if action_is_js(doc, &action) {
            dict.remove(b"A");
            if let Some(id) = ref_id(&action) {
                delete_ids.push(id);
            }
            saw_script = true;
        }
    }
    if let Ok(action) = dict.get(b"AA").cloned() {
        if aa_has_js(doc, &action) {
            dict.remove(b"AA");
            if let Some(id) = ref_id(&action) {
                delete_ids.push(id);
            }
            saw_script = true;
        }
    }

    doc.set_object(annot_id, dict);
    for id in delete_ids {
        doc.delete_object(id);
    }
    if saw_script {
        push_warning(warnings, WARN_JAVASCRIPT);
    }
}

fn strip_signatures(doc: &mut Document, warnings: &mut Vec<String>) {
    let sig_ids: Vec<ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(id, object)| {
            object_dict(object)
                .filter(|dict| dict_is_sig(dict))
                .map(|_| *id)
        })
        .collect();
    if sig_ids.is_empty() {
        return;
    }
    for id in sig_ids {
        doc.delete_object(id);
    }
    push_warning(warnings, WARN_SIGNATURE);
}

fn strip_dct_images(
    doc: &mut Document,
    keep: &[TagFamily],
    removed: &mut Vec<RemovedTag>,
    warnings: &mut Vec<String>,
) {
    let image_ids: Vec<ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let stream = object.as_stream().ok()?;
            let subtype = stream.dict.get(b"Subtype").and_then(Object::as_name).ok();
            if subtype != Some(b"Image") {
                return None;
            }
            let filter = stream.dict.get(b"Filter").ok();
            let dct = matches!(filter, Some(Object::Name(name)) if name.as_slice() == b"DCTDecode");
            dct.then_some(*id)
        })
        .collect();

    for id in image_ids {
        let content = match doc.get_object(id).and_then(Object::as_stream) {
            Ok(stream) => stream.content.clone(),
            Err(_) => continue,
        };
        match jpeg::strip_jpeg("embedded.jpg", &content, keep) {
            Ok((output, report)) => {
                if output != content {
                    if let Ok(stream) = doc.get_object_mut(id).and_then(Object::as_stream_mut) {
                        stream.allows_compression = false;
                        stream.set_content(output);
                    }
                }
                for tag in report.removed {
                    removed.push(RemovedTag {
                        family: tag.family,
                        label: format!("image:{}", tag.label),
                    });
                }
                for warning in report.warnings {
                    if !warning.contains("minimal JFIF") {
                        push_warning(warnings, &warning);
                    }
                }
            }
            Err(_) => push_warning(warnings, WARN_JPEG),
        }
    }
}

fn dict_value(doc: &Document, id: ObjectId, key: &[u8]) -> Option<Object> {
    doc.get_dictionary(id).ok()?.get(key).ok().cloned()
}

fn ref_id(object: &Object) -> Option<ObjectId> {
    match object {
        Object::Reference(id) => Some(*id),
        _ => None,
    }
}

fn delete_graph(doc: &mut Document, object: &Object) {
    let mut ids = Vec::new();
    collect_refs(object, &mut ids);
    for id in ids {
        doc.delete_object(id);
    }
}

fn collect_refs(object: &Object, ids: &mut Vec<ObjectId>) {
    match object {
        Object::Reference(id) => {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        Object::Array(items) => {
            for item in items {
                collect_refs(item, ids);
            }
        }
        Object::Dictionary(dict) => {
            for (_, value) in dict.iter() {
                collect_refs(value, ids);
            }
        }
        _ => {}
    }
}

fn resolve_dict<'a>(doc: &'a Document, object: &'a Object) -> Option<&'a Dictionary> {
    match object {
        Object::Dictionary(dict) => Some(dict),
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        Object::Stream(stream) => Some(&stream.dict),
        _ => None,
    }
}

fn object_dict(object: &Object) -> Option<&Dictionary> {
    match object {
        Object::Dictionary(dict) => Some(dict),
        Object::Stream(stream) => Some(&stream.dict),
        _ => None,
    }
}

fn action_is_js(doc: &Document, object: &Object) -> bool {
    let Some(dict) = resolve_dict(doc, object) else {
        return false;
    };
    dict.has(b"JS") || dict.get(b"S").and_then(Object::as_name).ok() == Some(b"JavaScript")
}

fn aa_has_js(doc: &Document, object: &Object) -> bool {
    let Some(dict) = resolve_dict(doc, object) else {
        return false;
    };
    dict.iter().any(|(_, value)| action_is_js(doc, value))
}

fn object_is_sig(doc: &Document, object: &Object) -> bool {
    resolve_dict(doc, object).is_some_and(dict_is_sig)
}

fn dict_is_sig(dict: &Dictionary) -> bool {
    dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Sig")
        || (dict.has(b"ByteRange") && dict.has(b"Contents") && dict.has(b"Filter"))
}

fn pdf_text(object: &Object) -> String {
    let bytes = match object {
        Object::String(bytes, _) => bytes.as_slice(),
        Object::Name(bytes) => bytes.as_slice(),
        Object::Integer(value) => return value.to_string(),
        _ => return String::new(),
    };
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (pairs, _) = bytes[2..].as_chunks::<2>();
        let units: Vec<u16> = pairs
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn labeled(prefix: &str, value: &str) -> String {
    if value.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}={}", truncate(value, 80))
    }
}

fn push_warning(warnings: &mut Vec<String>, warning: &str) {
    if !warnings.iter().any(|existing| existing == warning) {
        warnings.push(warning.to_string());
    }
}
