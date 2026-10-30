---
name: Pulpit — Settings
description: Settings as a quiet dark sheet — a side nav of pages, each page a stack of grouped cards with one control per row; teal for intent and status-ok, red for errors only.
colors:
  ground: "#171c21"
  tile: "#262e36"
  tile-raised: "#313a44"
  line: "#1d2a36"
  well: "#171c21"
  lift: "#3a4753"
  ink: "#ecf0f1"
  ink-2: "#95a5a6"
  ink-3: "#8b97a1"
  accent: "#1abc9c"
  accent-deep: "#16a085"
  ok: "#1abc9c"
  bad: "#e74c3c"
  bad-ink: "#f19488"
typography:
  bar-title:
    fontFamily: "Roboto, Segoe UI, system-ui, sans-serif"
    fontSize: "17px"
    fontWeight: 600
    letterSpacing: "0.2px"
  page-title:
    fontFamily: "Roboto, Segoe UI, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 600
  group-label:
    fontFamily: "Roboto, Segoe UI, system-ui, sans-serif"
    fontSize: "11.5px"
    fontWeight: 600
    letterSpacing: "1.4px"
  nav-item:
    fontFamily: "Roboto, Segoe UI, system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
  row-name:
    fontFamily: "Roboto, Segoe UI, system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 500
  lead:
    fontFamily: "Roboto, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.6
  note:
    fontFamily: "Roboto, Segoe UI, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
  chip:
    fontFamily: "Roboto, Segoe UI, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 400
  data:
    fontFamily: "ui-monospace, Cascadia Mono, Consolas, monospace"
    fontSize: "13px"
    fontWeight: 400
  data-display:
    fontFamily: "ui-monospace, Cascadia Mono, Consolas, monospace"
    fontSize: "30px"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "6px"
rounded:
  control: "6px"
  panel: "8px"
  card: "10px"
  chip: "999px"
spacing:
  bar-height: "56px"
  nav-width: "220px"
  page-max-width: "760px"
  page-padding: "28px 40px 64px"
  group-gap: "28px"
components:
  card:
    backgroundColor: "{colors.tile}"
    rounded: "{rounded.card}"
    padding: "0 18px"
  nav-item-current:
    backgroundColor: "{colors.tile}"
    textColor: "{colors.ink}"
  switch:
    backgroundColor: "{colors.tile-raised}"
    rounded: "13px"
    width: "46px"
    height: "26px"
  switch-on:
    backgroundColor: "{colors.accent}"
  chip:
    backgroundColor: "{colors.tile-raised}"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.chip}"
    padding: "3px 9px"
  chip-ok:
    backgroundColor: "rgba(26, 188, 156, 0.14)"
    textColor: "{colors.ok}"
  chip-bad:
    backgroundColor: "rgba(231, 76, 60, 0.14)"
    textColor: "{colors.bad-ink}"
  act:
    backgroundColor: "{colors.tile-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  act-primary:
    backgroundColor: "{colors.accent}"
    textColor: "#0c2a24"
    rounded: "{rounded.control}"
    padding: "8px 14px"
  field:
    backgroundColor: "{colors.well}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "7px 10px"
  code-display:
    backgroundColor: "{colors.well}"
    rounded: "{rounded.panel}"
    padding: "12px 18px"
  qr:
    backgroundColor: "#ffffff"
    rounded: "{rounded.panel}"
    padding: "6px"
    width: "116px"
    height: "116px"
---

# Design System: Pulpit — Settings

## Overview

**North Star: "a quiet sheet you leave quickly"**

Settings is a full-screen dark overlay over the editor: a 56px top bar (title +
close), a 220px side nav, and one scrolling page. The nav lists the core pages
(Ogólne, Tablety), then the integrations under an "Integracje" label (OBS,
Spotify, Discord, Voicemeeter, Limity AI), with "O Pulpicie" pinned to the bottom.
A page is a title with an optional status chip, one lead sentence, and a stack of
labeled groups; each group is a card of rows, one setting per row: name and note
on the left, the control on the right.

Every page tells you its state before you touch anything: a chip in the page head
(połączono / wymaga logowania / nie zainstalowano), the row notes, and the live
values (addresses, the port, the hotkey). Changes apply as you make them: switches
save on flip, the hotkey saves on capture, and the integration toggles connect at
once. An explicit Save button appears only for typed text, and only while it differs
from what is stored.

**Scope.** This world owns one surface: the settings overlay (`apps/desktop`,
`SettingsOverlay.vue` as the shell, one component per page in
`components/settings/`, shared sheet `components/settings/settings.css`). The
light sidebar, the board canvas and the light modal system are the **incumbent
world** inherited from Deckboard. Colors come from the shell tokens
(`style.css :root`), so both worlds share one palette; their modal language,
radii and shadows still differ, by decision.

**Key Characteristics:**
- Side nav + one page at a time; pages mount only while shown, so integration
  probes (Voicemeeter DLL, Discord RPC) run only on their own page
- The overlay reopens on the last page shown
- Grouped cards of rows; one control per row, right-aligned
- Changes apply immediately; nothing waits for a global Save
- Tonal depth, no elevation shadows
- Teal for intent and status-ok, red for errors only
- Roboto for prose, mono + tabular numerals for every technical value

## Colors

The incumbent charcoal family, one teal accent and one error red. Every neutral
and state value maps to a shell token (`--titlebar`, `--rail`, `--rail-hover`,
`--border`, `--text`, `--text-muted`, `--accent`, `--danger`); only `--ink-3` and
`--bad-ink` are derived here.

- **Ground** (#171c21): overlay background, recessed fields, the code display.
- **Tile** (#262e36): cards and the current nav item.
- **Tile Raised** (#313a44): buttons, chips, switch track, row separators.
- **Line** (#1d2a36): 1px outlines: bar and nav borders, cards, fields.
- **Lift** (#3a4753): hover for buttons and the switch track.
- **Ink / Ink 2 / Ink 3** (#ecf0f1 / #95a5a6 / #8b97a1): names and values / lead
  text and idle nav items / notes and group labels.
- **Accent Teal** (#1abc9c), **Accent Deep** (#16a085 hover).
- **Bad Red** (#e74c3c) as a surface tint, **Bad Ink** (#f19488) whenever red is
  small text on dark.

### Named Rules
**The Two-Channel Rule.** Teal and red never mix. Teal = intent and life: the
current nav marker, switch-on, the page's primary button, focus rings, ok chips.
Red = error: bad chips, error lines, the nav alert dot, danger buttons
(Odłącz / Cofnij / Wyloguj).

**The Rationed Accent Rule.** At most one filled teal (primary) button per page,
the main action (Wygeneruj kod, Połącz, Zaloguj). Confirming steps inside a row
use the accent rank (teal text on a raised fill).

## Typography

Roboto (400/500/700 via @fontsource) for all voice; `ui-monospace, "Cascadia Mono",
Consolas` with tabular numerals for computer-authored values.

- **Bar title** 600 17px: "Ustawienia".
- **Page title** 600 20px, beside its chip.
- **Lead** 400 13px/1.6, Ink 2, max 64ch: one sentence on what the page controls.
- **Group label** 600 11.5px, +1.4px, uppercase, Ink 3. The nav's "Integracje"
  label uses the same voice at 11px.
- **Row name** 500 13.5px; **row note** 400 12px/1.5 Ink 3.
- **Chip** 11px. **Data** mono 13px. **Data display** mono 700 30px +6px: the
  pairing code only.

**The Mono-Values Rule.** Ports, IPs, hotkeys, codes, client IDs, redirect URIs and
paths are monospace with tabular numerals. Prose never imitates mono.

## Layout

Fixed opaque overlay (`inset: 0`, z-index 45, above the light modals at 40; the
shared confirm dialog sits above at 120). Bar 56px. Nav 220px with its right
border; it collapses to a 64px icon rail under 760px window width. The page column
is max 760px wide with 28px/40px padding; groups sit 28px apart. Rows are min 58px
tall with 13px vertical padding and 1px Tile Raised separators. Text blocks inside
a card (QR + address list, the pairing code) use a `card-body` with 16px padding.

## Elevation & Depth

Tonal only: ground → tile → raised, with fields dipping back to ground, all
outlined by 1px lines. The switch knob's micro-shadow is the only shadow in the
world. Something that needs more emphasis takes a tonal step or the accent, not a
shadow.

## Shapes

Cards 10px; nav items, segmented controls, the QR plate and the code display 8px;
buttons, fields, kbd plates and address rows 6px; chips are pills; the switch is
46×26 with a 20px knob. The QR renders on a white 116px plate, the only white
surface, because scanners need it.

## Components

Motion: state changes 140–180ms ease-out; the switch knob uses
`cubic-bezier(0.2, 0, 0, 1)`; a page enters with one 180ms rise (opacity,
translateY 4px). Icons are Font Awesome solid only (no brand set is bundled).

### Shell (`SettingsOverlay.vue`)
- `role="dialog"` + `aria-modal`; Escape closes; Tab wraps inside the overlay.
- Nav items are buttons with `aria-current="page"` on the current one: Tile fill,
  Ink text, a 3px teal bar on the left edge, teal icon. A red dot on Tablety
  flags a dead server.
- `initialPage` opens a specific page; otherwise the last page shown.

### Row
Name + note on the left (`row-text`), control group on the right (`row-ctl`,
8px gaps). `.row.top` top-aligns rows whose note runs several lines. Validation
and backend errors render as `.err` (Bad Ink 12px) under the card; transient
confirmations as `.note.ok` (teal), cleared after a few seconds.

### Switch
`role="switch"` + `aria-checked`; applies immediately, disabled while the call is
in flight, rolls back and shows the reason on failure.

### Buttons (`.act`)
Default (raised fill), **accent** (teal text), **primary** (teal fill,
#0c2a24 text, one per page), **danger** (Bad Ink text, red-tinted hover),
**ghost** (transparent, Ink 2; refresh buttons, disclosure toggles, the
eye toggle for secrets).

### Shortcut capture
The current combo shows as a mono kbd plate. "Zmień" turns the plate into a
focused capture target ("Naciśnij klawisze…", teal outline); the next key press
with Ctrl/Alt/Win (or a bare F-key) saves at once. Esc cancels; a bare letter is
refused with a reason.

### Fields
Recessed (ground fill, line border), 6px; focus = teal border + 1px teal ring.
Widths: 220px default, 280px `.wide`, 90px `.narrow`. Secrets are never echoed
back: a saved secret shows as a placeholder and the note says "type a new one to
replace it".

### Chips
Read-only live state in page heads and lists; ok = teal on 14% teal, bad = Bad Ink
on 14% red, neutral = raised fill.

### Help steps
Integrations that need a developer app (Spotify, Discord) carry a ghost
disclosure "How to create …" with a numbered list of whole sentences. It opens
by itself while the integration is not set up yet.

## Do's and Don'ts

### Do:
- **Do** give every page a state chip or a status row that reads the backend honestly.
- **Do** apply changes immediately; show Save only for edited text, and save it implicitly before an action that needs it (Connect, Log in).
- **Do** keep one setting per row, control on the right.
- **Do** put technical values in mono with tabular numerals.
- **Do** confirm destructive actions through the shared `ask()` dialog (kind "warning").
- **Do** poll only while the page that needs it is mounted.

### Don't:
- **Don't** add a global Save/Apply button or a second primary button on a page.
- **Don't** show settings that cannot change (fixed behavior) as controls; if it must be said, say it in a note.
- **Don't** use shadows for depth or red as decoration.
- **Don't** hard-code Polish (or any language) in backend status strings; send a state code and word it in the frontend through `t()`.
- **Don't** import incumbent modal idioms (white dialogs, uppercase text buttons, 4px radius) into settings, or settings idioms into them.
