import { describe, it, expect } from "vitest";
import { hotkeyFromEvent, hotkeyParts } from "../src/hotkey";

const ev = (key, code, mods = {}) => ({ key, code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods });

describe("hotkeyFromEvent", () => {
  it("spells modifiers then the key in the backend's format", () => {
    expect(hotkeyFromEvent(ev("k", "KeyK", { ctrlKey: true, shiftKey: true }))).toBe("CTRL + SHIFT + K");
    expect(hotkeyFromEvent(ev("Tab", "Tab", { altKey: true }))).toBe("ALT + TAB");
    expect(hotkeyFromEvent(ev("d", "KeyD", { metaKey: true }))).toBe("WIN + D");
  });
  it("uses the physical key for letters and digits", () => {
    expect(hotkeyFromEvent(ev("!", "Digit1", { shiftKey: true }))).toBe("SHIFT + 1");
    expect(hotkeyFromEvent(ev("ф", "KeyA"))).toBe("A");
  });
  it("names special keys the way parse_key_name reads them", () => {
    expect(hotkeyFromEvent(ev("F13", "F13"))).toBe("F13");
    expect(hotkeyFromEvent(ev("ArrowLeft", "ArrowLeft", { ctrlKey: true }))).toBe("CTRL + LEFT");
    expect(hotkeyFromEvent(ev(" ", "Space"))).toBe("SPACE");
    expect(hotkeyFromEvent(ev("Escape", "Escape"))).toBe("ESC");
    expect(hotkeyFromEvent(ev("+", "NumpadAdd", { ctrlKey: true }))).toBe("CTRL + PLUS");
  });
  it("waits while only modifiers are down", () => {
    expect(hotkeyFromEvent(ev("Control", "ControlLeft", { ctrlKey: true }))).toBeNull();
    expect(hotkeyFromEvent(ev("Shift", "ShiftLeft", { shiftKey: true }))).toBeNull();
  });
});

describe("hotkeyParts", () => {
  it("splits a stored keystroke into key caps", () => {
    expect(hotkeyParts("ctrl+shift + k")).toEqual(["CTRL", "SHIFT", "K"]);
    expect(hotkeyParts("+")).toEqual(["+"]);
    expect(hotkeyParts("")).toEqual([]);
  });
});
