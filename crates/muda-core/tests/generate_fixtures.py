#!/usr/bin/env python3
"""Generate tiny JPEG/PNG fixtures with planted metadata strings."""

from __future__ import annotations

import os
import struct
import zlib
from io import BytesIO

from PIL import Image, PngImagePlugin

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures")
os.makedirs(ROOT, exist_ok=True)


def write(name: str, data: bytes) -> None:
    path = os.path.join(ROOT, name)
    with open(path, "wb") as fh:
        fh.write(data)
    print(
        f"{name:16} {len(data):5}B  "
        f"Exif={b'Exif' in data} GPS={b'GPS' in data} "
        f"SECRET={b'SECRET_TAG_XYZ' in data}"
    )


def jpeg_plain() -> bytes:
    im = Image.new("RGB", (16, 16), (10, 20, 30))
    buf = BytesIO()
    im.save(buf, format="JPEG", quality=90)
    data = buf.getvalue()
    write("jpeg_plain.jpg", data)
    return data


def tiff_entry(tag: int, typ: int, count: int, value_or_off: int) -> bytes:
    return struct.pack("<HHI I", tag, typ, count, value_or_off)


def build_exif_tiff() -> bytes:
    """Little-endian TIFF with Make, Software, ImageDescription=GPS, GPS IFD."""
    desc = b"GPS\x00"  # 4 bytes, fits inline
    make = b"Canon\x00"  # 6
    software = b"Adobe\x00"  # 6
    n_ifd0 = 4
    ifd0_size = 2 + n_ifd0 * 12 + 4  # 54
    ifd0_off = 8
    extra_off = ifd0_off + ifd0_size  # 62
    make_off = extra_off
    software_off = make_off + len(make)
    gps_ifd_off = software_off + len(software)

    n_gps = 5
    gps_ifd_size = 2 + n_gps * 12 + 4  # 66
    lat_off = gps_ifd_off + gps_ifd_size
    lon_off = lat_off + 24

    ifd0 = b"".join(
        [
            struct.pack("<H", n_ifd0),
            tiff_entry(0x010E, 2, len(desc), struct.unpack("<I", desc)[0]),
            tiff_entry(0x010F, 2, len(make), make_off),
            tiff_entry(0x0131, 2, len(software), software_off),
            tiff_entry(0x8825, 4, 1, gps_ifd_off),
            struct.pack("<I", 0),
        ]
    )

    gps_ver = b"\x02\x03\x00\x00"
    gps_ifd = b"".join(
        [
            struct.pack("<H", n_gps),
            tiff_entry(0x0000, 1, 4, struct.unpack("<I", gps_ver)[0]),
            tiff_entry(0x0001, 2, 2, struct.unpack("<I", b"N\x00\x00\x00")[0]),
            tiff_entry(0x0002, 5, 3, lat_off),
            tiff_entry(0x0003, 2, 2, struct.unpack("<I", b"W\x00\x00\x00")[0]),
            tiff_entry(0x0004, 5, 3, lon_off),
            struct.pack("<I", 0),
        ]
    )

    def rationals(values: list[tuple[int, int]]) -> bytes:
        return b"".join(struct.pack("<II", n, d) for n, d in values)

    lat = rationals([(37, 1), (46, 1), (0, 1)])
    lon = rationals([(122, 1), (25, 1), (0, 1)])

    header = b"II" + struct.pack("<HI", 42, ifd0_off)
    tiff = header + ifd0 + make + software + gps_ifd + lat + lon
    assert tiff[ifd0_off : ifd0_off + len(ifd0)] == ifd0
    return tiff


def jpeg_segment(marker: int, payload: bytes) -> bytes:
    return bytes([0xFF, marker]) + struct.pack(">H", len(payload) + 2) + payload


def insert_after_first_marker(jpeg: bytes, blob: bytes) -> bytes:
    if jpeg[:2] != b"\xff\xd8":
        raise SystemExit("not a jpeg")
    pos = 2
    if jpeg[pos : pos + 2] == b"\xff\xe0":
        length = struct.unpack(">H", jpeg[pos + 2 : pos + 4])[0]
        insert_at = pos + 2 + length
    else:
        insert_at = 2
    return jpeg[:insert_at] + blob + jpeg[insert_at:]


def jpeg_gps(plain: bytes) -> bytes:
    tiff = build_exif_tiff()
    app1 = jpeg_segment(0xE1, b"Exif\x00\x00" + tiff)
    com = jpeg_segment(0xFE, b"SECRET_TAG_XYZ")
    data = insert_after_first_marker(plain, app1 + com)
    if b"Exif" not in data or b"GPS" not in data or b"SECRET_TAG_XYZ" not in data:
        raise SystemExit("jpeg_gps missing planted strings")
    write("jpeg_gps.jpg", data)
    return data


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    crc = zlib.crc32(kind + payload) & 0xFFFFFFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", crc)


def png_text(tiff: bytes) -> None:
    im = Image.new("RGB", (16, 16), (200, 10, 10))
    meta = PngImagePlugin.PngInfo()
    meta.add_text("Comment", "SECRET_TAG_XYZ")
    meta.add_text("Software", "Adobe")
    buf = BytesIO()
    im.save(buf, format="PNG", pnginfo=meta)
    png = buf.getvalue()
    exif_chunk = png_chunk(b"eXIf", tiff)
    iend = png.rfind(b"IEND")
    png = png[: iend - 4] + exif_chunk + png[iend - 4 :]
    if b"SECRET_TAG_XYZ" not in png or b"eXIf" not in png or b"GPS" not in png:
        raise SystemExit("png_text missing planted chunks")
    write("png_text.png", png)


def main() -> None:
    plain = jpeg_plain()
    gps = jpeg_gps(plain)
    app1 = gps.find(b"\xff\xe1")
    length = struct.unpack(">H", gps[app1 + 2 : app1 + 4])[0]
    payload = gps[app1 + 4 : app1 + 2 + length]
    tiff = payload[6:] if payload.startswith(b"Exif\x00\x00") else payload
    png_text(tiff)


if __name__ == "__main__":
    main()
