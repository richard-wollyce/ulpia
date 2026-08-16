"""Generates the tray icons, in pure Python so the build needs no image toolchain.

The progress ring exists because a tray icon has no progress API on any platform.
The only mechanism available is swapping the image, which is what the reference
video did too. Eleven frames is enough to read as motion and few enough to keep
the binary small.

Run once: python make-icons.py
"""
import math
import struct
import zlib
from pathlib import Path

HERE = Path(__file__).parent
ICONS = HERE / "src-tauri" / "icons"
ICONS.mkdir(parents=True, exist_ok=True)


def png(path: Path, size: int, pixels) -> None:
    """Writes an RGBA PNG. `pixels(x, y)` returns (r, g, b, a)."""
    raw = bytearray()
    for y in range(size):
        raw.append(0)  # filter type 0, none
        for x in range(size):
            raw.extend(pixels(x, y))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def rounded(x, y, size, inset, radius):
    """Signed coverage of a rounded square, antialiased at the edge."""
    lo, hi = inset, size - inset
    cx = min(max(x + 0.5, lo + radius), hi - radius)
    cy = min(max(y + 0.5, lo + radius), hi - radius)
    d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
    return max(0.0, min(1.0, radius - d + 0.5))


def mark(size: int, progress: float | None = None):
    """The mark: a rounded square with three index bars, optionally a progress arc.

    Drawn in white with alpha. Windows tray icons sit on an unknown background and
    a white mark with a dark outline reads on both; the icon is also declared as a
    template on macOS, which inverts it per theme.
    """
    unit = size / 32.0
    bars = [(9, 12, 14), (9, 16, 10), (9, 20, 6)]  # x, y, width, in 32px units

    def px(x, y):
        cover = rounded(x, y, size, 2 * unit, 7 * unit)
        if cover <= 0:
            return (0, 0, 0, 0)

        r, g, b, a = 24, 24, 27, int(235 * cover)

        for bx, by, bw in bars:
            if (
                bx * unit <= x < (bx + bw) * unit
                and by * unit <= y < (by + 2.5) * unit
            ):
                r, g, b = 250, 250, 250

        if progress is not None:
            # A filled bar across the bottom, the one shape still legible at 16px.
            top = 25 * unit
            if top <= y < 29 * unit and 5 * unit <= x < 27 * unit:
                filled = 5 * unit + progress * 22 * unit
                r, g, b = (110, 231, 183) if x < filled else (63, 63, 70)

        return (r, g, b, a)

    return px


# The source the Tauri icon generator expands into every platform format.
png(ICONS / "source.png", 512, mark(512))

# The tray icon itself, plus one frame per progress step.
png(ICONS / "tray.png", 32, mark(32))
for step in range(11):
    png(ICONS / f"tray-{step * 10:03d}.png", 32, mark(32, progress=step / 10))

print(f"wrote {len(list(ICONS.glob('*.png')))} icons to {ICONS}")
