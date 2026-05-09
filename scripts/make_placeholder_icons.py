#!/usr/bin/env python3
"""生成 Tauri 占位 PNG 图标（cargo check / dev 用；正式发布请用 `pnpm tauri icon` 重新生成）。"""

from __future__ import annotations

import os
import struct
import sys
import zlib
from pathlib import Path

PRIMARY = (0x4F, 0x6F, 0xFF, 0xFF)


def make_png(path: Path, size: int) -> None:
    width = height = size
    color = bytes(PRIMARY)
    raw = bytearray()
    for _ in range(height):
        raw.append(0)  # filter byte (None)
        for _ in range(width):
            raw.extend(color)
    idat = zlib.compress(bytes(raw), 9)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b""))


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    icons = root / "src-tauri" / "icons"
    make_png(icons / "32x32.png", 32)
    make_png(icons / "128x128.png", 128)
    make_png(icons / "128x128@2x.png", 256)
    make_png(icons / "icon.png", 512)
    print(f"placeholder icons written to {icons}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
