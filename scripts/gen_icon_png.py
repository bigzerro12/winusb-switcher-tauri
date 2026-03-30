"""Emit a minimal 1024x1024 RGB PNG (solid color) for `yarn tauri icon` / `npx tauri icon`."""
import struct
import zlib


def _chunk(chunk_type: bytes, data: bytes) -> bytes:
    return (
        struct.pack("!I", len(data))
        + chunk_type
        + data
        + struct.pack("!I", zlib.crc32(chunk_type + data) & 0xFFFFFFFF)
    )


def write_png(path: str, w: int, h: int, rgb: tuple[int, int, int] = (70, 130, 180)) -> None:
    r, g, b = rgb
    ihdr = struct.pack("!IIBBBBB", w, h, 8, 2, 0, 0, 0)
    line = bytes([r, g, b] * w)
    raw = b"".join(b"\x00" + line for _ in range(h))
    idat = zlib.compress(raw, 9)
    sig = b"\x89PNG\r\n\x1a\n"
    with open(path, "wb") as f:
        f.write(sig)
        f.write(_chunk(b"IHDR", ihdr))
        f.write(_chunk(b"IDAT", idat))
        f.write(_chunk(b"IEND", b""))


if __name__ == "__main__":
    import sys

    out = sys.argv[1] if len(sys.argv) > 1 else "icon-source.png"
    write_png(out, 1024, 1024)
    print(f"wrote {out}")
