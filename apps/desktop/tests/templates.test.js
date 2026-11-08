import { describe, it, expect, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { calls, handle } from "./tauri.js";
import { api } from "../src/api.js";
import { TEMPLATES, createFromTemplate } from "../src/templates.js";
import { boardCreateFromSnapshotCommand } from "../src/editorHistory.js";
import BoardModal from "../src/components/BoardModal.vue";
import { CATALOG } from "../src/catalog.js";

afterEach(() => {
  document.body.innerHTML = "";
});

const christmas = TEMPLATES.find((t) => t.id === "christmas");

function fakeIds() {
  let next = 100;
  handle("create_board", () => 7);
  handle("create_button", () => next++);
}

describe("christmas template", () => {
  it("fits its board and never overlaps two tiles", () => {
    const spec = christmas.board();
    const taken = new Set();
    for (const b of spec.buttons) {
      expect(b.x + b.w).toBeLessThanOrEqual(spec.width);
      expect(b.y + b.h).toBeLessThanOrEqual(spec.height);
      for (let x = b.x; x < b.x + b.w; x++)
        for (let y = b.y; y < b.y + b.h; y++) {
          expect(taken.has(`${x},${y}`)).toBe(false);
          taken.add(`${x},${y}`);
        }
    }
  });

  it("uses only kinds the tile catalog knows", () => {
    for (const b of christmas.board().buttons) expect(CATALOG.some((c) => c.value === b.type), b.type).toBe(true);
  });

  it("shows the Wigilia countdown on an untitled custom-value tile", () => {
    const countdown = christmas.board().buttons.find((b) => b.type === "season-countdown");
    expect(countdown).toMatchObject({ mode: "button", title: "" });
  });

  it("creates the board, then every tile in place", async () => {
    fakeIds();
    const snap = await createFromTemplate(api, christmas);
    const spec = christmas.board();
    expect(calls[0]).toEqual({
      cmd: "create_board",
      args: { name: spec.name, background: spec.background, width: 6, height: 3 },
    });
    expect(calls.filter((c) => c.cmd === "create_button")).toHaveLength(spec.buttons.length);
    const updates = calls.filter((c) => c.cmd === "update_button").map((c) => c.args.button);
    expect(updates.map((b) => b.board_id)).toEqual(spec.buttons.map(() => 7));
    expect(updates[0]).toMatchObject({ id: 100, type: "season-countdown", w: 2, h: 2 });
    expect(snap).toMatchObject({ id: 7, name: spec.name, width: 6, height: 3 });
    expect(snap.buttons.map((b) => b.id)).toEqual(updates.map((b) => b.id));
  });

  it("deletes the half-built board when a tile fails", async () => {
    fakeIds();
    let n = 0;
    handle("update_button", () => {
      if (++n === 3) throw "db locked";
    });
    await expect(createFromTemplate(api, christmas)).rejects.toBe("db locked");
    expect(calls.at(-1)).toEqual({ cmd: "delete_board", args: { boardId: 7 } });
  });
});

describe("boardCreateFromSnapshotCommand", () => {
  it("undo deletes the board, redo rebuilds board and tiles", async () => {
    fakeIds();
    const snap = await createFromTemplate(api, christmas);
    calls.length = 0;
    const remapped = [];
    const ctx = {
      remapBoardId: (a, b) => remapped.push(["board", a, b]),
      remapTileId: (a, b) => remapped.push(["tile", a, b]),
    };
    const cmd = boardCreateFromSnapshotCommand({ boardSnapshot: snap });
    await cmd.undo(api, ctx);
    expect(calls).toEqual([{ cmd: "delete_board", args: { boardId: 7 } }]);

    calls.length = 0;
    handle("create_board", () => 8);
    await cmd.redo(api, ctx);
    expect(calls[0].cmd).toBe("create_board");
    expect(calls.filter((c) => c.cmd === "update_button")).toHaveLength(snap.buttons.length);
    expect(remapped[0]).toEqual(["board", 7, 8]);

    // the next undo targets the rebuilt board
    calls.length = 0;
    await cmd.undo(api, ctx);
    expect(calls).toEqual([{ cmd: "delete_board", args: { boardId: 8 } }]);
  });
});

describe("BoardModal templates", () => {
  const templateButton = (w) => w.find(".template-btn");

  it("offers templates only when creating", () => {
    expect(templateButton(mount(BoardModal, { props: { mode: "create" } })).exists()).toBe(true);
    const board = { id: 3, name: "Main", width: 5, height: 3, background: "#101010" };
    expect(templateButton(mount(BoardModal, { props: { mode: "edit", board } })).exists()).toBe(false);
  });

  it("emits the created snapshot as a template save", async () => {
    fakeIds();
    const w = mount(BoardModal, { props: { mode: "create" } });
    expect(templateButton(w).text()).toBe("Christmas");
    await templateButton(w).trigger("click");
    await flushPromises();
    const [[info]] = w.emitted("saved");
    expect(info.mode).toBe("template");
    expect(info.board.id).toBe(7);
    expect(info.board.buttons).toHaveLength(christmas.board().buttons.length);
  });

  it("keeps the dialog open on failure", async () => {
    handle("create_board", () => {
      throw "disk full";
    });
    const w = mount(BoardModal, { props: { mode: "create" } });
    await templateButton(w).trigger("click");
    await flushPromises();
    expect(w.find(".modal-error").text()).toBe("disk full");
    expect(w.emitted("saved")).toBeUndefined();
  });
});
