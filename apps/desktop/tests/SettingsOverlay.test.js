import { describe, it, expect, afterEach, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import SettingsOverlay from "../src/components/SettingsOverlay.vue";
import GeneralPage, { comboFromEvent, comboAllowed } from "../src/components/settings/GeneralPage.vue";
import TabletsPage from "../src/components/settings/TabletsPage.vue";
import AboutPage from "../src/components/settings/AboutPage.vue";
import ObsPage from "../src/components/settings/ObsPage.vue";
import SpotifyPage from "../src/components/settings/SpotifyPage.vue";
import DiscordPage from "../src/components/settings/DiscordPage.vue";
import VoicemeeterPage from "../src/components/settings/VoicemeeterPage.vue";
import AidevPage from "../src/components/settings/AidevPage.vue";
import { calls, handle, dialog } from "./tauri.js";

// Pages are mounted on their own: their script-setup bindings are
// reachable through wrapper.vm (test-utils proxies setupState), which
// keeps these tests on behavior - the api calls and the state the
// template renders - instead of on markup details.

const status = { dbOk: true, port: 8500, clients: 0, version: "1.4.0" };

function defaults() {
  handle("get_settings", () => ({ hotkey: "Ctrl+Alt+D", port: 8500, port_locked: false }));
  handle("get_autostart", () => false);
  handle("list_lan_addresses", () => [{ ipv4: "192.168.1.10", name: "Wi-Fi", qr: "data:qr" }]);
  handle("list_devices", () => []);
  handle("adb_devices", () => []);
  handle("aidev_status_config", () => ({ detected: [], show: [], summary: true, row_style: "name" }));
  handle("spotify_status", () => ({ configured: true, clientId: "", loggedIn: false }));
  handle("check_for_updates", () => ({ update_available: false, latest: "1.4.0", current: "1.4.0" }));
}

async function mountPage(component, props = {}) {
  const w = mount(component, { props, attachTo: document.body });
  await flushPromises();
  return w;
}

const cmds = (name) => calls.filter((c) => c.cmd === name);

afterEach(() => {
  document.body.innerHTML = "";
  vi.useRealTimers();
});

describe("SettingsOverlay shell", () => {
  it("opens on the requested page and switches pages from the nav", async () => {
    defaults();
    const w = await mountPage(SettingsOverlay, { status, initialPage: "general" });
    expect(w.find("[aria-current='page']").attributes("data-page")).toBe("general");
    expect(cmds("get_autostart")).toHaveLength(1);
    // pages load lazily: nothing from the other pages ran yet
    expect(cmds("list_devices")).toHaveLength(0);
    expect(cmds("discord_status")).toHaveLength(0);

    await w.find("[data-page='tablets']").trigger("click");
    await flushPromises();
    expect(w.find("[aria-current='page']").attributes("data-page")).toBe("tablets");
    expect(cmds("list_devices")).toHaveLength(1);
    expect(w.text()).toContain("192.168.1.10:8500");
  });

  it("reopens on the last page shown", async () => {
    defaults();
    const first = await mountPage(SettingsOverlay, { status, initialPage: "about" });
    first.unmount();
    const again = await mountPage(SettingsOverlay, { status });
    expect(again.find("[aria-current='page']").attributes("data-page")).toBe("about");
  });

  it("flags the tablets page while the server is down", async () => {
    defaults();
    const w = await mountPage(SettingsOverlay, { status: { ...status, dbOk: false }, initialPage: "general" });
    expect(w.find("[data-page='tablets'] .nav-alert").exists()).toBe(true);
  });

  it("closes on Escape and the close button", async () => {
    defaults();
    const w = await mountPage(SettingsOverlay, { status, initialPage: "general" });
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await w.find("button.close").trigger("click");
    expect(w.emitted("close")).toHaveLength(2);
    w.unmount();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
  });
});

describe("General page", () => {
  it("loads the stored values", async () => {
    defaults();
    handle("get_settings", () => ({ hotkey: "Ctrl+Shift+P", port: 8500, start_minimized: true }));
    handle("get_autostart", () => true);
    const w = await mountPage(GeneralPage);
    expect(w.vm.hotkey).toBe("Ctrl+Shift+P");
    expect(w.vm.startMinimized).toBe(true);
    expect(w.vm.autostart).toBe(true);
  });

  it("keeps defaults when the backend is unreachable", async () => {
    handle("get_settings", () => {
      throw "down";
    });
    handle("get_autostart", () => {
      throw "down";
    });
    const w = await mountPage(GeneralPage);
    expect(w.vm.hotkey).toBe("Ctrl+Alt+D");
    expect(w.vm.autostart).toBe(false);
  });

  it("flips autostart and reverts on failure", async () => {
    defaults();
    const w = await mountPage(GeneralPage);
    await w.vm.toggleAutostart();
    expect(cmds("set_autostart")[0].args).toEqual({ enable: true });
    expect(w.vm.autostart).toBe(true);
    handle("set_autostart", () => {
      throw "registry locked";
    });
    await w.vm.toggleAutostart();
    expect(w.vm.autostart).toBe(true);
    expect(w.vm.autostartError).toBe("registry locked");
  });

  it("flips start-minimized, reverting on failure", async () => {
    defaults();
    handle("get_settings", () => ({ hotkey: "X", start_minimized: true }));
    const w = await mountPage(GeneralPage);
    await w.vm.toggleStartMinimized();
    expect(cmds("set_start_minimized")[0].args).toEqual({ enable: false });
    expect(w.vm.startMinimized).toBe(false);
    handle("set_start_minimized", () => {
      throw "disk full";
    });
    await w.vm.toggleStartMinimized();
    expect(w.vm.startMinimized).toBe(false);
    expect(w.vm.startMinimizedError).toBe("disk full");
  });

  it("records the shortcut from a key press", async () => {
    defaults();
    const w = await mountPage(GeneralPage);
    await w.find("button.act").trigger("click");
    const capture = w.find(".kbd.capturing");
    expect(capture.exists()).toBe(true);
    // modifiers alone keep waiting
    await capture.trigger("keydown", { key: "Control", code: "ControlLeft", ctrlKey: true });
    expect(cmds("set_touch_mode_hotkey")).toHaveLength(0);
    await capture.trigger("keydown", { key: "t", code: "KeyT", ctrlKey: true, shiftKey: true });
    await flushPromises();
    expect(cmds("set_touch_mode_hotkey")[0].args).toEqual({ combo: "Ctrl+Shift+T" });
    expect(w.vm.hotkey).toBe("Ctrl+Shift+T");
    expect(w.vm.capturing).toBe(false);
  });

  it("refuses a bare key and keeps Escape inside the capture", async () => {
    defaults();
    const w = await mountPage(GeneralPage);
    await w.find("button.act").trigger("click");
    const capture = w.find(".kbd.capturing");
    await capture.trigger("keydown", { key: "a", code: "KeyA" });
    expect(cmds("set_touch_mode_hotkey")).toHaveLength(0);
    expect(w.vm.hotkeyError).toContain("Ctrl or Alt");
    const onWindow = vi.fn();
    window.addEventListener("keydown", onWindow);
    await capture.trigger("keydown", { key: "Escape", code: "Escape" });
    window.removeEventListener("keydown", onWindow);
    expect(onWindow).not.toHaveBeenCalled();
    expect(w.vm.capturing).toBe(false);
  });

  it("shows why a shortcut was refused and keeps the old one", async () => {
    defaults();
    handle("set_touch_mode_hotkey", () => {
      throw "taken by another app";
    });
    const w = await mountPage(GeneralPage);
    await w.vm.saveHotkey("Ctrl+C");
    await flushPromises();
    expect(w.vm.hotkey).toBe("Ctrl+Alt+D");
    expect(w.find(".err").text()).toBe("taken by another app");
  });

  it.each([
    [{ code: "KeyD", ctrlKey: true, altKey: true }, "Ctrl+Alt+D"],
    [{ code: "Digit5", altKey: true }, "Alt+5"],
    [{ code: "F9" }, "F9"],
    [{ code: "Space", metaKey: true }, "Super+Space"],
    [{ code: "ShiftLeft", shiftKey: true }, null],
  ])("builds a combo from %j", (event, combo) => {
    expect(comboFromEvent(event)).toBe(combo);
  });

  it.each([
    ["Ctrl+Alt+D", true],
    ["F9", true],
    ["Shift+F12", true],
    ["Shift+A", false],
    ["A", false],
  ])("allows %s: %s", (combo, ok) => {
    expect(comboAllowed(combo)).toBe(ok);
  });
});

describe("Tablets page", () => {
  it("lists addresses with the QR and resets the pick when the list shrinks", async () => {
    defaults();
    handle("list_lan_addresses", () => [
      { ipv4: "10.0.0.1", name: "a", qr: "q1" },
      { ipv4: "10.0.0.2", name: "b", qr: "q2" },
    ]);
    const w = await mountPage(TabletsPage, { status });
    await w.findAll("button.addr")[1].trigger("click");
    expect(w.find("img.qr").attributes("src")).toBe("q2");
    handle("list_lan_addresses", () => [{ ipv4: "10.0.0.1", name: "a", qr: "q1" }]);
    await w.vm.refreshLan();
    await flushPromises();
    expect(w.vm.activeAddress).toBe(0);
    expect(w.find("img.qr").attributes("src")).toBe("q1");
  });

  it("shows the no-network note when listing fails", async () => {
    defaults();
    handle("list_lan_addresses", () => {
      throw "no adapters";
    });
    const w = await mountPage(TabletsPage, { status });
    expect(w.vm.lanAddresses).toEqual([]);
    expect(w.text()).toContain("sees no local network");
  });

  it("shows a stored port that waits for a restart", async () => {
    defaults();
    handle("get_settings", () => ({ hotkey: "X", port: 9000 }));
    const w = await mountPage(TabletsPage, { status });
    expect(w.vm.portPending).toBe(9000);
    expect(w.text()).toContain("Port 9000 is saved");
  });

  it.each(["80", "70000", "abc", "1024.5", ""])("rejects port %j", async (draft) => {
    defaults();
    const w = await mountPage(TabletsPage, { status });
    w.vm.editPort();
    w.vm.portDraft = draft;
    await w.vm.savePort();
    expect(cmds("set_server_port")).toHaveLength(0);
    expect(w.vm.portError).toContain("1024");
  });

  it("saves a valid port as pending until restart", async () => {
    defaults();
    const w = await mountPage(TabletsPage, { status });
    w.vm.editPort();
    w.vm.portDraft = " 9100 ";
    await w.vm.savePort();
    expect(cmds("set_server_port")[0].args).toEqual({ port: 9100 });
    expect(w.vm.portPending).toBe(9100);
    expect(w.vm.editingPort).toBe(false);
    w.vm.editPort();
    w.vm.portDraft = "8500";
    await w.vm.savePort();
    expect(w.vm.portPending).toBeNull();
  });

  it("keeps the editor open with the backend reason for a refused port", async () => {
    defaults();
    handle("set_server_port", () => {
      throw "port in use";
    });
    const w = await mountPage(TabletsPage, { status });
    w.vm.editPort();
    w.vm.portDraft = "9100";
    await w.vm.savePort();
    expect(w.vm.portError).toBe("port in use");
    expect(w.vm.editingPort).toBe(true);
  });

  it("hides the port editor when the environment pins it", async () => {
    defaults();
    handle("get_settings", () => ({ port: 8500, port_locked: true }));
    const w = await mountPage(TabletsPage, { status });
    expect(w.text()).toContain("PULPIT_PORT");
    expect(w.findAll("button").some((b) => b.text() === "Change")).toBe(false);
  });

  it("mints a pairing code and counts it down to expiry", async () => {
    vi.useFakeTimers();
    defaults();
    handle("create_pairing_code", () => ({ code: "ABC123", expires_in_secs: 3, addresses: [] }));
    const w = await mountPage(TabletsPage, { status });
    await w.vm.generatePairing();
    expect(w.vm.pairingClock).toBe("0:03");
    await vi.advanceTimersByTimeAsync(1000);
    expect(w.vm.pairingClock).toBe("0:02");
    await vi.advanceTimersByTimeAsync(2000);
    expect(w.vm.pairingOffer).toBeNull();
    w.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("restarts the clock for a new code instead of stacking timers", async () => {
    vi.useFakeTimers();
    defaults();
    handle("create_pairing_code", () => ({ code: "X", expires_in_secs: 60 }));
    const w = await mountPage(TabletsPage, { status });
    const base = vi.getTimerCount(); // the device poll
    await w.vm.generatePairing();
    await w.vm.generatePairing();
    expect(vi.getTimerCount()).toBe(base + 1);
    w.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("reports minting failures", async () => {
    defaults();
    handle("create_pairing_code", () => {
      throw "v2 stack down";
    });
    const w = await mountPage(TabletsPage, { status });
    await w.vm.generatePairing();
    expect(w.vm.pairingError).toBe("v2 stack down");
    expect(w.vm.pairingBusy).toBe(false);
  });

  it("picks up a newly paired device while open", async () => {
    vi.useFakeTimers();
    defaults();
    const w = await mountPage(TabletsPage, { status });
    expect(w.vm.devices).toEqual([]);
    handle("list_devices", () => [{ id: "d1", name: "Tab", last_seen: 0 }]);
    await vi.advanceTimersByTimeAsync(4000);
    expect(w.vm.devices).toHaveLength(1);
    w.unmount();
  });

  describe("devices", () => {
    const device = { id: "d1", name: "Tablet", last_seen: 0 };

    it("revokes only after the confirmation", async () => {
      defaults();
      handle("list_devices", () => [device]);
      const w = await mountPage(TabletsPage, { status });
      await w.vm.revokeDevice(device);
      const [, opts] = dialog.ask.mock.calls[0];
      expect(opts).toMatchObject({ kind: "warning" });
      expect(opts.title).toContain("Tablet");
      await flushPromises();
      expect(cmds("revoke_device")[0].args).toEqual({ id: "d1" });
      expect(cmds("list_devices").length).toBeGreaterThanOrEqual(2);
    });

    it("does nothing when the confirmation is cancelled", async () => {
      defaults();
      dialog.ask.mockResolvedValueOnce(false);
      const w = await mountPage(TabletsPage, { status });
      await w.vm.revokeDevice(device);
      expect(cmds("revoke_device")).toHaveLength(0);
    });

    it("shows why a revoke failed", async () => {
      defaults();
      handle("revoke_device", () => {
        throw "unknown device";
      });
      const w = await mountPage(TabletsPage, { status });
      await w.vm.revokeDevice(device);
      expect(w.vm.revokeError).toBe("unknown device");
    });

    it.each([
      [0, "never"],
      [-30, "now"],
      [-600, "10 min ago"],
      [-3 * 3600, "3 h ago"],
    ])("labels last-seen offset %d as %s", async (offset, label) => {
      defaults();
      const w = await mountPage(TabletsPage, { status });
      const at = offset === 0 ? 0 : Date.now() / 1000 + offset;
      expect(w.vm.lastSeenLabel(at)).toBe(label);
    });
  });

  describe("adb", () => {
    it("lists USB devices on open, even with no paired tablet", async () => {
      defaults();
      handle("adb_devices", () => ["R52T (device)"]);
      const w = await mountPage(TabletsPage, { status });
      expect(cmds("adb_devices")).toHaveLength(1);
      expect(w.vm.readyAdbDevices).toEqual(["R52T (device)"]);
      expect(w.text()).toContain("R52T (device)");
    });

    it("says when adb is missing instead of failing loudly", async () => {
      defaults();
      handle("adb_devices", () => {
        throw "adb nie znaleziony w PATH";
      });
      const w = await mountPage(TabletsPage, { status });
      expect(w.vm.adbMissing).toContain("adb");
      expect(w.text()).toContain("platform-tools");
      expect(w.vm.adbNoteBad).toBe(false);
    });

    it("installs a picked APK and refreshes the device list", async () => {
      defaults();
      dialog.open.mockResolvedValueOnce("C:/pulpit.apk");
      handle("adb_install_apk", () => "Success");
      const w = await mountPage(TabletsPage, { status });
      handle("adb_devices", () => ["emulator-5554 (device)"]);
      await w.vm.installApk();
      await flushPromises();
      expect(cmds("adb_install_apk")[0].args).toEqual({ path: "C:/pulpit.apk" });
      expect(w.vm.adbNote).toBe("Success");
      expect(w.vm.adbList).toEqual(["emulator-5554 (device)"]);
    });

    it("skips the install when the picker is cancelled", async () => {
      defaults();
      const w = await mountPage(TabletsPage, { status });
      await w.vm.installApk();
      expect(cmds("adb_install_apk")).toHaveLength(0);
    });

    it("updates the tablet from the feed and reports failures", async () => {
      defaults();
      const w = await mountPage(TabletsPage, { status });
      handle("install_tablet_update", () => "R52T: zainstalowano v1.5.2");
      await w.vm.updateTablet();
      expect(w.vm.adbNote).toBe("R52T: zainstalowano v1.5.2");
      expect(w.vm.adbNoteBad).toBe(false);
      handle("install_tablet_update", () => {
        throw "Brak tabletu w adb";
      });
      await w.vm.updateTablet();
      expect(w.vm.adbNote).toBe("Brak tabletu w adb");
      expect(w.vm.adbNoteBad).toBe(true);
    });
  });
});

describe("About page", () => {
  it("checks for updates on open and installs one", async () => {
    defaults();
    handle("check_for_updates", () => ({ update_available: true, latest: "1.5.0", current: "1.4.0" }));
    handle("install_update", () => "Restarting...");
    const w = await mountPage(AboutPage, { status });
    expect(cmds("check_for_updates")).toHaveLength(1);
    expect(w.vm.updateReady).toBe("1.5.0");
    expect(w.text()).toContain("Install 1.5.0");
    await w.vm.installUpdate();
    expect(w.vm.updateNote).toBe("Restarting...");
    // stays busy while the app restarts itself
    expect(w.vm.updateBusy).toBe(true);
  });

  it("says when the app is up to date", async () => {
    defaults();
    const w = await mountPage(AboutPage, { status });
    expect(w.vm.updateReady).toBe("");
    expect(w.vm.updateNote).toBe("You have the latest version.");
  });

  it("re-enables the buttons after a failed update", async () => {
    defaults();
    handle("install_update", () => {
      throw "sha256 mismatch";
    });
    const w = await mountPage(AboutPage, { status });
    await w.vm.installUpdate();
    expect(w.vm.updateNote).toBe("sha256 mismatch");
    expect(w.vm.updateNoteBad).toBe(true);
    expect(w.vm.updateBusy).toBe(false);
  });

  it("opens the data and log folders", async () => {
    defaults();
    const w = await mountPage(AboutPage, { status });
    await w.vm.openFolder(false);
    await w.vm.openFolder(true);
    expect(cmds("open_data_dir").map((c) => c.args)).toEqual([{ logs: false }, { logs: true }]);
  });
});

describe("AI usage page", () => {
  const detected = [
    { id: "glm:5h", label: "GLM 5h" },
    { id: "glm:week", label: "GLM week" },
    { id: "claude:5h", label: "Claude 5h" },
    { id: "mystery:x", label: "Mystery" },
  ];

  it("groups rows by provider and checks everything for an empty selection", async () => {
    defaults();
    handle("aidev_status_config", () => ({ detected, show: [], summary: false, row_style: "logo" }));
    const w = await mountPage(AidevPage);
    expect(w.vm.groups.map((g) => [g.name, g.rows.length])).toEqual([
      ["GLM", 2],
      ["Claude", 1],
      ["mystery", 1],
    ]);
    expect(w.vm.show).toEqual(detected.map((r) => r.id));
    expect(w.vm.summary).toBe(false);
    expect(w.vm.rowStyle).toBe("logo");
  });

  it("keeps only still-detected rows of a saved selection", async () => {
    defaults();
    handle("aidev_status_config", () => ({ detected, show: ["glm:5h", "gone:row"] }));
    const w = await mountPage(AidevPage);
    expect(w.vm.show).toEqual(["glm:5h"]);
  });

  it("saves a partial selection explicitly and a full one as []", async () => {
    defaults();
    handle("aidev_status_config", () => ({ detected, show: [] }));
    const w = await mountPage(AidevPage);
    w.vm.toggleRow("glm:week");
    await flushPromises();
    expect(cmds("set_aidev_status_config").at(-1).args).toEqual({
      show: ["glm:5h", "claude:5h", "mystery:x"],
      summary: true,
      rowStyle: "name",
    });
    w.vm.toggleRow("glm:week");
    await flushPromises();
    expect(cmds("set_aidev_status_config").at(-1).args.show).toEqual([]);
  });

  it("refuses to uncheck the last row", async () => {
    defaults();
    handle("aidev_status_config", () => ({ detected: [detected[0]], show: [] }));
    const w = await mountPage(AidevPage);
    w.vm.toggleRow("glm:5h");
    await flushPromises();
    expect(cmds("set_aidev_status_config")).toHaveLength(0);
    expect(w.vm.note).toContain("At least one");
  });

  it("saves summary and row-style toggles, skipping no-op style changes", async () => {
    defaults();
    const w = await mountPage(AidevPage);
    w.vm.toggleSummary();
    await flushPromises();
    w.vm.setRowStyle("name");
    w.vm.setRowStyle("logo");
    await flushPromises();
    const saves = cmds("set_aidev_status_config").map((c) => c.args);
    expect(saves).toHaveLength(2);
    expect(saves[0]).toMatchObject({ summary: false, rowStyle: "name" });
    expect(saves[1]).toMatchObject({ summary: false, rowStyle: "logo" });
  });

  it("re-reads while nothing is detected yet", async () => {
    vi.useFakeTimers();
    defaults();
    const w = await mountPage(AidevPage);
    handle("aidev_status_config", () => ({ detected, show: [] }));
    await vi.advanceTimersByTimeAsync(5000);
    expect(w.vm.detected).toHaveLength(4);
    w.unmount();
  });
});

describe("Spotify page", () => {
  it("reads as not set up without a client id", async () => {
    defaults();
    const w = await mountPage(SpotifyPage);
    expect(w.vm.loggedIn).toBe(false);
    expect(w.vm.chip.text).toBe("not set up");
    expect(w.vm.helpOpen).toBe(true);
  });

  it("describes a premium login with the live device", async () => {
    defaults();
    handle("spotify_status", () => ({ configured: true, clientId: "cid", loggedIn: true, user: "tymek", product: "premium" }));
    const w = await mountPage(SpotifyPage, { customValues: { "spotify-device": "Desk" } });
    expect(w.vm.clientIdDraft).toBe("cid");
    expect(w.vm.statusLine).toBe("Logged in as tymek (Premium) - device: Desk");
    expect(w.vm.chip).toEqual({ text: "logged in", cls: "ok" });
  });

  it("lets live auth pushes override the fetched status", async () => {
    defaults();
    handle("spotify_status", () => ({ configured: true, clientId: "cid", loggedIn: true, user: "u" }));
    const w = await mountPage(SpotifyPage, { customValues: { "spotify-auth": "needs-login" } });
    expect(w.vm.loggedIn).toBe(false);
    expect(w.vm.chip.text).toBe("login required");
    await w.setProps({ customValues: { "spotify-auth": "ok" } });
    expect(w.vm.loggedIn).toBe(true);
  });

  it("reports a broken spotify.json", async () => {
    defaults();
    handle("spotify_status", () => ({ configured: false }));
    const w = await mountPage(SpotifyPage, { customValues: { "spotify-auth": "ok" } });
    expect(w.vm.loggedIn).toBe(false);
    expect(w.vm.statusLine).toContain("corrupted");
  });

  it("saves a pasted client id before logging in", async () => {
    defaults();
    handle("spotify_login", () => ({ user: "tymek" }));
    const w = await mountPage(SpotifyPage);
    w.vm.clientIdDraft = "  abc  ";
    await w.vm.login();
    expect(cmds("spotify_set_client_id")[0].args).toEqual({ clientId: "abc" });
    expect(calls.findIndex((c) => c.cmd === "spotify_set_client_id")).toBeLessThan(
      calls.findIndex((c) => c.cmd === "spotify_login"),
    );
    expect(w.vm.note).toBe("Logged in as tymek.");
  });

  it("logs out and shows login failures", async () => {
    defaults();
    const w = await mountPage(SpotifyPage);
    await w.vm.logout();
    expect(cmds("spotify_logout")).toHaveLength(1);
    handle("spotify_login", () => {
      throw "login timed out";
    });
    w.vm.clientIdDraft = "";
    await w.vm.login();
    expect(w.vm.error).toBe("login timed out");
  });

  it("copies the redirect URI", async () => {
    defaults();
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const w = await mountPage(SpotifyPage);
    await w.vm.copyRedirectUri();
    expect(writeText).toHaveBeenCalledWith("http://127.0.0.1:8502/spotify/callback");
  });
});

describe("Discord page", () => {
  it("words each backend state in the UI language", async () => {
    handle("discord_status", () => ({ clientId: "1", hasSecret: true, statusType: "not_running" }));
    const w = await mountPage(DiscordPage);
    expect(w.vm.statusLine).toContain("not running");
    handle("discord_status", () => ({ clientId: "1", hasSecret: true, statusType: "error", error: "pipe broke" }));
    await w.vm.recheck();
    expect(w.vm.statusLine).toBe("Error: pipe broke");
    expect(w.vm.chip.cls).toBe("bad");
  });

  it("saves edited credentials before authorizing", async () => {
    handle("discord_status", () => ({ clientId: "", hasSecret: false, statusType: "not_configured" }));
    handle("discord_authorize", () => ({ statusType: "connected", username: "tymek" }));
    const w = await mountPage(DiscordPage);
    w.vm.clientIdDraft = "123";
    w.vm.secretDraft = "s3cret";
    await w.vm.connect();
    expect(cmds("discord_save_config")[0].args).toEqual({ clientId: "123", clientSecret: "s3cret" });
    expect(cmds("discord_authorize")).toHaveLength(1);
    expect(w.vm.connected).toBe(true);
    expect(w.vm.statusLine).toBe("Connected as tymek.");
  });

  it("does not authorize when saving the credentials fails", async () => {
    handle("discord_status", () => ({ clientId: "", statusType: "not_configured" }));
    handle("discord_save_config", () => {
      throw "disk full";
    });
    const w = await mountPage(DiscordPage);
    w.vm.clientIdDraft = "123";
    w.vm.secretDraft = "x";
    await w.vm.connect();
    expect(cmds("discord_authorize")).toHaveLength(0);
    expect(w.vm.error).toBe("disk full");
  });
});

describe("OBS page", () => {
  const obs = { enabled: false, connected: false, authFailed: false, host: "10.0.0.5", port: 4455, hasPassword: true };

  it("applies the switch at once with the stored connection", async () => {
    handle("obs_status", () => obs);
    const w = await mountPage(ObsPage);
    expect(w.vm.hostDraft).toBe("10.0.0.5");
    await w.vm.toggleEnabled();
    expect(cmds("obs_apply_config")[0].args).toEqual({ enabled: true, host: "10.0.0.5", port: 4455, password: null });
    w.unmount();
  });

  it("keeps the stored password unless a new one is typed", async () => {
    handle("obs_status", () => obs);
    const w = await mountPage(ObsPage);
    await w.vm.save();
    expect(cmds("obs_apply_config")[0].args.password).toBeNull();
    w.vm.passwordDraft = "pw";
    await w.vm.save();
    expect(cmds("obs_apply_config")[1].args.password).toBe("pw");
    await w.vm.clearPassword();
    expect(cmds("obs_apply_config")[2].args.password).toBe("");
    w.unmount();
  });

  it("validates the port before saving", async () => {
    handle("obs_status", () => obs);
    const w = await mountPage(ObsPage);
    w.vm.portDraft = "99999";
    await w.vm.save();
    expect(cmds("obs_apply_config")).toHaveLength(0);
    expect(w.vm.error).toContain("65535");
    w.unmount();
  });

  it("names a wrong password", async () => {
    handle("obs_status", () => ({ ...obs, enabled: true, authFailed: true }));
    const w = await mountPage(ObsPage);
    expect(w.vm.chip).toEqual({ text: "wrong password", cls: "bad" });
    w.unmount();
  });
});

describe("Voicemeeter page", () => {
  it("offers to start an installed but closed Voicemeeter", async () => {
    handle("vm_status", () => ({ installed: true, logged_in: false, dll_path: "C:/vm.dll" }));
    const w = await mountPage(VoicemeeterPage);
    expect(w.vm.chip.text).toBe("not running");
    await w.vm.runVoicemeeter();
    expect(cmds("vm_run")[0].args).toEqual({ vmType: 2 });
    w.unmount();
  });

  it("opens the DLL picker when Voicemeeter is not found", async () => {
    handle("vm_status", () => ({ installed: false }));
    const w = await mountPage(VoicemeeterPage);
    expect(w.text()).toContain("Remote API DLL");
    w.unmount();
  });
});
