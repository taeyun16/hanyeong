"""Build assets/AppIcon.icns from assets/icon.svg.

Quick Look renders the SVG onto an opaque white page, so the pixels outside the rounded square are
made transparent here, with the same geometry as the SVG (x, y 100..924, corner radius 185). Only the
Python standard library and the macOS tools qlmanage, sips and iconutil are used.

    python3 tools/make_icon.py
"""
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parent.parent
SIZE, EDGE, RADIUS = 1024, 100, 185
SIZES = [16, 32, 128, 256, 512]


def chunks(data):
    position = 8
    while position < len(data):
        length, kind = struct.unpack('>I4s', data[position:position + 8])
        yield kind, data[position + 8:position + 8 + length]
        position += 12 + length


def decode(path):
    """Return the RGBA rows of an 8-bit RGBA PNG."""
    data = path.read_bytes()
    header = next(body for kind, body in chunks(data) if kind == b'IHDR')
    width, height, depth, colour = struct.unpack('>IIBB', header[:10])
    if (width, height, depth, colour) != (SIZE, SIZE, 8, 6):
        raise SystemExit('expected a %dx%d 8-bit RGBA PNG, got %r' % (SIZE, SIZE, (width, height, depth, colour)))
    raw = zlib.decompress(b''.join(body for kind, body in chunks(data) if kind == b'IDAT'))
    stride, rows, previous = width * 4, [], bytearray(width * 4)
    for y in range(height):
        kind, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for x in range(stride):
            left = line[x - 4] if x >= 4 else 0
            up, corner = previous[x], previous[x - 4] if x >= 4 else 0
            if kind == 1:
                line[x] = (line[x] + left) & 255
            elif kind == 2:
                line[x] = (line[x] + up) & 255
            elif kind == 3:
                line[x] = (line[x] + (left + up) // 2) & 255
            elif kind == 4:
                estimate = left + up - corner
                nearest = min((abs(estimate - left), 0, left), (abs(estimate - up), 1, up), (abs(estimate - corner), 2, corner))
                line[x] = (line[x] + nearest[2]) & 255
        rows.append(line)
        previous = line
    return rows


def coverage(x, y):
    """How much of pixel (x, y) lies inside the rounded square: 0..1, sampled 4x4."""
    inside = 0
    low, high = EDGE + RADIUS, SIZE - EDGE - RADIUS
    for sub_y in range(4):
        for sub_x in range(4):
            px, py = x + (sub_x + 0.5) / 4, y + (sub_y + 0.5) / 4
            if not (EDGE <= px <= SIZE - EDGE and EDGE <= py <= SIZE - EDGE):
                continue
            cx, cy = min(max(px, low), high), min(max(py, low), high)
            inside += (px - cx) ** 2 + (py - cy) ** 2 <= RADIUS ** 2
    return inside / 16


def mask(rows):
    low, high = EDGE + RADIUS, SIZE - EDGE - RADIUS
    for y, line in enumerate(rows):
        for x in range(SIZE):
            # Away from the edge strips and corners the pixel is fully inside: leave it.
            if EDGE + 1 <= x < SIZE - EDGE - 1 and low <= y < high or low <= x < high and EDGE + 1 <= y < SIZE - EDGE - 1:
                continue
            line[x * 4 + 3] = round(255 * coverage(x, y))


def encode(rows, path):
    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body))
    raw = b''.join(b'\x00' + bytes(line) for line in rows)
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', SIZE, SIZE, 8, 6, 0, 0, 0))
                     + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


def main():
    with tempfile.TemporaryDirectory() as scratch:
        scratch = Path(scratch)
        subprocess.run(['qlmanage', '-t', '-s', str(SIZE), '-o', str(scratch), str(ROOT / 'assets' / 'icon.svg')],
                       check=True, capture_output=True)
        rows = decode(scratch / 'icon.svg.png')
        mask(rows)
        master = scratch / 'master.png'
        encode(rows, master)
        iconset = scratch / 'AppIcon.iconset'
        iconset.mkdir()
        for size in SIZES:
            for scale in (1, 2):
                name = 'icon_%dx%d%s.png' % (size, size, '@2x' if scale == 2 else '')
                subprocess.run(['sips', '-z', str(size * scale), str(size * scale), str(master), '--out', str(iconset / name)],
                               check=True, capture_output=True)
        subprocess.run(['iconutil', '-c', 'icns', str(iconset), '-o', str(ROOT / 'assets' / 'AppIcon.icns')], check=True)
        (ROOT / 'assets' / 'icon.png').write_bytes((iconset / 'icon_256x256.png').read_bytes())
    print('wrote assets/AppIcon.icns and assets/icon.png')
    return 0


if __name__ == '__main__':
    sys.exit(main())
