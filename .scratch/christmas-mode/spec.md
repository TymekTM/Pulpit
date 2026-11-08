# Tryb świąteczny (christmas mode)

Status: in-progress

## Co to jest

A seasonal skin plus a few small festive extras, switched on automatically
during the Christmas season and controllable from Settings. Only the look
changes. Boards, tiles and commands behave exactly as before.

Decisions from the owner (2026-11-08), extended the same day with three
more seasons (Halloween, Wielkanoc, Walentynki), which get the skin and
effects but no board template:

- Scope: the desktop look, the look on tablets, a festive board template
  with tiles, and press effects. All four.
- Activation: on automatically during the season, with a switch in
  Settings to force it on or turn it off.

## Season resolution (one source of truth)

- New crate `crates/season` (`pulpit-season`), a producer like `sysinfo`.
- Mode is stored in `~/pulpitApp/editor.json` under `"season"`:
  `"auto"` (the default when the key is missing or unknown), `"off"`, or a
  season's wire name to force that season (`"christmas"`, `"halloween"`,
  `"easter"`, `"valentines"`). `"on"` from the first build reads as
  `"christmas"`.
- With `auto` the calendar decides, local time, every window inclusive:

  | Season | Wire name | Window |
  | --- | --- | --- |
  | Walentynki | `valentines` | 10-14 February |
  | Wielkanoc | `easter` | Palm Sunday to Easter Monday (Gregorian computus) |
  | Halloween | `halloween` | 25 October to 1 November |
  | Boże Narodzenie | `christmas` | 1 December to 6 January (Trzech Króli) |

  The windows never overlap (Easter falls between 22 March and 25 April).
- The producer pushes two custom-value keys through the normal producer
  lanes (`pulpit_host::forward_producer`), so they reach v2 (`ext.<key>`),
  the legacy hub and the desktop webview with no new wire types:
  - `season`: the effective season's wire name, or `"off"`.
  - `season-countdown`: the days until 24 December as a display string in
    the payload locale (`"46 dni"` / `"46 days"`, `"Wigilia!"` on the day
    itself, then the count to next year's). It is a plain string because
    every custom-value renderer (desktop, stock client, v2 Toggle text)
    shows strings verbatim and ignores objects.
- `season-countdown` is also a native input kind (`input_declarations`,
  registered by `pulpit_host::register_inputs`) with mode
  `custom-value`: legacy `app: "custom-value"`, a v2 Toggle on
  `ext.season-countdown`. Tiles store it with the ordinary press mode
  `button`; the registry, not the row, makes it a live-value tile.
- The producer re-checks every few minutes, so it rolls over at midnight,
  and it re-checks right away when the mode changes (a `watch` channel
  from the desktop). The headless server reads the mode from
  `editor.json` once at startup (`auto` if the file is missing) and
  cannot change it at runtime.

## Surfaces

| Surface | What changes |
| --- | --- |
| Desktop (editor + touch mode) | `data-season="<name>"` on `<html>`. Per season (src/season.css): an accent (holly green, pumpkin orange, lavender, rose; never red), a string of lights under the top bar in the season's colours, ambient particles behind the board (snow, autumn leaves, petals, rising hearts) and a press burst in touch mode (snowflakes, embers, eggs, hearts). Settings → General → Appearance gets a "Motyw sezonowy" select: Auto (kalendarz) / Nigdy / one entry per season. |
| Server | Pushes `season` and `season-countdown` (above). Nothing in `crates/proto` changes: the values ride the existing `state.sync` / `state.patch` frames as `ext.season` / `ext.season-countdown`. The fixtures gain these channels so `ProtoFixturesTest` pins the parse. |
| Android (`apps/mobile`) | Reads `ext.season` from the state values (`seasonOf`, `ui/Season.kt`). The same particles, bursts and accents as the desktop, drawn on a Canvas; the board chip takes the season's accent. |
| Stock Deckboard client | Gets no skin (it can't render one). It does get the template board and the countdown value. |

On the desktop every animation honours `prefers-reduced-motion`. Android
deliberately does not gate on the animator duration scale: deck tablets
are routinely set to scale 0, which would switch the feature off for
exactly its audience. Instead motion runs on raw frame time
(`withFrameNanos`, ~30 fps, draw phase only), and the Compose frame
clock stops on its own while the app is in the background.

## Template board

"Albo zacznij od szablonu → Święta" in the new-board dialog (create
mode only) builds a 6x3 board in one go (`apps/desktop/src/templates.js`):

- a 2x2 untitled countdown tile (`season-countdown`),
- a 2x1 clock (`tool-clock`, 24h),
- media keys: previous / "Kolędy" (play-pause) / next, plus quieter,
  louder and mute,
- a 6x1 volume slider (`speaker-volume`) along the bottom.

Tile faces use pine, holly green, a muted red and gold. Titles go through
`t()` at creation time. The board and its tiles count as one undo step
(`boardCreateFromSnapshotCommand`). If any tile fails, the half-built
board is deleted. A template is ordinary tile data, so the server and the
tablets need nothing new for it.

## Out of scope

Board templates and countdowns for the seasons other than Christmas.
