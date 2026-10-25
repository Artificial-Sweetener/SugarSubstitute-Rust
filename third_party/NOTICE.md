# Bundled material

The installer uses provisioning source from
[Comfy Desktop](https://github.com/Comfy-Org/Comfy-Desktop), pinned by the
`third_party/comfy-desktop` Git submodule. The build applies the reviewed patches
in `installer/patches` to an isolated source snapshot. This integration uses
the upstream AGPL-3.0-or-later license option. Its source inventory and license
text are recorded in `third_party/manifest.toml`.

The installer wordmark is preserved from SugarSubstitute's
`docs/readme/sugarsubstitute-logo.svg` under GPL-3.0-or-later.

The original SugarSubstitute splash character images are preserved from
[SugarSubstitute](https://github.com/Artificial-Sweetener/SugarSubstitute).
They include the numbered mascot poses and the named guest images Cass, Comfy,
Comfy2, Cubby, Diffusion, Liz, LVGF, Married, Pixel and Library, Ren, Syl and Witchy.
The source distribution carries GPL-3.0-or-later. The original files are bundled
unchanged; their license text and exact inventory are recorded in the manifest.

The console glyph comes from Microsoft Fluent UI System Icons (MIT). Its glyph is
bundled as white alpha PNGs at nine device scales.
The close glyph comes from PyQt-Frameless-Window (MIT). The close glyph is bundled at the same scales;
the Rust executable includes no Qt runtime. Both license texts are bundled.

The splash raster contains Rust adaptations of numerical rules from
[Qt Base 6.11.0](https://github.com/qt/qtbase/tree/v6.11.0): fixed-point cosmetic
stroke integration, cubic outline mapping, pixel coverage, gradient lookup,
premultiplied byte composition and affine image sampling. The source references
are `qcosmeticstroker.cpp`, `qbezier.cpp`, `qoutlinemapper.cpp`, `qgrayraster.c`,
`qdrawhelper.cpp`, `qdrawhelper_p.h`, `qpaintengine_raster.cpp`, `qtransform.cpp`
and `qcssutil.cpp`. Windows status rendering applies the native grayscale glyph
coverage and premultiplied byte rounding rules through DirectWrite.
The adapted runtime files are listed in the manifest. Their Rust implementation
is maintained with the splash owner and its tests.

Qt portions are copyright The Qt Company Ltd. (2016â€“2022) and Intel Corporation
(2018), distributed here under GPL-3.0-only. The FreeType-derived coverage rules
are copyright David Turner, Robert Wilhelm and Werner Lemberg (2000â€“2016),
distributed here under the FreeType Project License (FTL). The combined license
file contains both license texts. Modifications are copyright Artificial
Sweetener and contributors (2026). The application uses these raster rules in
its Rust renderer. This software is based in part on the work of the FreeType
Team. The original FreeType license is also preserved as `licenses/FTL.TXT`.


Crashpad registration and source-build adaptations derive from Crashpad commit
60dd943f48d77dc8d05dabc04badbd8561d0b8c4 (Apache-2.0) and mini-chromium commit
e5169551c51f3a52eee36b3b03f219cefe380237 (BSD-3-Clause). The manifest identifies
the exact maintained derivatives and preserved upstream license texts. Changes
are copyright Artificial Sweetener and contributors (2026), GPL-3.0-or-later.
The native capture bundle uses Crashpad, mini-chromium, ICU, zlib and getopt;
its source-build receipt records their pinned sources and runtime license texts.
The Crashpad client and registration portions retain Copyright 2015 The Crashpad
Authors. The mini-chromium Windows build-helper portion retains Copyright 2017
The Chromium Authors.


Wry 0.57.0 supplies the native browser adapter under Apache-2.0 or MIT.
Maintained Windows derivatives add staged UI-affine construction and checked
retirement. Those changes are copyright Artificial Sweetener and contributors
(2026), GPL-3.0-or-later. The manifest identifies the exact packaged files and
preserved upstream license texts.

The bounded finished-report parser implements the Windows metadata version-one
and minidump Crashpad-info version-one layouts from Crashpad commit
60dd943f48d77dc8d05dabc04badbd8561d0b8c4 (Copyright The Crashpad Authors,
Apache-2.0). The manifest records its exact source and test files. These maintained
parsers use fixed storage, file-relative bounded reads, and complete module
identity dictionaries. Adaptations are copyright Artificial Sweetener and
contributors (2026), GPL-3.0-or-later. The upstream license text is preserved in
`licenses/crashpad.Apache-2.0.txt`.

The native capture-attachment adapter and its deterministic boundary tests
derive from the same pinned Crashpad and mini-chromium sources. The native
client exposes separately named version-one capture/source operations and
version-two attachment operations. Maintained adaptations and test extraction
are copyright Artificial Sweetener and contributors (2026), GPL-3.0-or-later.

The installer uses the original Fluent icon SVGs from PyQt-Fluent-Widgets,
Copyright zhiyiYo and contributors, under GPL-3.0. The manifest pins their source
revision and lists each embedded asset; the license is preserved in
`licenses/qfluentwidgets.GPL-3.0.txt`.

The provisioning worker uses Node.js 24.22.1, Copyright Node.js contributors.
The runtime pin identifies each official archive and its SHA-256 digest. The
complete upstream license and bundled dependency notices are preserved in
`licenses/nodejs-24.22.1.txt` and travel with the runtime as `worker/runtime/LICENSE`.
