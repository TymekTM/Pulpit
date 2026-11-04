# Pulpit logo

The mark is a capital **P built from deck keys**: a tall stem key, a bowl
key with a round end and an open counter, and one spare key still dark -
a board with a free slot waiting for its next action. It sits on the
`--ground` tile (`#171c21`) in the shared accent (`#1abc9c`); the spare
key uses `--tile-2` (`#2c353e`). The wordmark is Inter SemiBold, the
chrome font, converted to outlines.

| File | Use |
| --- | --- |
| `pulpit-icon.svg` | App icon: glyph on the rounded dark tile |
| `pulpit-glyph.svg` | Glyph alone, for dark UI (sidebar, connect screen) |
| `pulpit-glyph-mono.svg` | One-color glyph (spare key at 35% opacity) |
| `pulpit-lockup-dark.svg` | Icon + wordmark on dark backgrounds |
| `pulpit-lockup-light.svg` | Icon + wordmark on light backgrounds |

Every asset - these SVGs, `apps/desktop/public/favicon.svg` and the
Android drawables (`ic_launcher*`, `ic_notification`, `pulpit_glyph`) -
comes from one geometry in `generate.py`. Change the shape there and
rerun it from the repo root:

```bash
python3 docs/brand/generate.py .
```

It needs `fonttools` for the wordmark. The desktop raster icons
(`apps/desktop/src-tauri/icons/icon.ico` with 16-256 px frames, and the
512 px `icon.png`) are renders of `pulpit-icon.svg`; regenerate them
with any SVG renderer when the shape changes.

Rules: keep the glyph upright and the spare key dark (it is part of the
mark, not decoration); give it at least a quarter of its width of clear
space; below 16 px use the app icon, never the bare glyph.
