# Demo library

A ready-made library for screenshots, videos and demos, so marketing material
never uses anyone's personal photos.

`photos.json` lists 66 photos (about 660 MB) from Wikimedia Commons, all
**Quality Images dedicated to the public domain under CC0 1.0**, with their
original camera EXIF and GPS. They are grouped into trips (Greece, Québec,
Mongolia, Korea, Japan, Andalusia, the Alps, Sweden, Iceland and more) so the
timeline, Places map, search and smart albums all have something to show.
A few short series (camels in the Gobi, Ongryongam Temple, White Lake, the
Cretan coast) give Similar Shots and Compare something to work with.

## Build it

```bash
python3 scripts/demo-library/build.py                # ~/Pictures/Lumina Demo Library
python3 scripts/demo-library/build.py /path/to/dir   # somewhere else
```

The script downloads each original, checks its SHA-1, and writes an XMP
sidecar beside it with a title, hierarchical keywords (`places/asia/mongolia/gobi`),
a star rating, a colour label and the photographer's credit. The sidecars use
Lumina's own layout, so Lumina reads them and keeps them up to date. Re-running
skips files that are already there. A `CREDITS.md` in the output lists every
photographer and source page.

## Load it in Lumina

1. Open Existing Library… and pick the output folder.
2. Select everything (⌘A) and run **Import XMP Metadata** from the command
   palette (⌘K) to apply the ratings, labels and keywords.

Pick and reject flags are not stored in XMP, so mark a few picks by hand
(`P`, `X`) before capturing culling shots.

## Changing the set

Add or remove entries in `photos.json`. Each needs the exact Commons file
name, its SHA-1 and size (from the Commons API's `imageinfo`), the author,
and the folder, title, keywords, rating (0 to 5) and label (`Red`, `Orange`,
`Yellow`, `Green`, `Blue`, `Purple` or empty) to seed. Only add CC0 or
public-domain photos, and avoid photos where a person is the subject.
