import { vi, beforeEach } from "vitest";
import { handle, resetTauri } from "./tauri.js";

// happy-dom does not expose `localStorage` as a global on every Node
// version (works on 22, undefined on 26); tests only use it for the
// "recently used" group, so an in-memory Storage is enough.
if (typeof localStorage === "undefined") {
  const store = new Map();
  const storage = {
    getItem: (k) => (store.has(String(k)) ? store.get(String(k)) : null),
    setItem: (k, v) => void store.set(String(k), String(v)),
    removeItem: (k) => void store.delete(String(k)),
    clear: () => store.clear(),
    key: (i) => Array.from(store.keys())[i] ?? null,
    get length() {
      return store.size;
    },
  };
  vi.stubGlobal("localStorage", storage);
}


vi.mock("@tauri-apps/api/core", async () => {
  const t = await import("./tauri.js");
  return { invoke: t.invoke };
});
vi.mock("@tauri-apps/api/event", async () => {
  const t = await import("./tauri.js");
  return { listen: t.listen };
});
vi.mock("@tauri-apps/plugin-dialog", async () => {
  const t = await import("./tauri.js");
  return {
    ask: (...a) => t.dialog.ask(...a),
    open: (...a) => t.dialog.open(...a),
    save: (...a) => t.dialog.save(...a),
  };
});

// The in-app confirm popup replaced Tauri's native `ask`; tests still script
// the answer through `dialog.ask`. The real state/answer exports stay in
// place so ConfirmDialog itself can be tested against the actual module.
vi.mock("../src/confirm.js", async () => {
  const actual = await vi.importActual("../src/confirm.js");
  const t = await import("./tauri.js");
  return { ...actual, ask: (...a) => t.dialog.ask(...a) };
});

beforeEach(() => {
  resetTauri();
  // integration status commands every settings / editor mount polls; the
  // real backend always answers with an object, never null
  handle("obs_status", () => ({
    enabled: false,
    connected: false,
    authFailed: false,
    version: null,
    host: "127.0.0.1",
    port: 4455,
    hasPassword: false,
  }));
  handle("obs_choices", () => ({ scenes: [], sources: [], inputs: [], filters: [] }));
  handle("discord_status", () => ({
    configured: false,
    clientId: "",
    hasSecret: false,
    statusType: "not_configured",
    statusLine: "nie skonfigurowano",
    username: null,
  }));
});
