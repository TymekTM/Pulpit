"""Generate every Pulpit logo asset from one geometry (256-unit grid)."""
import os, sys
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

ROOT = sys.argv[1]
ACCENT, SPARE, BG, INK = "#1abc9c", "#2c353e", "#171c21", "#ecf0f1"

# keys: stem (left, full height), bowl (top right, round end + open counter), spare (bottom right)
X0, Y0, X1, Y1 = 52, 48, 204, 208
SW, GAP, R_KEY = 58, 14, 14
BX = X0 + SW + GAP            # 124
BH = 96
R = BH / 2                    # 48
HR, HY, HX1 = 17, Y0 + BH / 2, X1 - R

def n(v):
    return f"{v:g}"

STEM = f"M{n(X0+R_KEY)} {n(Y0)}H{n(X0+SW-R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(X0+SW)} {n(Y0+R_KEY)}V{n(Y1-R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(X0+SW-R_KEY)} {n(Y1)}H{n(X0+R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(X0)} {n(Y1-R_KEY)}V{n(Y0+R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(X0+R_KEY)} {n(Y0)}Z"
BOWL = (f"M{n(BX+R_KEY)} {n(Y0)}H{n(X1-R)}A{n(R)} {n(R)} 0 0 1 {n(X1-R)} {n(Y0+BH)}H{n(BX+R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(BX)} {n(Y0+BH-R_KEY)}V{n(Y0+R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(BX+R_KEY)} {n(Y0)}Z"
        f"M{n(BX)} {n(HY-HR)}H{n(HX1)}A{HR} {HR} 0 0 1 {n(HX1)} {n(HY+HR)}H{n(BX)}Z")
SY = Y0 + BH + GAP
SPARE_D = f"M{n(BX+R_KEY)} {n(SY)}H{n(X1-R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(X1)} {n(SY+R_KEY)}V{n(Y1-R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(X1-R_KEY)} {n(Y1)}H{n(BX+R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(BX)} {n(Y1-R_KEY)}V{n(SY+R_KEY)}A{R_KEY} {R_KEY} 0 0 1 {n(BX+R_KEY)} {n(SY)}Z"
TILE = "M64 8H192A56 56 0 0 1 248 64V192A56 56 0 0 1 192 248H64A56 56 0 0 1 8 192V64A56 56 0 0 1 64 8Z"

def glyph(accent=ACCENT, spare=SPARE, spare_op=None):
    op = f' fill-opacity="{spare_op}"' if spare_op else ""
    return (f'<path fill="{accent}" d="{STEM}"/>'
            f'<path fill="{accent}" fill-rule="evenodd" d="{BOWL}"/>'
            f'<path fill="{spare}"{op} d="{SPARE_D}"/>')

def svg(body, vb="0 0 256 256", w=None, h=None):
    size = f' width="{w}" height="{h}"' if w else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}"{size}>{body}</svg>\n'

def write(rel, text):
    p = os.path.join(ROOT, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "w").write(text)

APP_ICON = svg(f'<path fill="{BG}" d="{TILE}"/>' + glyph())
write("docs/brand/pulpit-icon.svg", APP_ICON)
write("docs/brand/pulpit-glyph.svg", svg(glyph(), "52 48 152 160"))
write("docs/brand/pulpit-glyph-mono.svg", svg(glyph("#000", "#000", 0.35), "52 48 152 160"))
write("apps/desktop/public/favicon.svg", APP_ICON)

# wordmark: Inter SemiBold outlines, so the lockup needs no font installed
font = TTFont(os.path.join(ROOT, "apps/mobile/app/src/main/res/font/inter_semibold.ttf"))
gs, cmap = font.getGlyphSet(), font.getBestCmap()
upm = font["head"].unitsPerEm
def word(text, x, baseline, size, tracking=-0.01):
    s = size / upm
    pen = SVGPathPen(gs)
    for ch in text:
        g = cmap[ord(ch)]
        gs[g].draw(TransformPen(pen, (s, 0, 0, -s, x, baseline)))
        x += (gs[g].width + tracking * upm) * s
    return pen.getCommands(), x
def lockup(ink):
    d, end = word("Pulpit", 300, 168, 128)
    return svg(f'<path fill="{BG}" d="{TILE}"/>' + glyph() + f'<path fill="{ink}" d="{d}"/>', f"0 0 {round(end)+16} 256")
write("docs/brand/pulpit-lockup-dark.svg", lockup(INK))     # for dark backgrounds
write("docs/brand/pulpit-lockup-light.svg", lockup("#171c21"))  # for light backgrounds

# Android: VectorDrawable paths take SVG path syntax as-is
def vpath(color, d, even=False, alpha=None):
    extra = ' android:fillType="evenOdd"' if even else ""
    extra += f' android:fillAlpha="{alpha}"' if alpha else ""
    return f'        <path android:fillColor="{color}"{extra}\n            android:pathData="{d}"/>\n'
def vglyph(accent=ACCENT, spare=SPARE, spare_alpha=None):
    return vpath(accent, STEM) + vpath(accent, BOWL, True) + vpath(spare, SPARE_D, alpha=spare_alpha)
HDR = '<?xml version="1.0" encoding="utf-8"?>\n<!-- Generated from docs/brand (see docs/brand/README.md). -->\n'
def vector(dp, vp, body, tint=None):
    t = f'\n    android:tint="{tint}"' if tint else ""
    return (HDR + f'<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
            f'    android:width="{dp}dp" android:height="{dp}dp"\n'
            f'    android:viewportWidth="{vp}" android:viewportHeight="{vp}"{t}>\n{body}</vector>\n')
RES = "apps/mobile/app/src/main/res/"
# pre-O launcher (API 24/25): the full tile
write(RES + "drawable/ic_launcher.xml", vector(48, 256, vpath(BG, TILE) + vglyph()))
# adaptive launcher (API 26+): glyph inside the 66dp safe zone of a 108dp layer
S = 0.30
T = 54 - 128 * S
write(RES + "drawable/ic_launcher_foreground.xml", vector(108, 108,
      f'    <group android:scaleX="{S}" android:scaleY="{S}"\n        android:translateX="{T:g}" android:translateY="{T:g}">\n'
      + vglyph().replace("        <path", "        <path") + "    </group>\n"))
write(RES + "drawable/ic_launcher_monochrome.xml", vector(108, 108,
      f'    <group android:scaleX="{S}" android:scaleY="{S}"\n        android:translateX="{T:g}" android:translateY="{T:g}">\n'
      + vglyph("#FFFFFFFF", "#FFFFFFFF", 0.4) + "    </group>\n"))
write(RES + "drawable-anydpi-v26/ic_launcher.xml", HDR +
      '<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">\n'
      '    <background android:drawable="@color/ic_launcher_background"/>\n'
      '    <foreground android:drawable="@drawable/ic_launcher_foreground"/>\n'
      '    <monochrome android:drawable="@drawable/ic_launcher_monochrome"/>\n'
      '</adaptive-icon>\n')
write(RES + "values/ic_launcher_background.xml", '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n'
      f'    <color name="ic_launcher_background">{BG.upper()}</color>\n</resources>\n')
# status bar: alpha-only glyph (a colored icon renders as a solid blob there)
G = 4  # margin, in glyph units
write(RES + "drawable/ic_notification.xml", vector(24, 168,
      f'    <group android:translateX="{-X0+8:g}" android:translateY="{-Y0+G:g}">\n'
      + vglyph("#FFFFFFFF", "#FFFFFFFF", 0.4) + "    </group>\n"))
# in-app brand mark (connect screen)
write(RES + "drawable/pulpit_glyph.xml", vector(38, 160,
      f'    <group android:translateX="{-X0+4:g}" android:translateY="{-Y0:g}">\n' + vglyph() + "    </group>\n"))
print("ok", BOWL)
