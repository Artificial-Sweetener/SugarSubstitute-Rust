# Bundled splash material

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

Qt portions are copyright The Qt Company Ltd. (2016–2022) and Intel Corporation
(2018), distributed here under GPL-3.0-only. The FreeType-derived coverage rules
are copyright David Turner, Robert Wilhelm and Werner Lemberg (2000–2016),
distributed here under the FreeType Project License (FTL). The combined license
file contains both license texts. Modifications are copyright Artificial
Sweetener and contributors (2026). The application uses these raster rules in
its Rust renderer. This software is based in part on the work of the FreeType
Team. The original FreeType license is also preserved as `licenses/FTL.TXT`.
