# Third-party notices

Bili DM uses librqbit 9.0.1 (Apache-2.0), Tauri, React, SQLite, and dynamically loaded libmpv. Application source is independently implemented; Frame Player application source and UI are not vendored.

The macOS arm64 native runtime is the LGPL library set built by [Frame Player](https://github.com/risenxxx/frame-player/tree/e9767259d5f5ca25a5b2a5951b1ebc80c702ea73), archive macos-arm64-9cb06b14559c, SHA-256 4d5b1eb5ea390d1dadd388fba88aa9a7ca41177188ba367b03ab67d75db5b796. mpv 0.41.0 includes its macOS NSView embedding patch and upstream CoreAudio initialization fix. The build scripts, version pins, configure flags and patches are available in that exact source revision. FFmpeg is built with GPL and nonfree components disabled, and mpv with gpl=false.

The libraries are replaceable shared libraries in Contents/Resources/lib. Their license and copyright texts accompany this app under licenses/native. Replacement builds of mpv must implement the macOS NSView wid extension or use a corresponding render adapter. The unused libmpv-wrapper from the archive is not shipped.

Components: mpv and FFmpeg (LGPL-2.1-or-later), libplacebo, GLib, FriBidi, Graphite2 and gettext runtime (LGPL); FreeType (FTL), libass (ISC), HarfBuzz/Little CMS/LuaJIT/X11 libraries (MIT), libpng (libpng), uchardet (MPL-1.1), dav1d (BSD-2-Clause), libunibreak (Zlib), PCRE2 (BSD-3-Clause), libjpeg-turbo (IJG/BSD/Zlib), MoltenVK/Vulkan loader/shaderc (Apache-2.0). GLib uses the included LGPL-2.1 text.

Frame Player itself is GPL-3.0-or-later; its library components retain their individual licenses. This notice describes the runtime used here, not a change to Frame Player's application license.

## Native runtime inventory

These source links identify the projects used in the pinned archive. The exact mpv/FFmpeg pins, macOS patches and dependency packaging are recorded by the [upstream build recipe](https://github.com/risenxxx/frame-player/blob/e9767259d5f5ca25a5b2a5951b1ebc80c702ea73/scripts/build-macos-libs.sh).

| Component | License | Source | License text |
| --- | --- | --- | --- |
| MoltenVK | Apache-2.0 | [Source](https://github.com/KhronosGroup/MoltenVK) | [Text](licenses/native/molten-vk.txt) |
| libX11 | MIT | [Source](https://gitlab.freedesktop.org/xorg/lib/libx11) | [Text](licenses/native/libx11.txt) |
| libXau | MIT | [Source](https://gitlab.freedesktop.org/xorg/lib/libxau) | [Text](licenses/native/libxau.txt) |
| libXdmcp | MIT | [Source](https://gitlab.freedesktop.org/xorg/lib/libxdmcp) | [Text](licenses/native/libxdmcp.txt) |
| libass | ISC | [Source](https://github.com/libass/libass) | [Text](licenses/native/libass.txt) |
| FFmpeg | LGPL-2.1-or-later | [Source](https://github.com/FFmpeg/FFmpeg) | [Text](licenses/native/ffmpeg.txt) |
| dav1d | BSD-2-Clause | [Source](https://code.videolan.org/videolan/dav1d) | [Text](licenses/native/dav1d.txt) |
| FreeType | FTL | [Source](https://gitlab.freedesktop.org/freetype/freetype) | [Text](licenses/native/freetype.txt) |
| GNU FriBidi | LGPL-2.1-or-later | [Source](https://github.com/fribidi/fribidi) | [Text](licenses/native/fribidi.txt) |
| GLib | LGPL-2.1-or-later | [Source](https://gitlab.gnome.org/GNOME/glib) | [Text](licenses/native/LGPL-2.1.txt) |
| Graphite2 | LGPL-2.1-or-later | [Source](https://github.com/silnrsi/graphite) | [Text](licenses/native/graphite2.txt) |
| HarfBuzz | MIT | [Source](https://github.com/harfbuzz/harfbuzz) | [Text](licenses/native/harfbuzz.txt) |
| GNU gettext runtime (libintl) | LGPL-2.1-or-later | [Source](https://git.savannah.gnu.org/cgit/gettext.git) | [Text](licenses/native/gettext.txt) |
| libjpeg-turbo | IJG AND Zlib AND BSD-3-Clause | [Source](https://github.com/libjpeg-turbo/libjpeg-turbo) | [Text](licenses/native/jpeg-turbo.txt) |
| Little CMS | MIT | [Source](https://github.com/mm2/Little-CMS) | [Text](licenses/native/lcms2.txt) |
| LuaJIT | MIT | [Source](https://github.com/LuaJIT/LuaJIT) | [Text](licenses/native/luajit.txt) |
| mpv | LGPL-2.1-or-later | [Source](https://github.com/mpv-player/mpv) | [Text](licenses/native/mpv.txt) |
| PCRE2 | BSD-3-Clause | [Source](https://github.com/PCRE2Project/pcre2) | [Text](licenses/native/pcre2.txt) |
| libplacebo | LGPL-2.1-or-later | [Source](https://code.videolan.org/videolan/libplacebo) | [Text](licenses/native/libplacebo.txt) |
| libpng | libpng-2.0 | [Source](https://github.com/pnggroup/libpng) | [Text](licenses/native/libpng.txt) |
| shaderc | Apache-2.0 | [Source](https://github.com/google/shaderc) | [Text](licenses/native/shaderc.txt) |
| uchardet | MPL-1.1 | [Source](https://gitlab.freedesktop.org/uchardet/uchardet) | [Text](licenses/native/uchardet.txt) |
| libunibreak | Zlib | [Source](https://github.com/adah1972/libunibreak) | [Text](licenses/native/libunibreak.txt) |
| Vulkan-Loader | Apache-2.0 | [Source](https://github.com/KhronosGroup/Vulkan-Loader) | [Text](licenses/native/vulkan-loader.txt) |
| libxcb | MIT | [Source](https://gitlab.freedesktop.org/xorg/lib/libxcb) | [Text](licenses/native/libxcb.txt) |
