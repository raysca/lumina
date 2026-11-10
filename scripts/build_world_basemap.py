#!/usr/bin/env python3
"""Builds resources/map/land_110m.bin, the offline map's land outline.

Maintainer-run only; the build and the app never run it or touch the
network. Download the pinned Natural Earth file yourself, then:

    curl -fsSLo /tmp/ne_110m_land.geojson \\
      https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_110m_land.geojson
    python3 -I scripts/build_world_basemap.py /tmp/ne_110m_land.geojson

Natural Earth is public domain (https://www.naturalearthdata.com/about/terms-of-use/).
Rings are copied without simplifying. Output, little-endian:
u32 ring count, then per ring u32 point count and (f32 lon, f32 lat) pairs.
The point count's top bit marks a hole (water inside land, the Caspian Sea at
this scale); the map draws holes in the water colour over the land.
"""

import hashlib
import json
import pathlib
import struct
import sys

RELEASE = "v5.1.2"
SHA256 = "9e0729ee253ca7d7a5c4ae9395fb1902264c5377c52e224d13dd85010e2835d9"
OUT = pathlib.Path(__file__).resolve().parent.parent / "resources" / "map" / "land_110m.bin"


HOLE = 0x8000_0000


def rings(geometry):
    """(ring, is_hole): GeoJSON puts a polygon's outer ring first."""
    if geometry["type"] == "Polygon":
        polygons = [geometry["coordinates"]]
    elif geometry["type"] == "MultiPolygon":
        polygons = geometry["coordinates"]
    else:
        raise SystemExit(f"unexpected geometry {geometry['type']}")
    for polygon in polygons:
        for index, ring in enumerate(polygon):
            yield ring, index > 0


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: build_world_basemap.py <ne_110m_land.geojson>")
    data = pathlib.Path(sys.argv[1]).read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != SHA256:
        raise SystemExit(f"SHA-256 {digest} is not the pinned Natural Earth {RELEASE} file")
    features = json.loads(data)["features"]
    out = bytearray()
    count = 0
    for feature in features:
        for ring, is_hole in rings(feature["geometry"]):
            for lon, lat in ring:
                if not (-180.0 <= lon <= 180.0 and -90.0 <= lat <= 90.0):
                    raise SystemExit(f"coordinate out of range: {lon}, {lat}")
            out += struct.pack("<I", len(ring) | (HOLE if is_hole else 0))
            for lon, lat in ring:
                out += struct.pack("<ff", lon, lat)
            count += 1
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(struct.pack("<I", count) + out)
    print(f"wrote {OUT} ({count} rings, {OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
