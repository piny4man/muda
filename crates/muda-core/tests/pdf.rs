use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use muda_core::{sniff_kind, strip_image, strip_image_selective, FileKind, StripError, TagFamily};

fn has(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn save(doc: &mut Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save pdf");
    bytes
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|err| panic!("read {path}: {err}"))
}

fn page_shell() -> (Document, ObjectId, ObjectId) {
    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let content = Content {
        operations: vec![Operation::new(
            "Tj",
            vec![Object::string_literal("KEEP_THIS_TEXT")],
        )],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(200),
            Object::Integer(200),
        ],
        "Contents" => content_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));
    (doc, page_id, catalog_id)
}

fn rich_pdf() -> Vec<u8> {
    let (mut doc, page_id, catalog_id) = page_shell();
    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("KEEP_TITLE"),
        "Author" => Object::string_literal("SECRET_AUTHOR_XYZ"),
        "Creator" => Object::string_literal("SECRET_CREATOR_XYZ"),
        "Producer" => Object::string_literal("SECRET_PRODUCER_XYZ"),
        "Subject" => Object::string_literal("SECRET_SUBJECT_XYZ"),
        "Keywords" => Object::string_literal("SECRET_KEYWORDS_XYZ"),
        "CreationDate" => Object::string_literal("D:20200101120000Z"),
        "ModDate" => Object::string_literal("D:20200102120000Z"),
        "Company" => Object::string_literal("SECRET_COMPANY_XYZ"),
    });
    doc.trailer.set("Info", Object::Reference(info_id));
    doc.trailer.set(
        "ID",
        vec![
            Object::string_literal("OLD_DOCUMENT_ID_XYZ"),
            Object::string_literal("OLD_DOCUMENT_ID_XYZ"),
        ],
    );

    let xmp = b"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?><x:xmpmeta>SECRET_XMP_PACKET</x:xmpmeta><?xpacket end='w'?>";
    let meta_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        xmp.to_vec(),
    ));
    doc.get_dictionary_mut(catalog_id)
        .unwrap()
        .set("Metadata", Object::Reference(meta_id));

    let note_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Text",
        "Contents" => Object::string_literal("KEEP_NOTE_TEXT"),
        "T" => Object::string_literal("SECRET_ANNOT_AUTHOR"),
        "M" => Object::string_literal("D:20200103120000Z"),
        "Rect" => vec![
            Object::Integer(1),
            Object::Integer(1),
            Object::Integer(20),
            Object::Integer(20),
        ],
    });
    doc.get_dictionary_mut(page_id)
        .unwrap()
        .set("Annots", vec![Object::Reference(note_id)]);

    save(&mut doc)
}

fn page_text(bytes: &[u8]) -> Vec<u8> {
    let doc = Document::load_mem(bytes).expect("reload pdf");
    let page_id = doc.page_iter().next().expect("page");
    doc.get_page_content(page_id)
}

#[test]
fn pdf_info_xmp_and_text() {
    let input = rich_pdf();
    assert_eq!(sniff_kind(&input), Some(FileKind::Pdf));
    assert!(has(&input, b"SECRET_AUTHOR_XYZ"));
    assert!(has(&input, b"SECRET_XMP_PACKET"));
    assert!(has(&input, b"KEEP_THIS_TEXT"));

    let (output, report) = strip_image("notes.pdf", &input).expect("strip pdf");
    assert_eq!(report.kind, FileKind::Pdf);
    assert_eq!(report.output_name, "notes.cleaned.pdf");
    assert!(output.starts_with(b"%PDF-"));
    assert!(has(&page_text(&output), b"KEEP_THIS_TEXT"));
    assert!(has(&output, b"KEEP_TITLE"));
    assert!(has(&output, b"KEEP_NOTE_TEXT"));

    for secret in [
        b"SECRET_AUTHOR_XYZ".as_slice(),
        b"SECRET_CREATOR_XYZ",
        b"SECRET_PRODUCER_XYZ",
        b"SECRET_SUBJECT_XYZ",
        b"SECRET_KEYWORDS_XYZ",
        b"SECRET_COMPANY_XYZ",
        b"SECRET_XMP_PACKET",
        b"SECRET_ANNOT_AUTHOR",
        b"OLD_DOCUMENT_ID_XYZ",
    ] {
        assert!(
            !has(&output, secret),
            "still present: {}",
            String::from_utf8_lossy(secret)
        );
    }

    let labels: Vec<&str> = report
        .removed
        .iter()
        .map(|tag| tag.label.as_str())
        .collect();
    for prefix in [
        "Author=",
        "Creator=",
        "Producer=",
        "CreationDate=",
        "ModDate=",
        "Info:Company=",
    ] {
        assert!(
            labels.iter().any(|label| label.starts_with(prefix)),
            "missing {prefix} in {labels:?}"
        );
    }
    assert!(report
        .removed
        .iter()
        .any(|tag| tag.family == TagFamily::Xmp));
    assert!(labels.contains(&"ID"));

    let (_again, second) = strip_image("notes.pdf", &output).expect("second strip");
    assert!(
        !second
            .removed
            .iter()
            .any(|tag| tag.label.starts_with("Author")),
        "{:?}",
        second.removed
    );
}

#[test]
fn pdf_keep_software_and_comments() {
    let input = rich_pdf();

    let (software, report) =
        strip_image_selective("notes.pdf", &input, &[TagFamily::Software]).unwrap();
    assert!(has(&software, b"SECRET_CREATOR_XYZ"));
    assert!(has(&software, b"SECRET_PRODUCER_XYZ"));
    assert!(!has(&software, b"SECRET_AUTHOR_XYZ"));
    assert!(report
        .removed
        .iter()
        .any(|tag| tag.label.starts_with("Author=")));
    assert!(!report
        .removed
        .iter()
        .any(|tag| tag.label.starts_with("Creator=")));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("XMP")));

    let (comments, report) =
        strip_image_selective("notes.pdf", &input, &[TagFamily::Comment]).unwrap();
    assert!(has(&comments, b"SECRET_AUTHOR_XYZ"));
    assert!(has(&comments, b"SECRET_SUBJECT_XYZ"));
    assert!(has(&comments, b"SECRET_KEYWORDS_XYZ"));
    assert!(has(&comments, b"SECRET_ANNOT_AUTHOR"));
    assert!(!has(&comments, b"SECRET_CREATOR_XYZ"));
    assert!(!report
        .removed
        .iter()
        .any(|tag| tag.label.starts_with("Author=")));
}

#[test]
fn pdf_incremental_revision_is_dropped() {
    let input = incremental_pdf();
    assert!(has(&input, b"SECRET_AUTHOR_XYZ"));
    let (output, _) = strip_image("rev.pdf", &input).expect("strip incremental");
    assert!(output.starts_with(b"%PDF-"));
    assert!(!has(&output, b"SECRET_AUTHOR_XYZ"));
}

#[test]
fn pdf_embedded_jpeg_gps() {
    let jpeg = fixture("jpeg_gps.jpg");
    let lat = [
        0x25, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x2e, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00,
    ];
    assert!(has(&jpeg, b"SECRET_TAG_XYZ"));
    assert!(has(&jpeg, &lat));

    let (mut doc, page_id, _) = page_shell();
    let image_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 16,
            "Height" => 16,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "Filter" => "DCTDecode",
        },
        jpeg,
    ));
    doc.get_dictionary_mut(page_id).unwrap().set(
        "Resources",
        dictionary! {
            "XObject" => dictionary! {
                "Im0" => Object::Reference(image_id),
            },
        },
    );
    let input = save(&mut doc);
    assert!(has(&input, b"SECRET_TAG_XYZ"));

    let (output, report) = strip_image("scan.pdf", &input).unwrap();
    assert!(has(&output, b"DCTDecode"), "image filter should stay");
    assert!(!has(&output, b"SECRET_TAG_XYZ"));
    assert!(!has(&output, &lat));
    assert!(report
        .removed
        .iter()
        .any(|tag| tag.family == TagFamily::Gps));
    assert!(has(&page_text(&output), b"KEEP_THIS_TEXT"));

    let (kept, _) = strip_image_selective("scan.pdf", &input, &[TagFamily::Gps]).unwrap();
    assert!(has(&kept, &lat));
    assert!(!has(&kept, b"SECRET_TAG_XYZ"));
}

#[test]
fn pdf_script_attachment_and_signature() {
    let (mut doc, page_id, catalog_id) = page_shell();
    let script_id = doc.add_object(dictionary! {
        "S" => "JavaScript",
        "JS" => Object::string_literal("SECRET_SCRIPT_BODY"),
    });
    doc.get_dictionary_mut(catalog_id)
        .unwrap()
        .set("OpenAction", Object::Reference(script_id));

    let file_id = doc.add_object(Stream::new(
        dictionary! { "Type" => "EmbeddedFile" },
        b"SECRET_ATTACHMENT_BYTES".to_vec(),
    ));
    let spec_id = doc.add_object(dictionary! {
        "Type" => "Filespec",
        "F" => Object::string_literal("secret.bin"),
        "EF" => dictionary! { "F" => Object::Reference(file_id) },
    });
    let sig_id = doc.add_object(dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "Name" => Object::string_literal("SECRET_SIGNER"),
        "ByteRange" => vec![
            Object::Integer(0),
            Object::Integer(8),
            Object::Integer(16),
            Object::Integer(24),
        ],
        "Contents" => Object::string_literal("SECRET_SIG_BYTES"),
    });
    let widget_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "V" => Object::Reference(sig_id),
        "Rect" => vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(10),
            Object::Integer(10),
        ],
    });
    let attach_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "FileAttachment",
        "FS" => Object::Reference(spec_id),
        "Contents" => Object::string_literal("KEEP_NOTE_TEXT"),
        "Rect" => vec![
            Object::Integer(12),
            Object::Integer(12),
            Object::Integer(20),
            Object::Integer(20),
        ],
    });
    doc.get_dictionary_mut(page_id).unwrap().set(
        "Annots",
        vec![Object::Reference(widget_id), Object::Reference(attach_id)],
    );
    doc.get_dictionary_mut(catalog_id)
        .unwrap()
        .set("AcroForm", dictionary! { "Fields" => Vec::<Object>::new() });

    let input = save(&mut doc);
    let (output, report) = strip_image("bundle.pdf", &input).unwrap();
    assert!(!has(&output, b"SECRET_SCRIPT_BODY"));
    assert!(!has(&output, b"SECRET_ATTACHMENT_BYTES"));
    assert!(!has(&output, b"SECRET_SIGNER"));
    assert!(!has(&output, b"SECRET_SIG_BYTES"));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("JavaScript")));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("embedded files")));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("digital signature")));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("form values")));
}

#[test]
fn pdf_encrypted_is_rejected() {
    let (mut doc, _, _) = page_shell();
    let encrypt_id = doc.add_object(dictionary! {
        "Filter" => "Standard",
        "V" => 1,
        "R" => 2,
        "P" => -4,
        "O" => Object::string_literal("0123456789abcdef0123456789abcdef"),
        "U" => Object::string_literal("0123456789abcdef0123456789abcdef"),
    });
    doc.trailer.set("Encrypt", Object::Reference(encrypt_id));
    let input = save(&mut doc);
    match strip_image("locked.pdf", &input) {
        Err(StripError::InvalidImage(message)) => {
            assert_eq!(message, "encrypted PDF; decryption is not supported");
        }
        other => panic!("expected encrypted error, got {other:?}"),
    }
}

fn incremental_pdf() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend(b"%PDF-1.4\n");
    let mut offsets = [0usize; 5];

    fn push(buf: &mut Vec<u8>, offsets: &mut [usize], num: usize, body: &str) {
        offsets[num] = buf.len();
        buf.extend(format!("{num} 0 obj\n{body}\nendobj\n").as_bytes());
    }

    push(
        &mut buf,
        &mut offsets,
        1,
        "<< /Type /Catalog /Pages 2 0 R >>",
    );
    push(
        &mut buf,
        &mut offsets,
        2,
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    );
    push(
        &mut buf,
        &mut offsets,
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>",
    );
    push(
        &mut buf,
        &mut offsets,
        4,
        "<< /Author (SECRET_AUTHOR_XYZ) /Title (Notes) /Creator (Word) >>",
    );

    let xref1 = buf.len();
    buf.extend(b"xref\n0 5\n");
    buf.extend(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        buf.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    buf.extend(
        format!("trailer\n<< /Size 5 /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref1}\n%%EOF\n")
            .as_bytes(),
    );

    let gen1 = buf.len();
    buf.extend(b"4 1 obj\n<< /Title (Notes) >>\nendobj\n");
    let xref2 = buf.len();
    buf.extend(b"xref\n4 1\n");
    buf.extend(format!("{gen1:010} 00001 n \n").as_bytes());
    buf.extend(
        format!(
            "trailer\n<< /Size 5 /Root 1 0 R /Info 4 1 R /Prev {xref1} >>\nstartxref\n{xref2}\n%%EOF\n"
        )
        .as_bytes(),
    );
    buf
}
