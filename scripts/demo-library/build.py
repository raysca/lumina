#!/usr/bin/env python3
"""Build Lumina's demo library from public-domain (CC0) photos.

Downloads the originals listed in photos.json from Wikimedia Commons (camera
EXIF and GPS intact, so the inspector, search and map have real data),
checks each file's SHA-1, sorts them into trip folders and writes an XMP
sidecar beside each one with a title, keywords, a star rating, a colour label
and the photographer's credit. The sidecars use the same layout Lumina writes,
so Lumina treats them as its own.

    python3 scripts/demo-library/build.py [OUTPUT_DIR]

The default output is ~/Pictures/Lumina Demo Library. Re-running skips files
that are already present and correct. Then, in Lumina: Open Existing Library…
on that folder, select all (⌘A) and run "Import XMP Metadata" from the
command palette (⌘K) to apply the ratings, labels and keywords.

This library exists for screenshots, videos and demos. Never use personal
photos for marketing material.
"""

import hashlib
import json
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
USER_AGENT = "LuminaDemoLibrary/1.0 (https://github.com/raysca/lumina)"
LABELS = {"Red", "Orange", "Yellow", "Green", "Blue", "Purple"}


def commons_url(name: str) -> str:
    """The original file's address on upload.wikimedia.org."""
    name = name.replace(" ", "_")
    digest = hashlib.md5(name.encode("utf-8")).hexdigest()
    return "https://upload.wikimedia.org/wikipedia/commons/{}/{}/{}".format(
        digest[0], digest[:2], urllib.parse.quote(name)
    )


def page_url(name: str) -> str:
    return "https://commons.wikimedia.org/wiki/File:" + urllib.parse.quote(name.replace(" ", "_"))


def sha1_of(path: Path) -> str:
    h = hashlib.sha1()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url: str, dest: Path, attempts: int = 4) -> None:
    tmp = dest.with_name(dest.name + ".part")
    for attempt in range(1, attempts + 1):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=120) as resp, tmp.open("wb") as out:
                while chunk := resp.read(1 << 20):
                    out.write(chunk)
            tmp.replace(dest)
            return
        except Exception as err:  # network errors and HTTP 429/5xx
            if attempt == attempts:
                tmp.unlink(missing_ok=True)
                raise
            wait = 5 * attempt
            print(f"    retrying in {wait}s ({err})")
            time.sleep(wait)


def esc(value: str) -> str:
    return (
        value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")
    )


def alt(name: str, value: str) -> str:
    return (
        f"   <{name}>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{esc(value)}</rdf:li>\n"
        f"    </rdf:Alt>\n   </{name}>\n"
    )


def bag(name: str, values: list) -> str:
    if not values:
        return ""
    items = "".join(f"     <rdf:li>{esc(v)}</rdf:li>\n" for v in values)
    return f"   <{name}>\n    <rdf:Bag>\n{items}    </rdf:Bag>\n   </{name}>\n"


def sidecar(photo: dict) -> str:
    """Mirrors crates/lumina-core/src/xmp/render.rs."""
    segments, hierarchy = [], []
    for tag in photo["tags"]:
        hierarchy.append(tag.replace("/", "|"))
        for seg in filter(None, tag.split("/")):
            if seg not in segments:
                segments.append(seg)
    label = photo.get("label") or ""
    if label and label not in LABELS:
        raise ValueError(f"unknown label {label!r} for {photo['file']}")
    author = photo["author"]
    head = (
        "<?xpacket begin=\"﻿\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n"
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n"
        " <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n"
        "  <rdf:Description rdf:about=\"\"\n"
        "    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n"
        "    xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\"\n"
        "    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n"
        + ("    xmlns:lr=\"http://ns.adobe.com/lightroom/1.0/\"\n" if hierarchy else "")
        + "    xmlns:xmpRights=\"http://ns.adobe.com/xap/1.0/rights/\"\n"
        + f"    xmp:Rating=\"{int(photo['rating'])}\"\n"
        + (f"    xmp:Label=\"{label}\"\n" if label else "")
        + "  >\n"
    )
    body = (
        alt("dc:title", photo["title"])
        + bag("dc:subject", segments)
        + bag("lr:hierarchicalSubject", hierarchy)
        + f"   <dc:source>{esc(page_url(photo['file']))}</dc:source>\n"
        + f"   <photoshop:Credit>{esc(author)} / Wikimedia Commons</photoshop:Credit>\n"
        + f"   <dc:creator>\n    <rdf:Seq>\n     <rdf:li>{esc(author)}</rdf:li>\n    </rdf:Seq>\n   </dc:creator>\n"
        + alt("dc:rights", f"{author}, CC0 1.0 (public domain dedication)")
        + alt("xmpRights:UsageTerms", "CC0 1.0 Universal: https://creativecommons.org/publicdomain/zero/1.0/")
    )
    return head + body + "  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>\n"


def main() -> int:
    out = Path(sys.argv[1]).expanduser() if len(sys.argv) > 1 else Path.home() / "Pictures" / "Lumina Demo Library"
    manifest = json.loads((HERE / "photos.json").read_text(encoding="utf-8"))
    photos = manifest["photos"]
    total = sum(p["bytes"] for p in photos)
    print(f"Building {len(photos)} photos ({total / 1e6:.0f} MB) into {out}")
    out.mkdir(parents=True, exist_ok=True)

    failed = []
    for i, photo in enumerate(photos, 1):
        folder = out / photo["folder"]
        folder.mkdir(exist_ok=True)
        dest = folder / photo["file"]
        if dest.exists() and sha1_of(dest) == photo["sha1"]:
            print(f"[{i}/{len(photos)}] ok   {photo['file']}")
        else:
            print(f"[{i}/{len(photos)}] get  {photo['file']}")
            try:
                download(commons_url(photo["file"]), dest)
            except Exception as err:
                print(f"    FAILED: {err}")
                failed.append(photo["file"])
                continue
            if sha1_of(dest) != photo["sha1"]:
                print("    FAILED: checksum mismatch (the file on Commons changed)")
                dest.unlink()
                failed.append(photo["file"])
                continue
            time.sleep(0.5)  # be gentle with Commons
        Path(str(dest) + ".xmp").write_text(sidecar(photo), encoding="utf-8")

    credits = ["# Credits", "", "Every photo here is dedicated to the public domain under CC0 1.0", "by its photographer and was downloaded from Wikimedia Commons.", ""]
    for photo in photos:
        credits.append(f"- {photo['folder']}/{photo['file']}: {photo['author']}, {page_url(photo['file'])}")
    (out / "CREDITS.md").write_text("\n".join(credits) + "\n", encoding="utf-8")

    if failed:
        print(f"\n{len(failed)} photo(s) failed; run again to retry:")
        for name in failed:
            print("  " + name)
        return 1
    print(f"\nDone. Open {out} in Lumina, select all and run Import XMP Metadata (⌘K).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
