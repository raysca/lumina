# Third-Party Notices

This repository bundles the following third-party assets. Lumina itself is
released under the MIT License; see [LICENSE](LICENSE). The assets below remain
under their respective licenses.

## Fonts — Inter, Inter Tight, JetBrains Mono

Unmodified font files in `resources/fonts/`, compiled into the app. Each is
licensed under the SIL Open Font License, Version 1.1; the full licence files
sit beside them (`Inter-LICENSE.txt`, `InterTight-OFL.txt`,
`JetBrainsMono-OFL.txt`) and the licence text is reproduced below.

| Files | Version | Copyright |
|---|---|---|
| `Inter-Regular.otf`, `Inter-Medium.otf`, `Inter-SemiBold.otf` | Inter 4.1 | Copyright (c) 2016 The Inter Project Authors (https://github.com/rsms/inter) |
| `InterTight-SemiBold.ttf` | Inter Tight, `googlefonts/inter-gf-tight` @ c194f94 | Copyright 2022 The Inter Project Authors (https://github.com/rsms/inter-tight) |
| `JetBrainsMono-Regular.ttf` | JetBrains Mono 2.304 | Copyright 2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono) |

```text
This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
https://scripts.sil.org/OFL


-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded, 
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

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

## Map — Natural Earth

`resources/map/land_110m.bin`, the offline map's land outline, is the
1:110m land polygons (`ne_110m_land`) of [Natural Earth](https://www.naturalearthdata.com)
release v5.1.2, converted without simplification to a compact binary by
`scripts/build_world_basemap.py`, which checks the source file's pinned
SHA-256. A maintainer runs the script by hand; the build and the app never
download it, and the file is compiled into the app.

Natural Earth is in the public domain; no permission is needed to use it.
See <https://www.naturalearthdata.com/about/terms-of-use/>.

## Bundled helper — yt-dlp

The release app bundle packages the standalone universal macOS build of
[yt-dlp](https://github.com/yt-dlp/yt-dlp) (`Contents/MacOS/yt-dlp`, version
and SHA-256 pinned in `scripts/fetch_yt_dlp.sh`) for video import. yt-dlp is
dedicated to the public domain under The Unlicense (<https://unlicense.org>).
That executable is a PyInstaller build: it embeds a CPython runtime (Python
Software Foundation License) and third-party libraries and Python packages,
some under copyleft licenses (for example GPL-2.0-or-later mutagen). Their
license texts, yt-dlp's `THIRD_PARTY_LICENSES.txt` at the pinned release tag
(SHA-256 pinned in `scripts/fetch_yt_dlp.sh`), ship as
`Contents/Resources/yt-dlp-THIRD_PARTY_LICENSES.txt`. That file also says
where to get their source. Lumina only runs `yt-dlp` as a separate process.

## Bundled helper — FFmpeg

The release app bundle also packages [FFmpeg](https://ffmpeg.org) 9.0.2
(`Contents/MacOS/ffmpeg`), built unmodified from
<https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz> (SHA-256 pinned in
`scripts/fetch_ffmpeg.sh`). It is licensed under the GNU Lesser General
Public License version 2.1 or later; no GPL or nonfree components are
enabled. The license text ships as `Contents/Resources/ffmpeg-LICENSE.txt`.
The complete corresponding source, the unmodified tarball and the build
script with the exact configure flags, ships in
`Contents/Resources/ffmpeg-source/`. It is configured with:

```text
--disable-everything --disable-autodetect --disable-network --disable-doc
--disable-debug --disable-ffplay --disable-ffprobe --disable-x86asm
--enable-zlib --enable-protocol=file,pipe
--enable-demuxer=mov,matroska,mpegts,aac,mp3,h264,hevc
--enable-muxer=mp4,mov,ipod
--enable-parser=h264,hevc,aac,av1,vp9,opus,mpegaudio,ac3
--enable-decoder=aac,h264,hevc,vp9
--enable-bsf=aac_adtstoasc,extract_extradata,vp9_superframe,null
```

Lumina only runs the `ffmpeg` executable as a separate process.

## Rust crates

The UI framework is GPUI Kit (`gpui-kit`, `gpui-base`) and the GPUI snapshot
it pins (`gpui-pre-*`), all Apache-2.0. Its optional component and icon-asset
features are off, so no gpui-component code or Lucide icons are built in. The full dependency
graph and versions are pinned in `Cargo.lock`. License
texts for each crate are available from its publishing source (crates.io) and
should be reviewed before redistribution.
