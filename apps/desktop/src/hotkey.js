// Keystroke strings the backend's parse_hotkey reads ("CTRL + SHIFT + K"):
// key names joined by " + ", case-insensitive, "PLUS" for the plus key.
// The edit dialog records them from a real key press and shows them as
// key caps; both directions live here so they stay in step.

const NAMED = {
  Enter: "ENTER",
  Tab: "TAB",
  Escape: "ESC",
  " ": "SPACE",
  Backspace: "BACKSPACE",
  Delete: "DELETE",
  Insert: "INSERT",
  Home: "HOME",
  End: "END",
  PageUp: "PAGEUP",
  PageDown: "PAGEDOWN",
  ArrowUp: "UP",
  ArrowDown: "DOWN",
  ArrowLeft: "LEFT",
  ArrowRight: "RIGHT",
  "+": "PLUS",
};
const MODIFIERS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph", "OS"]);

// The hotkey a keydown spells, or null while only modifiers are held (the
// recorder waits for the main key). Letters and digits come from the
// physical key (`code`), so Shift+1 records "SHIFT + 1", not "!".
export function hotkeyFromEvent(e) {
  if (MODIFIERS.has(e.key)) return null;
  let key = NAMED[e.key];
  if (!key && /^F([1-9]|1\d|2[0-4])$/.test(e.key)) key = e.key;
  if (!key && /^Key[A-Z]$/.test(e.code || "")) key = e.code.slice(3);
  if (!key && /^Digit\d$/.test(e.code || "")) key = e.code.slice(5);
  if (!key && e.key && e.key.length === 1) key = e.key.toUpperCase();
  if (!key) return null;
  const parts = [];
  if (e.ctrlKey) parts.push("CTRL");
  if (e.shiftKey) parts.push("SHIFT");
  if (e.altKey) parts.push("ALT");
  if (e.metaKey) parts.push("WIN");
  parts.push(key);
  return parts.join(" + ");
}

// key caps for display: "ctrl+shift + k" -> ["CTRL", "SHIFT", "K"]
export function hotkeyParts(value) {
  const raw = String(value || "").trim();
  if (!raw) return [];
  if (raw === "+") return ["+"];
  return raw
    .split("+")
    .map((p) => p.trim().toUpperCase())
    .filter(Boolean);
}
