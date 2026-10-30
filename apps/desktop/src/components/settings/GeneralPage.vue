<script>
const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

/// The global-shortcut string ("Ctrl+Alt+D") for a keydown, or null while
/// only modifiers are held. Keys come from `e.code` (the physical key),
/// so the combo does not depend on the keyboard layout or Shift state;
/// letters and digits drop their `Key`/`Digit` prefix, everything else
/// (F5, Space, ArrowUp, Numpad1) is a name the backend parser accepts.
export function comboFromEvent(e) {
  if (!e.code || MODIFIER_CODES.has(e.code)) return null;
  let key = e.code;
  if (/^Key[A-Z]$/.test(key)) key = key.slice(3);
  else if (/^Digit[0-9]$/.test(key)) key = key.slice(5);
  const mods = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  return [...mods, key].join("+");
}

/// A global shortcut without Ctrl/Alt/Super would swallow ordinary typing
/// in every app; only the function keys may stand alone (or with Shift).
export function comboAllowed(combo) {
  const parts = combo.split("+");
  const key = parts[parts.length - 1];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(key)) return true;
  return parts.some((p) => p === "Ctrl" || p === "Alt" || p === "Super");
}
</script>

<script setup>
import { nextTick, onMounted, ref } from "vue";
import { api } from "../../api";
import { setUiLang, t, uiLang } from "../../i18n";

const autostart = ref(false);
const autostartBusy = ref(false);
const autostartError = ref("");

// applies on the next launch: the window is created hidden (or not)
// once, in the backend's setup
const startMinimized = ref(false);
const startMinimizedBusy = ref(false);
const startMinimizedError = ref("");

const hotkey = ref("Ctrl+Alt+D");
const capturing = ref(false);
const hotkeyBusy = ref(false);
const hotkeyError = ref("");
const captureBtn = ref(null);

const langError = ref("");

async function onLangChange(e) {
  langError.value = "";
  try {
    await setUiLang(e.target.value);
  } catch (err) {
    // setUiLang already reverted the switch; say why it did not stick
    langError.value = err ? String(err) : t("Could not save the language.");
  }
}

async function toggleAutostart() {
  autostartBusy.value = true;
  autostartError.value = "";
  const next = !autostart.value;
  autostart.value = next;
  try {
    await api.setAutostart(next);
  } catch (e) {
    // flip back and say why - a silent revert leaves the user guessing
    autostart.value = !next;
    autostartError.value = e ? String(e) : t("Could not save the autostart setting.");
  } finally {
    autostartBusy.value = false;
  }
}

async function toggleStartMinimized() {
  startMinimizedBusy.value = true;
  startMinimizedError.value = "";
  const next = !startMinimized.value;
  startMinimized.value = next;
  try {
    await api.setStartMinimized(next);
  } catch (e) {
    startMinimized.value = !next;
    startMinimizedError.value = e ? String(e) : t("Could not save the start option.");
  } finally {
    startMinimizedBusy.value = false;
  }
}

async function startCapture() {
  hotkeyError.value = "";
  capturing.value = true;
  await nextTick();
  captureBtn.value?.focus();
}

function stopCapture() {
  capturing.value = false;
}

// keydown on the focused capture button: the key never reaches the
// overlay (Escape here cancels the capture, not the whole Settings)
async function onCaptureKey(e) {
  if (e.key === "Tab") {
    stopCapture();
    return;
  }
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
    stopCapture();
    return;
  }
  const combo = comboFromEvent(e);
  if (!combo) return; // still holding modifiers only
  if (!comboAllowed(combo)) {
    hotkeyError.value = t("Add Ctrl or Alt - a bare key would block typing in every app.");
    return;
  }
  await saveHotkey(combo);
}

async function saveHotkey(combo) {
  hotkeyBusy.value = true;
  hotkeyError.value = "";
  try {
    await api.setHotkey(combo);
    hotkey.value = combo;
    capturing.value = false;
  } catch (e) {
    // the backend says why the combo was refused (taken by another app,
    // unparseable); the previous shortcut stays registered
    hotkeyError.value = e
      ? String(e)
      : t("This shortcut is taken or not allowed - pick another combination.");
  } finally {
    hotkeyBusy.value = false;
  }
}

onMounted(async () => {
  try {
    const settings = await api.getSettings();
    if (settings?.hotkey) hotkey.value = settings.hotkey;
    startMinimized.value = Boolean(settings?.start_minimized);
  } catch {
    /* keep the defaults when the backend is unreachable */
  }
  autostart.value = Boolean(await api.getAutostart().catch(() => false));
});
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2>{{ t("General") }}</h2>
    </header>

    <section class="group">
      <h3 class="group-title">{{ t("Startup") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Run after sign-in") }}</span>
            <span class="row-note">{{ t("Pulpit starts with Windows, so tablets can connect without opening the editor.") }}</span>
          </div>
          <button
            class="switch"
            role="switch"
            :aria-checked="autostart"
            :aria-label="t('Run after sign-in')"
            :disabled="autostartBusy"
            @click="toggleAutostart"
          ><span class="knob"></span></button>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Start minimized") }}</span>
            <span class="row-note">{{ t("Starts hidden in the tray. Applies from the next launch.") }}</span>
          </div>
          <button
            class="switch"
            role="switch"
            :aria-checked="startMinimized"
            :aria-label="t('Start minimized')"
            :disabled="startMinimizedBusy"
            @click="toggleStartMinimized"
          ><span class="knob"></span></button>
        </div>
      </div>
      <p v-if="autostartError" class="err">{{ autostartError }}</p>
      <p v-if="startMinimizedError" class="err">{{ startMinimizedError }}</p>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Editor") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Language") }}</span>
            <span class="row-note">{{ t("The language of this window.") }}</span>
          </div>
          <select class="select" :value="uiLang" :aria-label="t('Language')" @change="onLangChange">
            <option value="auto">{{ t("Auto (system)") }}</option>
            <option value="pl">Polski</option>
            <option value="en">English</option>
          </select>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Touch-mode shortcut") }}</span>
            <span class="row-note">
              {{ capturing
                ? t("Press the new key combination. Esc cancels.")
                : t("Switches the editor into full-screen touch mode from anywhere in Windows.") }}
            </span>
          </div>
          <div class="row-ctl">
            <button
              v-if="capturing"
              ref="captureBtn"
              class="kbd capturing"
              :disabled="hotkeyBusy"
              @keydown="onCaptureKey"
              @blur="stopCapture"
            >{{ t("Press keys…") }}</button>
            <template v-else>
              <kbd class="kbd">{{ hotkey }}</kbd>
              <button class="act" @click="startCapture">{{ t("Change") }}</button>
            </template>
          </div>
        </div>
      </div>
      <p v-if="hotkeyError" class="err">{{ hotkeyError }}</p>
      <p v-if="langError" class="err">{{ langError }}</p>
    </section>
  </div>
</template>
