//! Filter-and-rebuild EXIF TIFF so kept tag families can stay without
//! leaking the rest of the IFD (GPS, MakerNote, dates, thumbnail, …).

use crate::TagFamily;

const TAG_IMAGE_DESCRIPTION: u16 = 0x010E;
const TAG_MAKE: u16 = 0x010F;
const TAG_MODEL: u16 = 0x0110;
const TAG_SOFTWARE: u16 = 0x0131;
const TAG_ARTIST: u16 = 0x013B;
const TAG_THUMB_OFFSET: u16 = 0x0201;
const TAG_THUMB_LENGTH: u16 = 0x0202;
const TAG_COPYRIGHT: u16 = 0x8298;
const TAG_EXIF_IFD: u16 = 0x8769;
const TAG_GPS_IFD: u16 = 0x8825;
const TAG_EXIF_VERSION: u16 = 0x9000;
const TAG_MAKER_NOTE: u16 = 0x927C;
const TAG_USER_COMMENT: u16 = 0x9286;
const TAG_INTEROP: u16 = 0xA005;
const TAG_CAMERA_OWNER: u16 = 0xA430;
const TAG_BODY_SERIAL: u16 = 0xA431;
const TAG_LENS_MAKE: u16 = 0xA433;
const TAG_LENS_MODEL: u16 = 0xA434;
const TAG_LENS_SERIAL: u16 = 0xA435;
const TAG_UNIQUE_CAMERA_MODEL: u16 = 0xC614;
const TAG_CAMERA_SERIAL_NUMBER: u16 = 0xC62F;

const TIFF_MAGIC: u16 = 42;
const TYPE_LONG: u16 = 4;
const TYPE_UNDEFINED: u16 = 7;
const MAX_IFD_ENTRIES: usize = 256;
const DEFAULT_EXIF_VERSION: &[u8] = b"0230";

#[derive(Clone, Copy)]
struct Endian {
    little: bool,
}

impl Endian {
    fn u16(self, b: &[u8]) -> u16 {
        if self.little {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        }
    }

    fn u32(self, b: &[u8]) -> u32 {
        if self.little {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        }
    }

    fn write_u16(self, v: u16) -> [u8; 2] {
        if self.little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }

    fn write_u32(self, v: u32) -> [u8; 4] {
        if self.little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IfdKind {
    Ifd0,
    Exif,
    Gps,
}

#[derive(Clone)]
struct Entry {
    tag: u16,
    type_: u16,
    count: u32,
    data: Vec<u8>,
}

/// Rebuild a TIFF with only `keep` families. `None` means drop the EXIF blob.
pub(crate) fn rewrite_exif_tiff(tiff: &[u8], keep: &[TagFamily]) -> Option<Vec<u8>> {
    if keep.is_empty() {
        return None;
    }
    rewrite_inner(tiff, keep)
}

pub(crate) fn tiff_from_exif_payload(payload: &[u8]) -> &[u8] {
    if payload.starts_with(b"Exif\0\0") && payload.len() > 6 {
        &payload[6..]
    } else {
        payload
    }
}

pub(crate) fn jpeg_exif_payload(tiff: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + tiff.len());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(tiff);
    out
}

fn rewrite_inner(tiff: &[u8], keep: &[TagFamily]) -> Option<Vec<u8>> {
    let (endian, ifd0) = parse_header_and_ifd0(tiff)?;
    let exif_off = pointer_offset(&ifd0, TAG_EXIF_IFD, endian);
    let gps_off = pointer_offset(&ifd0, TAG_GPS_IFD, endian);
    let exif_orig = exif_off.and_then(|off| parse_ifd(tiff, endian, off));
    let gps_orig = gps_off.and_then(|off| parse_ifd(tiff, endian, off));

    let orig_exif_version = exif_orig
        .as_ref()
        .and_then(|entries| entries.iter().find(|e| e.tag == TAG_EXIF_VERSION).cloned());

    let ifd0_kept: Vec<Entry> = ifd0
        .into_iter()
        .filter(|e| should_keep(IfdKind::Ifd0, e.tag, keep))
        .collect();
    let mut exif_kept: Vec<Entry> = exif_orig
        .unwrap_or_default()
        .into_iter()
        .filter(|e| should_keep(IfdKind::Exif, e.tag, keep))
        .collect();
    let gps_kept: Vec<Entry> = gps_orig
        .unwrap_or_default()
        .into_iter()
        .filter(|e| should_keep(IfdKind::Gps, e.tag, keep))
        .collect();

    if ifd0_kept.is_empty() && exif_kept.is_empty() && gps_kept.is_empty() {
        return None;
    }

    if !exif_kept.is_empty() && !exif_kept.iter().any(|e| e.tag == TAG_EXIF_VERSION) {
        exif_kept.push(orig_exif_version.unwrap_or(Entry {
            tag: TAG_EXIF_VERSION,
            type_: TYPE_UNDEFINED,
            count: 4,
            data: DEFAULT_EXIF_VERSION.to_vec(),
        }));
    }

    Some(write_tiff(endian, ifd0_kept, exif_kept, gps_kept))
}

fn should_keep(kind: IfdKind, tag: u16, keep: &[TagFamily]) -> bool {
    if is_structural_tag(tag) {
        return false;
    }
    keep.contains(&classify_tag(kind, tag))
}

fn is_structural_tag(tag: u16) -> bool {
    matches!(
        tag,
        TAG_EXIF_IFD
            | TAG_GPS_IFD
            | TAG_INTEROP
            | TAG_THUMB_OFFSET
            | TAG_THUMB_LENGTH
            | TAG_EXIF_VERSION
    )
}

fn classify_tag(kind: IfdKind, tag: u16) -> TagFamily {
    if kind == IfdKind::Gps {
        return TagFamily::Gps;
    }
    match tag {
        TAG_MAKE
        | TAG_MODEL
        | TAG_MAKER_NOTE
        | TAG_CAMERA_OWNER
        | TAG_BODY_SERIAL
        | TAG_LENS_MAKE
        | TAG_LENS_MODEL
        | TAG_LENS_SERIAL
        | TAG_UNIQUE_CAMERA_MODEL
        | TAG_CAMERA_SERIAL_NUMBER => TagFamily::Camera,
        TAG_SOFTWARE => TagFamily::Software,
        TAG_IMAGE_DESCRIPTION | TAG_ARTIST | TAG_COPYRIGHT | TAG_USER_COMMENT => TagFamily::Comment,
        _ => TagFamily::Other,
    }
}

// Used by classify_tag; keep the enum crate-private by not exporting IfdKind.
// classify_tag is only used here.

fn parse_header_and_ifd0(data: &[u8]) -> Option<(Endian, Vec<Entry>)> {
    if data.len() < 8 {
        return None;
    }
    let little = match &data[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let endian = Endian { little };
    if endian.u16(&data[2..4]) != TIFF_MAGIC {
        return None;
    }
    let ifd0_off = endian.u32(&data[4..8]) as usize;
    let ifd0 = parse_ifd(data, endian, ifd0_off)?;
    Some((endian, ifd0))
}

fn parse_ifd(data: &[u8], endian: Endian, offset: usize) -> Option<Vec<Entry>> {
    if offset.checked_add(2)? > data.len() {
        return None;
    }
    let count = endian.u16(&data[offset..offset + 2]) as usize;
    if count == 0 || count > MAX_IFD_ENTRIES {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let eoff = offset + 2 + i * 12;
        if eoff + 12 > data.len() {
            return None;
        }
        let tag = endian.u16(&data[eoff..eoff + 2]);
        let type_ = endian.u16(&data[eoff + 2..eoff + 4]);
        let value_count = endian.u32(&data[eoff + 4..eoff + 8]);
        let vor = &data[eoff + 8..eoff + 12];
        let unit = type_size(type_)?;
        let size = (unit as u64).checked_mul(u64::from(value_count))? as usize;
        if size > data.len() {
            return None;
        }
        let bytes = if size <= 4 {
            vor[..size].to_vec()
        } else {
            let data_off = endian.u32(vor) as usize;
            let end = data_off.checked_add(size)?;
            if end > data.len() {
                return None;
            }
            data[data_off..end].to_vec()
        };
        entries.push(Entry {
            tag,
            type_,
            count: value_count,
            data: bytes,
        });
    }
    Some(entries)
}

fn type_size(type_: u16) -> Option<u32> {
    Some(match type_ {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    })
}

fn pointer_offset(entries: &[Entry], tag: u16, endian: Endian) -> Option<usize> {
    let entry = entries.iter().find(|e| e.tag == tag)?;
    if entry.data.len() >= 4 {
        Some(endian.u32(&entry.data[..4]) as usize)
    } else {
        None
    }
}

fn write_tiff(
    endian: Endian,
    mut ifd0: Vec<Entry>,
    mut exif: Vec<Entry>,
    mut gps: Vec<Entry>,
) -> Vec<u8> {
    exif.sort_by_key(|e| e.tag);
    gps.sort_by_key(|e| e.tag);

    if !exif.is_empty() {
        ifd0.push(long_pointer(TAG_EXIF_IFD, 0, endian));
    }
    if !gps.is_empty() {
        ifd0.push(long_pointer(TAG_GPS_IFD, 0, endian));
    }
    ifd0.sort_by_key(|e| e.tag);

    let ifd0_off = 8usize;
    let ifd0_dir = dir_len(ifd0.len());
    let ifd0_extra = extra_len(&ifd0);
    let mut cursor = ifd0_off + ifd0_dir + ifd0_extra;

    let exif_off = cursor;
    let exif_dir = if exif.is_empty() {
        0
    } else {
        dir_len(exif.len())
    };
    let exif_extra = extra_len(&exif);
    cursor += exif_dir + exif_extra;

    let gps_off = cursor;

    if !exif.is_empty() {
        set_long_pointer(&mut ifd0, TAG_EXIF_IFD, exif_off as u32, endian);
    }
    if !gps.is_empty() {
        set_long_pointer(&mut ifd0, TAG_GPS_IFD, gps_off as u32, endian);
    }

    let mut out = Vec::new();
    if endian.little {
        out.extend_from_slice(b"II");
    } else {
        out.extend_from_slice(b"MM");
    }
    out.extend_from_slice(&endian.write_u16(TIFF_MAGIC));
    out.extend_from_slice(&endian.write_u32(ifd0_off as u32));
    write_ifd(&mut out, endian, &ifd0);
    if !exif.is_empty() {
        write_ifd(&mut out, endian, &exif);
    }
    if !gps.is_empty() {
        write_ifd(&mut out, endian, &gps);
    }
    out
}

fn long_pointer(tag: u16, offset: u32, endian: Endian) -> Entry {
    Entry {
        tag,
        type_: TYPE_LONG,
        count: 1,
        data: endian.write_u32(offset).to_vec(),
    }
}

fn set_long_pointer(entries: &mut [Entry], tag: u16, offset: u32, endian: Endian) {
    if let Some(entry) = entries.iter_mut().find(|e| e.tag == tag) {
        *entry = long_pointer(tag, offset, endian);
    }
}

fn dir_len(n: usize) -> usize {
    2 + 12 * n + 4
}

fn extra_len(entries: &[Entry]) -> usize {
    entries
        .iter()
        .map(|e| {
            if e.data.len() > 4 {
                (e.data.len() + 1) & !1
            } else {
                0
            }
        })
        .sum()
}

fn write_ifd(out: &mut Vec<u8>, endian: Endian, entries: &[Entry]) {
    out.extend_from_slice(&endian.write_u16(entries.len() as u16));
    let dir_end = out.len() + 12 * entries.len() + 4;
    let mut extra_at = dir_end;
    let mut extra_offs = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.data.len() > 4 {
            extra_offs.push(Some(extra_at));
            extra_at += (entry.data.len() + 1) & !1;
        } else {
            extra_offs.push(None);
        }
    }
    for (entry, extra) in entries.iter().zip(extra_offs.iter()) {
        out.extend_from_slice(&endian.write_u16(entry.tag));
        out.extend_from_slice(&endian.write_u16(entry.type_));
        out.extend_from_slice(&endian.write_u32(entry.count));
        if let Some(off) = extra {
            out.extend_from_slice(&endian.write_u32(*off as u32));
        } else {
            let mut buf = [0u8; 4];
            let n = entry.data.len().min(4);
            buf[..n].copy_from_slice(&entry.data[..n]);
            out.extend_from_slice(&buf);
        }
    }
    out.extend_from_slice(&endian.write_u32(0));
    for entry in entries {
        if entry.data.len() > 4 {
            out.extend_from_slice(&entry.data);
            if entry.data.len() % 2 == 1 {
                out.push(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg_fixture_tiff() -> Vec<u8> {
        let path = format!("{}/tests/fixtures/jpeg_gps.jpg", env!("CARGO_MANIFEST_DIR"));
        let jpeg = std::fs::read(&path).unwrap();
        let app1 = jpeg.windows(2).position(|w| w == [0xFF, 0xE1]).unwrap();
        let len = u16::from_be_bytes([jpeg[app1 + 2], jpeg[app1 + 3]]) as usize;
        let payload = &jpeg[app1 + 4..app1 + 2 + len];
        tiff_from_exif_payload(payload).to_vec()
    }

    fn names(tiff: &[u8]) -> Vec<String> {
        let reader = exif::Reader::new().read_raw(tiff.to_vec()).unwrap();
        reader.fields().map(|f| f.tag.to_string()).collect()
    }

    #[test]
    fn empty_keep_drops_tiff() {
        let tiff = jpeg_fixture_tiff();
        assert!(rewrite_exif_tiff(&tiff, &[]).is_none());
    }

    #[test]
    fn keep_gps_only_roundtrip() {
        let tiff = jpeg_fixture_tiff();
        let out = rewrite_exif_tiff(&tiff, &[TagFamily::Gps]).expect("gps tiff");
        let tags = names(&out);
        assert!(
            tags.iter().any(|t| t.starts_with("GPSLatitude")),
            "{tags:?}"
        );
        assert!(
            tags.iter().any(|t| t.starts_with("GPSLongitude")),
            "{tags:?}"
        );
        assert!(!tags.iter().any(|t| t == "Make"), "{tags:?}");
        assert!(!tags.iter().any(|t| t == "Software"), "{tags:?}");
        assert!(!tags.iter().any(|t| t == "ImageDescription"), "{tags:?}");
    }

    #[test]
    fn keep_software_drops_gps() {
        let tiff = jpeg_fixture_tiff();
        let out = rewrite_exif_tiff(&tiff, &[TagFamily::Software]).expect("software tiff");
        let tags = names(&out);
        assert!(tags.iter().any(|t| t == "Software"), "{tags:?}");
        assert!(!tags.iter().any(|t| t.starts_with("GPS")), "{tags:?}");
    }
}
