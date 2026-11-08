// Ready-made boards for the new-board dialog. A template is plain tile
// data in the editor's own button shape (the same objects create_button +
// update_button take), so nothing on the server or the tablets knows
// templates exist. Titles go through t() when the board is created, so
// the board comes out in the UI language of the moment.
import { t } from "./i18n.js";

// tile faces: festive but muted, so white icons and labels stay readable
const PINE = "#1d2e25";
const GREEN = "#2f7d4a";
const RED = "#a8323a";
const GOLD = "#b8862b";

/// The Christmas board (.scratch/christmas-mode/spec.md): the Wigilia
/// countdown, a clock, playback for the carols and the volume.
function christmasTiles() {
  const tile = (x, y, w, h, fields) => ({
    x,
    y,
    w,
    h,
    mode: "button",
    command: "",
    title: "",
    ...fields,
  });
  return [
    // no title: a custom-value tile shows its value only untitled
    tile(0, 0, 2, 2, { type: "season-countdown", color: GREEN, icon: "gifts" }),
    tile(2, 0, 2, 1, { type: "tool-clock", command: JSON.stringify({ format: "24h" }), color: RED, icon: "clock" }),
    tile(4, 0, 1, 1, { type: "vol", command: "vol_down", color: GOLD, icon: "volume-down", title: t("Quieter") }),
    tile(5, 0, 1, 1, { type: "vol", command: "vol_up", color: GOLD, icon: "volume-up", title: t("Louder") }),
    tile(2, 1, 1, 1, { type: "vol", command: "prev", color: GREEN, icon: "step-backward" }),
    tile(3, 1, 1, 1, { type: "vol", command: "play", color: RED, icon: "music", title: t("Carols") }),
    tile(4, 1, 1, 1, { type: "vol", command: "next", color: GREEN, icon: "step-forward" }),
    tile(5, 1, 1, 1, { type: "vol", command: "vol_mute", color: GOLD, icon: "volume-mute" }),
    tile(0, 2, 6, 1, { type: "speaker-volume", mode: "slider", color: PINE, icon: "sliders-h", title: t("Volume") }),
  ];
}

export const TEMPLATES = [
  {
    id: "christmas",
    label: "Christmas",
    icon: "tree",
    board: () => ({
      name: t("Christmas"),
      background: PINE,
      width: 6,
      height: 3,
      buttons: christmasTiles(),
    }),
  },
];

/// Create a template board through the editor API. Returns the snapshot
/// of what was created (board id + tiles with their new ids), the shape
/// boardCreateFromSnapshotCommand records for undo.
export async function createFromTemplate(api, template) {
  const spec = template.board();
  const boardId = await api.createBoard(spec.name, spec.background, spec.width, spec.height);
  const buttons = [];
  try {
    for (const b of spec.buttons) {
      const id = await api.createButton(boardId, b.type, b.mode, b.x, b.y);
      const full = { ...b, id, board_id: boardId };
      await api.updateButton(full);
      buttons.push(full);
    }
  } catch (e) {
    // no half-built board left behind: the dialog reports the error and
    // nothing lands in the undo history
    await api.deleteBoard(boardId).catch(() => {});
    throw e;
  }
  return { id: boardId, name: spec.name, background: spec.background, width: spec.width, height: spec.height, buttons };
}
