#!/usr/bin/env python3
"""Writes core/design/.../Palettes.kt: the PrimerColors of the two GitHub
styles (light, dark, high contrast). Values:

  GitHubMobile   Primer primitives (.docs/android/primer-digest.md section 3.5;
                 fg.default #e6edf3 dark as GitHub Mobile ships it)
  GitHubDesktop  GitHub Desktop 3.6.6's tokens as crates/corvene-ui/src/theme/
                 computes them (ghd_light.rs, ghd_dark.rs, ghd_high_contrast.rs)

The Material style is derived at run time from the M3 scheme (MaterialPalette.kt).
Semantic groups are (fg, emphasis, muted, subtle) = (fgColor, bgColor-emphasis,
borderColor-muted, bgColor-muted). Colours are #rrggbb or #rrggbbaa.

    python3 tools/tokens/gen.py
"""
import pathlib

OUT = pathlib.Path(__file__).resolve().parents[2] / "core/design/src/main/kotlin/com/wasimaster/corvene/design/Palettes.kt"

FIELDS = """isDark
textPrimary textSecondary textTertiary textPlaceholder textLink textOnEmphasis textDisabled
iconPrimary iconSecondary iconLink
bgCanvas bgDefault bgInset bgSubtle bgOverlay bgSelected bgSelectedActive bgHover
borderDefault borderMuted borderEmphasis
accent success attention danger done sponsors
fileNew fileDeleted fileModified fileRenamed fileConflicted
diffAddBg diffAddGutterBg diffAddWordBg diffAddBorder diffDelBg diffDelGutterBg diffDelWordBg diffDelBorder
diffHunkBg diffHunkBorder diffHunkText diffGutterBg diffLineNumber diffText diffAltText diffSelectedBg diffSelectedText diffHoverBg diffEmptyRowBg
syntaxVariable syntaxAltVariable syntaxKeyword syntaxAtom syntaxString syntaxQualifier syntaxType syntaxComment syntaxTag syntaxAttribute syntaxLink syntaxHeader syntaxQuote
toolbarBg toolbarText toolbarTextSecondary toolbarButtonBorder toolbarButtonHoverBg tabBarActive tabBarCountBg badgeBg badgeText coAuthorTagBg coAuthorTagBorder
toastBg toastText scrim""".split()

SEMANTIC = {"accent", "success", "attention", "danger", "done", "sponsors"}

def p(spec):
    """'a b c' -> list; semantic groups as 'fg/emph/muted/subtle'."""
    vals = spec.split()
    assert len(vals) == len(FIELDS), (len(vals), len(FIELDS), vals)
    return dict(zip(FIELDS, vals))

PALETTES = {
"GitHubMobileLight": p("""false
#1f2328 #59636e #818b98 #59636e #0969da #ffffff #818b98
#59636e #818b98 #0969da
#ffffff #ffffff #f6f8fa #f6f8fa #ffffff #818b9826 #0969da #818b981a
#d1d9e0 #d1d9e0b3 #818b98
#0969da/#0969da/#54aeff66/#ddf4ff #1a7f37/#1f883d/#4ac26b66/#dafbe1 #9a6700/#9a6700/#d4a72c66/#fff8c5 #d1242f/#cf222e/#ff818266/#ffebe9 #8250df/#8250df/#c297ff66/#fbefff #bf3989/#bf3989/#ff80c866/#ffeff7
#1a7f37 #d1242f #9a6700 #0969da #bc4c00
#dafbe1 #aceebb #aceebb #4ac26b66 #ffebe9 #ffcecb #ffcecb #ff818266
#ddf4ff #b6e3ff #59636e #ffffff #59636e #1f2328 #59636e #0969da #ffffff #b6e3ff #f6f8fa
#6639ba #1f2328 #cf222e #0550ae #0a3069 #6639ba #cf222e #59636e #0550ae #6639ba #0a3069 #0550ae #116329
#ffffff #1f2328 #59636e #d1d9e0 #818b981a #fd8c73 #818b981f #818b981f #1f2328 #ddf4ff #54aeff66
#25292e #ffffff #00000066"""),
"GitHubMobileDark": p("""true
#e6edf3 #9198a1 #656c76 #9198a1 #4493f8 #ffffff #656c76
#9198a1 #656c76 #4493f8
#0d1117 #0d1117 #010409 #151b23 #151b23 #656c7633 #1f6feb #656c7633
#3d444d #3d444db3 #656c76
#4493f8/#1f6feb/#388bfd66/#388bfd1a #3fb950/#238636/#2ea04366/#2ea04326 #d29922/#9e6a03/#bb800966/#bb800926 #f85149/#da3633/#f8514966/#f851491a #ab7df8/#8957e5/#ab7df866/#ab7df826 #db61a2/#bf4b8a/#db61a266/#db61a21a
#3fb950 #f85149 #d29922 #4493f8 #db6d28
#2ea04326 #3fb9504d #2ea04366 #2ea04366 #f851491a #f851494d #f8514966 #f8514966
#388bfd1a #0c2d6b #9198a1 #0d1117 #9198a1 #e6edf3 #9198a1 #1f6feb #ffffff #388bfd33 #151b23
#d2a8ff #e6edf3 #ff7b72 #79c0ff #a5d6ff #d2a8ff #ff7b72 #9198a1 #7ee787 #d2a8ff #a5d6ff #1f6feb #7ee787
#151b23 #e6edf3 #9198a1 #3d444d #656c7633 #f78166 #656c7633 #656c7633 #e6edf3 #388bfd1a #388bfd66
#ffffff #010409 #00000080"""),
"GitHubMobileLightHighContrast": p("""false
#010409 #454c54 #59636e #454c54 #023b95 #ffffff #59636e
#454c54 #59636e #023b95
#ffffff #ffffff #eff2f5 #e6eaef #ffffff #e0e6eb #0349b4 #e6eaef
#454c54 #454c54 #454c54
#023b95/#0349b4/#368cf9/#dff7ff #04591f/#055d20/#26a148/#d2fedb #603700/#744500/#b58407/#fcf7be #960d1e/#a0111f/#ee5a5d/#fff0ee #5e2bb4/#622cbc/#a371f7/#faf0fe #7d0c57/#7d0c57/#ed4baf/#feeff7
#04591f #960d1e #603700 #023b95 #702c00
#d2fedb #82e596 #82e596 #26a148 #fff0ee #ffc1bc #ffc1bc #ee5a5d
#dff7ff #9cd7ff #454c54 #ffffff #454c54 #010409 #454c54 #0349b4 #ffffff #9cd7ff #e6eaef
#512598 #010409 #a0111f #023b95 #032563 #512598 #a0111f #454c54 #023b95 #512598 #032563 #023b95 #024c1a
#ffffff #010409 #454c54 #454c54 #e6eaef #ba2a00 #e0e6eb #e0e6eb #010409 #dff7ff #368cf9
#25292e #ffffff #00000080"""),
"GitHubMobileDarkHighContrast": p("""true
#ffffff #b7bdc8 #9ea7b3 #b7bdc8 #74b9ff #ffffff #656c76
#b7bdc8 #9ea7b3 #74b9ff
#010409 #010409 #010409 #151b23 #151b23 #3d444d #194fb1 #262c36
#b7bdc8 #b7bdc8 #b7bdc8
#74b9ff/#194fb1/#5cacff/#5cacff1a #2bd853/#006222/#0ac740/#0ac74026 #f0b72f/#7b4900/#edaa27/#edaa2726 #ff9492/#ad0116/#ff8080/#ff80801a #d3abff/#6921d7/#bf8fff/#bf8fff26 #ff90c8/#9c1d6a/#f87cbd/#f87cbd1a
#2bd853 #ff9492 #f0b72f #74b9ff #fe9a2d
#0ac74033 #28d7514d #09b43a #0ac740 #ff808033 #ff80804d #ad0116 #ff8080
#5cacff33 #5cacff66 #b7bdc8 #010409 #b7bdc8 #ffffff #b7bdc8 #194fb1 #ffffff #5cacff66 #151b23
#dbb7ff #ffffff #ff9492 #91cbff #addcff #dbb7ff #ff9492 #bdc4cc #72f088 #dbb7ff #addcff #409eff #72f088
#151b23 #ffffff #b7bdc8 #b7bdc8 #262c36 #ff967d #3d444d #3d444d #ffffff #5cacff1a #5cacff
#ffffff #010409 #000000a6"""),
"GitHubDesktopLight": p("""false
#24292e #6a737d #bbc0c5 #6a737d #0372ef #ffffff #bbc0c5
#24292e #6a737d #0366d6
#ffffff #ffffff #f6f8fa #f6f8fa #ffffff #ebeef1 #0366d6 #f6f8fa
#e1e4e8 #e1e4e8 #879099
#0366d6/#0366d6/#c8e1ff/#f1f8ff #22863a/#28a745/#85e89d/#dcffe4 #b08800/#dbab09/#ffea7f/#fffbdd #cb2431/#d73a49/#fdaeb7/#ffdce0 #6f42c1/#6f42c1/#b392f0/#f5f0ff #ea4aaa/#ea4aaa/#f9b3dd/#ffeef8
#22863a #cb2431 #aa8507 #0366d6 #c24e00
#e6ffed #cdffd8 #acf2bd #85e89d #ffeef0 #ffdce0 #fdb8c0 #fdaeb7
#f1f8ff #c8e1ff #586069 #ffffff #444d56 #24292e #444d56 #2188ff #ffffff #79b8ff #fafbfc
#6f42c1 #24292e #d73a49 #005cc5 #032f62 #6f42c1 #d73a49 #6a737d #22863a #6f42c1 #032f62 #0000ff #1a7e31
#24292e #ffffff #d1d5da #000000 #2f363d #0366d6 #e1e4e8 #e1e4e8 #2f363d #f1f8ff #c8e1ff
#24292e #f6f8fa #00000066"""),
"GitHubDesktopDark": p("""true
#f6f8fa #959da5 #535a61 #959da5 #2e8fff #ffffff #535a61
#f6f8fa #959da5 #2e8fff
#24292e #1d2125 #2b3137 #2b3137 #24292e #444d56 #0366d6 #2f363d
#141414 #141414 #717b85
#2e8fff/#0366d6/#044289/#05264c #28a745/#28a745/#165c26/#144620 #f9c513/#dbab09/#735c0f/#272216 #d73a49/#d73a49/#9e1c23/#79161a #b392f0/#6f42c1/#5a32a3/#2a1f3d #db61a2/#ea4aaa/#99306f/#3a1a2d
#28a745 #d73a49 #dbab09 #0366d6 #f66a0a
#113a1b #0b2611 #22863a #123e1c #450c0f #2f080a #b31d28 #5b1014
#1d2125 #2b3137 #959da5 #1d2125 #959da5 #f6f8fa #b4c5d6 #044289 #f6f8fa #0366d6 #22262b
#b392f0 #79b8ff #f97583 #79b8ff #ffab70 #b392f0 #f97583 #959da5 #34d058 #b392f0 #79b8ff #f97583 #34d058
#1d2125 #f6f8fa #959da5 #141414 #2f363d #0366d6 #444d56 #586069 #f6f8fa #032f62 #044289
#2f363d #f6f8fa #00000080"""),
"GitHubDesktopHighContrast": p("""true
#f0f3f6 #d9dee3 #9ea7b3 #9ea7b3 #71b7ff #0a0c10 #9ea7b3
#f0f3f6 #d9dee3 #71b7ff
#0a0c10 #0a0c10 #272b33 #272b33 #0a0c10 #525964 #409eff #272b33
#7a828e #7a828e #7a828e
#71b7ff/#409eff/#409eff66/#409eff26 #26cd4d/#09b43a/#26cd4d4d/#09b43a26 #f0b72f/#f0b72f/#f0b72f66/#f0b72f26 #ff6a69/#ff6a69/#ff6a694d/#ff6a691a #dbb7ff/#dbb7ff/#dbb7ff66/#dbb7ff26 #ff90c8/#ff90c8/#ff90c866/#ff90c826
#26cd4d #ff6a69 #f0b72f #71b7ff #e7811d
#09b43a26 #26cd4d4d #09b43a80 #26cd4d4d #ff6a691a #ff6a694d #ff6a6973 #ff6a694d
#409eff1a #409eff66 #f0f3f6 #0a0c10 #f0f3f6 #f0f3f6 #d9dee3 #409eff #0a0c10 #409eff66 #272b33
#ffb757 #ffb757 #ff9492 #91cbff #addcff #dbb7ff #dbb7ff #bdc4cc #72f088 #91cbff #addcff #409eff #72f088
#010409 #f0f3f6 #d9dee3 #7a828e #272b33 #409eff #525964 #525964 #f0f3f6 #272b33 #7a828e
#f0f3f6 #0a0c10 #00000080"""),
}

def color(v):
    v = v.lstrip("#")
    if len(v) == 6:
        v = v + "ff"
    rgb, a = v[:6], v[6:]
    return f"Color(0x{a.upper()}{rgb.upper()})"

def render(name, pal):
    lines = [f"internal val {name} = PrimerColors("]
    for f in FIELDS:
        v = pal[f]
        if f == "isDark":
            expr = v
        elif f in SEMANTIC:
            fg, em, mu, su = v.split("/")
            expr = f"SemanticColor({color(fg)}, {color(em)}, {color(mu)}, {color(su)})"
        else:
            expr = color(v)
        lines.append(f"    {f} = {expr},")
    lines.append(")")
    return "\n".join(lines)

body = "\n\n".join(render(n, p) for n, p in PALETTES.items())
OUT.write_text(f"""// Generated by tools/tokens/gen.py; edit the values there.
package com.wasimaster.corvene.design

import androidx.compose.ui.graphics.Color

{body}
""")
print(OUT)
