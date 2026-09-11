use wasm_bindgen::JsCast;

pub(crate) fn collect_file_list(list: &web_sys::FileList) -> Vec<web_sys::File> {
    (0..list.length()).filter_map(|i| list.item(i)).collect()
}

pub(crate) async fn read_file_bytes(file: web_sys::File) -> Result<Vec<u8>, String> {
    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
        .await
        .map_err(|e| format!("{e:?}"))?;
    let array = js_sys::Uint8Array::new(&buffer);
    let mut bytes = vec![0u8; array.length() as usize];
    array.copy_to(&mut bytes);
    Ok(bytes)
}

pub(crate) fn download_bytes(filename: &str, mime: &str, bytes: &[u8]) -> Result<(), String> {
    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
        .map_err(|e| format!("blob: {e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("object url: {e:?}"))?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| "no document".to_string())?;
    let anchor = document
        .create_element("a")
        .map_err(|e| format!("element: {e:?}"))?;
    anchor
        .set_attribute("href", &url)
        .map_err(|e| format!("href: {e:?}"))?;
    anchor
        .set_attribute("download", filename)
        .map_err(|e| format!("download: {e:?}"))?;
    let element: web_sys::HtmlElement = anchor
        .dyn_into()
        .map_err(|_| "anchor cast failed".to_string())?;
    element.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

pub(crate) fn format_bytes(n: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    if n < 1024 {
        format!("{n} B")
    } else if (n as f64) < MB {
        format!("{:.1} KB", n as f64 / KB)
    } else {
        format!("{:.1} MB", n as f64 / MB)
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn format_bytes_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }
}
