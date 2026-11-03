# Third-Party Notices

This repository bundles the following third-party assets. Lumina itself is
released under the MIT License; see [LICENSE](LICENSE). The assets below remain
under their respective licenses.

## Fonts — IBM Plex

`resources/fonts/IBMPlexSans-*.otf` and `resources/fonts/IBMPlexMono-Regular.otf`

Copyright © IBM Corp. Licensed under the SIL Open Font License, Version 1.1.
<https://scripts.sil.org/OFL>

## Icons — Lucide

Icons are unmodified SVG files from [Lucide](https://lucide.dev)
(`lucide-static` 0.460.0), vendored in `resources/icons/` and compiled into the
app. The transport icons and the filled heart/star are drawn solid at load time.

```text
ISC License

Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022 as part of Feather (MIT). All other copyright (c) for Lucide are held by Lucide Contributors 2022.

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

## Bundled helper — yt-dlp

The release app bundle may package a standalone universal build of [yt-dlp](https://github.com/yt-dlp/yt-dlp) for video import. yt-dlp is dedicated to the public domain under The Unlicense (<https://unlicense.org>).

## Rust crates

The full dependency graph and versions are pinned in `Cargo.lock`. License
texts for each crate are available from its publishing source (crates.io) and
should be reviewed before redistribution.
