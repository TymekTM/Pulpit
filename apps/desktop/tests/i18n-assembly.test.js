import { describe, it, expect, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";
import SettingsOverlay from "../src/components/SettingsOverlay.vue";
import EditTileModal from "../src/components/EditTileModal.vue";
import ToolTile from "../src/components/tiles/ToolTile.vue";
import { setLocaleForTest } from "../src/i18n.js";
import { handle } from "./tauri.js";

// The i18n assembly across all three locale domains: one UI-language
// switch to Polish must re-render a SETTINGS string (pl.json), a MODAL
// string (pl-modals.json) and an APP CHROME string (pl-chrome.json) on
// the same mounted components. English is the message key, so the
// English render is the key itself.

const status = { dbOk: true, port: 8500, clients: 0, version: "1.4.0" };

function settingsHandles() {
  handle("get_settings", () => ({ hotkey: "Ctrl+Alt+D", port: 8500, port_locked: false }));
  handle("get_autostart", () => false);
  handle("list_lan_addresses", () => []);
  handle("list_devices", () => []);
  handle("aidev_status_config", () => ({ detected: [], show: [], summary: true, row_style: "name" }));
  handle("spotify_status", () => ({ configured: true, clientId: "", loggedIn: false }));
}

const boards = [
  { id: 1, name: "Main" },
  { id: 2, name: "Stream" },
  { id: 3, name: "Games" },
];

const buttonTile = {
  id: 10,
  board_id: 1,
  type: "key",
  mode: "button",
  command: "CTRL + K",
  title: "Save",
  color: "#123456",
  border_color: "",
  shape: 0,
  title_position: 0,
  title_color: "",
  title_box_color: "",
  icon: "",
  icon_color: "",
  img: "",
  color2: "",
  icon2: "",
  img2: "",
  options: "",
};

const buttonText = (w, text) =>
  w.findAll("button").find((b) => b.text().trim() === text);

afterEach(() => {
  // pin back even if an assertion threw - setup.js re-pins before each
  // test anyway, but never leak Polish into a same-file later test
  setLocaleForTest("en");
  document.body.innerHTML = "";
});

describe("i18n assembly", () => {
  it("switching to pl re-renders a settings, a modal and a chrome string", async () => {
    settingsHandles();

    const settings = mount(SettingsOverlay, { props: { status }, attachTo: document.body });
    const edit = mount(EditTileModal, {
      props: { button: buttonTile, boards },
      attachTo: document.body,
    });
    const tool = mount(ToolTile, {
      // not running, part-way elapsed -> the "Paused" status from chrome
      props: {
        tile: { id: 1, type: "tool-timer", title: "" },
        state: { running: false, elapsedMs: 60000 },
      },
      attachTo: document.body,
    });
    await flushPromises();

    // English renders the key itself
    expect(settings.find("h1").text()).toBe("Settings");
    expect(buttonText(edit, "Cancel")).toBeTruthy();
    expect(tool.find(".tool-sub").text()).toBe("Paused");

    setLocaleForTest("pl");
    await nextTick();
    await nextTick();

    // settings domain (pl.json)
    expect(settings.find("h1").text()).toBe("Ustawienia");
    // modal domain (pl-modals.json)
    expect(buttonText(edit, "Anuluj")).toBeTruthy();
    // chrome domain (pl-chrome.json)
    expect(tool.find(".tool-sub").text()).toBe("Pauza");

    setLocaleForTest("en");
    await nextTick();
    expect(settings.find("h1").text()).toBe("Settings");
  });

  it("renders the tool clock date in the active UI language", async () => {
    const w = mount(ToolTile, {
      props: { tile: { id: 2, type: "tool-clock", title: "" }, cmd: { date: "yes" } },
      attachTo: document.body,
    });
    setLocaleForTest("en");
    await nextTick();
    const enDate = w.find(".tool-sub").text();
    // English and Polish short weekday names are disjoint sets, so the
    // rendered date proves which tag Intl.DateTimeFormat received
    expect(enDate).toMatch(/sun|mon|tue|wed|thu|fri|sat/i);

    setLocaleForTest("pl");
    await nextTick();
    const plDate = w.find(".tool-sub").text();
    expect(plDate).toMatch(/nd|pon|wt|śr|czw|pt|sob/);
    expect(plDate).not.toMatch(/sun|mon|tue|wed|thu|fri|sat/i);
    expect(plDate).not.toBe(enDate);

    setLocaleForTest("en");
    await nextTick();
    expect(w.find(".tool-sub").text()).toMatch(/sun|mon|tue|wed|thu|fri|sat/i);
  });
});
