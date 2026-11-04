<script setup>
import { computed, nextTick, reactive, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { ask } from "../confirm";
import { CATALOG, parsePlanWindows, setPlanWindows } from "../catalog";
import { api, vmDevicesState, refreshVmDevices } from "../api";
import { t } from "../i18n";
import SelectField from "./SelectField.vue";
import ActionPicker from "./ActionPicker.vue";
import { ICON_GROUPS, searchIcons } from "../icons";
import {
  AlignVerticalJustifyCenter,
  ArrowDownToLine,
  ArrowUpToLine,
  ChevronRight,
  Circle,
  Hand,
  Image as ImageIcon,
  Lock,
  MousePointerClick,
  MoveLeft,
  MoveRight,
  Paintbrush,
  SlidersVertical,
  Square,
  Timer,
  ToggleRight,
  Trash2,
  X,
  Zap,
} from "lucide-vue-next";
import { hotkeyFromEvent, hotkeyParts } from "../hotkey";

const props = defineProps({
  button: { type: Object, default: null }, // null = create mode
  create: { type: Object, default: null }, // {x, y, boardId} for create mode
  boards: { type: Array, required: true },
  boardBackground: { type: String, default: "#437072" },
  knownInputs: { type: Array, default: () => [] },
  // active audio endpoints ({id, name}) for the Set Audio Device select
  audioDevices: { type: Array, default: () => [] },
  // Spotify pickers ({name} devices, {uri, name} playlists); empty when
  // not logged in or the list failed - picker fields then fall back to
  // free text
  spotifyDevices: { type: Array, default: () => [] },
  spotifyPlaylists: { type: Array, default: () => [] },
});
const emit = defineEmits(["save", "create", "delete", "close"]);

refreshVmDevices();
const isCreate = computed(() => !props.button);

// editable copy: every column the dialog touches. Shallow spread, not a
// JSON deep clone: button fields are all primitives, and the base64 img
// strings are immutable - spreading them shares instead of duplicating
// multi-MB payloads on every dialog open.
const form = reactive(
  props.button
    ? { ...props.button }
    : {
        id: null,
        board_id: props.create?.boardId ?? null,
        type: "key",
        mode: "button",
        command: "",
        title: "",
        color: "#ef4836",
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
        x: props.create?.x ?? 0,
        y: props.create?.y ?? 0,
        w: 1,
        h: 1,
      }
);

// AI plan tile: which usage windows render, stored in the options column
// as "windows:5h,week" (no token = both). Applies only to ai-plan-limits.
// The token parses through the shared catalog helper (DESK-10) - the
// status tile filters rows with the same function.
const isPlanTile = computed(() => form.type === "ai-plan-limits");
const planWindows = computed(() => parsePlanWindows(form.options));
function setPlanWindow(key, event) {
  form.options = setPlanWindows(form.options, {
    ...planWindows.value,
    [key]: event.target.checked,
  });
}

// SMTC media tiles: the optional target app ("Aplikacja") lives in the
// options column as {"app": "<name substring>"} - the now-playing status
// tile keys its live payload by `command || type`, so the command must
// stay empty. The dropdown lists the live system playback sessions; a
// failed fetch leaves the picker empty (the current session stays the
// default), and clearing it back selects exactly that.
const isMediaTile = computed(() =>
  ["media-now-playing", "media-control", "media-seek"].includes(form.type)
);
const mediaApps = ref([]);
const mediaApp = computed(() => {
  try {
    return JSON.parse(form.options || "{}")?.app || "";
  } catch {
    return "";
  }
});
watch(
  isMediaTile,
  (yes) => {
    if (yes && !mediaApps.value.length) {
      api
        .mediaSessions()
        .then((list) => (mediaApps.value = Array.isArray(list) ? list : []))
        .catch(() => {});
    }
  },
  { immediate: true }
);
function setMediaApp(name) {
  let obj = {};
  try {
    obj = JSON.parse(form.options || "{}") || {};
  } catch {
    obj = {};
  }
  if (name) obj.app = name;
  else delete obj.app;
  form.options = Object.keys(obj).length ? JSON.stringify(obj) : "";
}

// ---- action catalog (static groups + live extension inputs) ----------------

function prettify(value) {
  const s = String(value).replace(/[-_]/g, " ").trim();
  return s.charAt(0).toUpperCase() + s.slice(1);
}

const extInputs = computed(() => {
  const seen = new Set(CATALOG.filter((c) => c.value).map((c) => c.value));
  return props.knownInputs.filter(
    (i) => i.source === "extension" && !seen.has(i.value)
  );
});

// extension field declarations ("input:text" / "input:select" / ...) use the
// same shape as catalog fields so the generic renderer can handle them
function extFieldShape(f) {
  if (f.kind === "input:select") {
    return {
      key: f.key,
      label: f.label,
      kind: "select",
      options: (f.items || []).map((it) => ({ value: it.value, label: it.label })),
    };
  }
  if (f.kind === "input:key") {
    return { key: f.key, label: f.label, placeholder: t("e.g. CTRL + K") };
  }
  if (f.kind === "input:file" || f.kind === "input:folder") {
    return {
      key: f.key,
      label: f.label,
      placeholder: f.kind === "input:file" ? t("file path") : t("folder path"),
    };
  }
  return { key: f.key, label: f.label };
}

const actionGroups = computed(() => {
  const groups = [];
  let current = null;
  for (const entry of CATALOG) {
    if (entry.header) {
      current = { header: entry.header, items: [] };
      groups.push(current);
    } else if (!entry.divider && current) {
      current.items.push(entry);
    }
  }
  if (extInputs.value.length) {
    // one group per extension package, named after the plugin
    const byExt = new Map();
    for (const i of extInputs.value) {
      const name = i.extension || "Extensions";
      if (!byExt.has(name)) byExt.set(name, []);
      byExt.get(name).push({
        value: i.value,
        label: i.label || prettify(i.value),
        icon: i.icon || "puzzle-piece",
        color: i.color || "#7f8c8d",
        extInput: i,
        fields: (i.fields || []).map(extFieldShape),
      });
    }
    for (const [header, items] of byExt) groups.push({ header, items });
  }
  return groups;
});

// catalog fields may declare a dynamic option source; "audio" is the
// audio endpoint list for speaker-device tiles, "spotify"/
// "spotify-playlists" the Spotify device/playlist pickers, "obs-*" the
// live OBS connection (scenes / sources / inputs / filters). A picker
// whose list is empty or failed to load (not logged in, OBS offline,
// API error) falls back to the raw text field, so the URI/name can
// always be typed.
function catalogFieldShape(f) {
  if (f.devices === "audio") {
    return {
      key: f.key,
      label: f.label,
      kind: "select",
      options: props.audioDevices.map((d) => ({ value: d.id, label: d.name })),
    };
  }
  if (f.devices === "spotify" && props.spotifyDevices.length) {
    return {
      key: f.key,
      label: f.label,
      kind: "select",
      options: props.spotifyDevices.map((d) => ({ value: d.name, label: d.name })),
    };
  }
  if (f.devices === "spotify-playlists" && props.spotifyPlaylists.length) {
    return {
      key: f.key,
      label: f.label,
      kind: "select",
      options: props.spotifyPlaylists.map((p) => ({ value: p.uri, label: p.name })),
    };
  }
  if (f.devices === "vm-strip" && vmDevicesState.value.strips.length) {
    return {
      key: f.key,
      label: f.label,
      kind: "select",
      options: vmDevicesState.value.strips,
    };
  }
  if (f.devices === "vm-bus" && vmDevicesState.value.buses.length) {
    return {
      key: f.key,
      label: f.label,
      kind: "select",
      options: vmDevicesState.value.buses,
    };
  }
  if (String(f.devices || "").startsWith("obs-")) {
    const list =
      f.devices === "obs-scenes"
        ? obsChoices.value.scenes
        : f.devices === "obs-sources"
          ? obsChoices.value.sources
          : f.devices === "obs-filters"
            ? obsChoices.value.filters
            : obsChoices.value.inputs;
    if (list.length) {
      return {
        key: f.key,
        label: f.label,
        kind: "select",
        options: list.map((n) => ({ value: n, label: n })),
      };
    }
  }
  return f;
}
// catalog entry for any action type (static catalog or extension group);
// shared by the main form and the per-gesture action overrides
function catalogEntryFor(type) {
  return (
    CATALOG.find((c) => c.value === type) ||
    actionGroups.value
      .flatMap((g) => g.items)
      .find((c) => c.value === type) ||
    null
  );
}
function fieldsShapeFor(type) {
  return (catalogEntryFor(type)?.fields || []).map(catalogFieldShape);
}
const catalogFields = computed(() => fieldsShapeFor(form.type));

const catalogEntry = computed(() => catalogEntryFor(form.type));

// OBS pickers: fetched once per dialog open when the selected action's
// fields ask for them (`devices: "obs-*"`); empty when OBS is offline,
// which leaves the fields as free text.
const obsChoices = ref({ scenes: [], sources: [], inputs: [], filters: [] });
watch(
  () => (catalogEntry.value?.fields || []).some((f) => String(f.devices || "").startsWith("obs-")),
  (need) => {
    if (!need) return;
    api
      .obsChoices()
      .then((c) => (obsChoices.value = c))
      .catch(() => (obsChoices.value = { scenes: [], sources: [], inputs: [], filters: [] }));
  },
  { immediate: true },
);
const showDual = computed(
  () =>
    Boolean(catalogEntry.value?.dual) ||
    form.mode === "toggle" ||
    Boolean(form.color2 || form.icon2 || form.img2)
);
const stepConfig = computed(() => catalogEntry.value?.stepEditor || null);

// ---- command fields --------------------------------------------------------

// field values derived from `command`: key "" is the raw command, other keys
// live inside the JSON object stored in command
const fields = reactive({});
function loadFields() {
  for (const k of Object.keys(fields)) delete fields[k];
  const shaped = catalogFields.value;
  if (!shaped.length) return;
  let obj = {};
  if (shaped.some((f) => f.key !== "")) {
    try {
      obj = JSON.parse(form.command || "{}") || {};
    } catch {
      obj = {};
    }
  }
  for (const f of shaped) {
    fields[f.key] = f.key === "" ? form.command || "" : obj[f.key] ?? "";
  }
  // sensible defaults for selects
  for (const f of shaped) {
    if (f.kind === "select" && fields[f.key] === "" && f.options.length) {
      fields[f.key] = f.options[0].value;
    }
  }
}
function fieldVisible(f) {
  return !f.showIf || fields[f.showIf.key] === f.showIf.value;
}
function applyFields() {
  const shaped = catalogFields.value;
  if (!shaped.length) return;
  if (shaped.every((f) => f.key === "")) {
    form.command = fields[""] ?? "";
    return;
  }
  // merge into the stored command so keys the extension declared outside
  // of its field list survive an edit
  let obj = {};
  try {
    obj = JSON.parse(form.command || "{}") || {};
  } catch {
    obj = {};
  }
  for (const f of shaped) {
    const raw = fields[f.key];
    if (f.kind === "number") {
      if (raw !== "" && raw !== null && !Number.isNaN(Number(raw))) {
        obj[f.key] = Number(raw);
      } else if (fieldVisible(f) && (raw === "" || raw === null)) {
        // a cleared visible field drops its key; skipping it would keep
        // the value pre-seeded from the old command, so the tile kept
        // targeting the old scene/value while the input looked empty
        delete obj[f.key];
      }
    } else if (raw !== "") {
      obj[f.key] = raw;
    } else if (fieldVisible(f)) {
      delete obj[f.key];
    }
  }
  form.command = JSON.stringify(obj);
}
loadFields();

function onTypeChange() {
  const entry = catalogEntry.value;
  // extension inputs carry a preconfigured command template / slider mode
  if (entry?.extInput) {
    if (entry.extInput.command) form.command = entry.extInput.command;
    if (entry.extInput.mode) form.mode = entry.extInput.mode;
    if (entry.extInput.icon && !form.icon) form.icon = entry.extInput.icon;
    if (entry.extInput.color && !form.color) form.color = entry.extInput.color;
  }
  if (entry?.mode) form.mode = entry.mode;
  loadFields();
}

// ActionPicker owns the action <select> replacement; keep the side effects
// of the old native @change in one place, in order
function onActionPicked(value) {
  form.type = value;
  onTypeChange();
  closePop();
}

// board select (type "board": command is {"id": <boardId>})
const boardId = ref(parseBoardId(form.command));
function parseBoardId(command) {
  try {
    return JSON.parse(command || "{}").id ?? "";
  } catch {
    return "";
  }
}
function applyBoardId() {
  form.command = boardId.value === "" ? "" : JSON.stringify({ id: Number(boardId.value) });
}

// select-style command (vol): command is the raw value
function applySelect(event) {
  form.command = event.target.value;
}

// ---- step editor (multiaction / advance-key) -------------------------------

const steps = ref(parseSteps(form.command));
function parseSteps(command) {
  try {
    const arr = JSON.parse(command || "[]");
    return Array.isArray(arr) ? arr.map((s) => ({ ...s })) : [];
  } catch {
    return [];
  }
}
function applySteps() {
  const cfg = stepConfig.value;
  const numberTypes = new Set(
    (cfg?.types || []).filter((t) => t.number).map((t) => t.value)
  );
  form.command = JSON.stringify(
    steps.value
      .filter((s) => s.type)
      .map((s) => ({
        type: s.type,
        command:
          numberTypes.has(s.type) && s.command !== "" && s.command !== null
            ? Number(s.command)
            : s.command,
      }))
  );
}
function addStep() {
  steps.value.push({ ...(stepConfig.value?.addDefaults || { type: "delay", command: "100" }) });
}
function removeStep(i) {
  steps.value.splice(i, 1);
}
function stepBoardChange(step, event) {
  step.command =
    event.target.value === "" ? "" : JSON.stringify({ id: Number(event.target.value) });
}
function stepBoardId(step) {
  try {
    return JSON.parse(step.command || "{}").id ?? "";
  } catch {
    return "";
  }
}
function stepTypeMeta(type) {
  return stepConfig.value?.types.find((t) => t.value === type) || null;
}

// ---- gesture editor ("Gesty") ----------------------------------------------

// Custom gestures live in the options column as JSON, the same shape the
// server and the tablet client already read:
//   {"gestures": ["long-press"], "gesture_actions": {"long-press": {...}}}
// A declared gesture without a `gesture_actions` entry fires the tile's own
// action; an entry ({type, command}) replaces it per gesture. Only
// button/toggle tiles offer the section - sliders keep their drag surface
// and display modes have nothing to trigger.
const GESTURE_DEFS = [
  { key: "long-press", label: "Hold" },
  { key: "double-tap", label: "Double tap" },
  { key: "swipe-left", label: "Swipe left" },
  { key: "swipe-right", label: "Swipe right" },
];

const gestures = reactive({});
function loadGestures() {
  for (const k of Object.keys(gestures)) delete gestures[k];
  let obj = null;
  try {
    const parsed = JSON.parse(form.options || "");
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) obj = parsed;
  } catch {}
  const declared = new Set(
    Array.isArray(obj?.gestures) ? obj.gestures.filter((g) => typeof g === "string") : []
  );
  const actions =
    obj?.gesture_actions && typeof obj.gesture_actions === "object"
      ? obj.gesture_actions
      : {};
  for (const def of GESTURE_DEFS) {
    const action = actions[def.key];
    const g = {
      enabled: declared.has(def.key) || Boolean(action),
      type: action && typeof action === "object" ? String(action.type || "") : "",
      command: action && typeof action === "object" ? String(action.command ?? "") : "",
      fields: {},
    };
    gestures[def.key] = g;
    loadGestureFields(def.key);
  }
}

// same field machinery as the main form, scoped to one gesture's picked
// action: fields load from the stored command and merge back into it
function loadGestureFields(key) {
  const g = gestures[key];
  for (const k of Object.keys(g.fields)) delete g.fields[k];
  const shaped = fieldsShapeFor(g.type);
  if (!shaped.length) return;
  let obj = {};
  if (shaped.some((f) => f.key !== "")) {
    try {
      obj = JSON.parse(g.command || "{}") || {};
    } catch {
      obj = {};
    }
  }
  for (const f of shaped) {
    g.fields[f.key] = f.key === "" ? g.command || "" : obj[f.key] ?? "";
  }
  for (const f of shaped) {
    if (f.kind === "select" && g.fields[f.key] === "" && f.options.length) {
      g.fields[f.key] = f.options[0].value;
    }
  }
}

function gestureFieldVisible(g, f) {
  return !f.showIf || g.fields[f.showIf.key] === f.showIf.value;
}

function applyGestureFields(key) {
  const g = gestures[key];
  const shaped = fieldsShapeFor(g.type);
  if (!shaped.length) return;
  if (shaped.every((f) => f.key === "")) {
    g.command = g.fields[""] ?? "";
    return;
  }
  let obj = {};
  try {
    obj = JSON.parse(g.command || "{}") || {};
  } catch {
    obj = {};
  }
  for (const f of shaped) {
    const raw = g.fields[f.key];
    if (f.kind === "number") {
      if (raw !== "" && raw !== null && !Number.isNaN(Number(raw))) {
        obj[f.key] = Number(raw);
      } else if (gestureFieldVisible(g, f) && (raw === "" || raw === null)) {
        delete obj[f.key];
      }
    } else if (raw !== "") {
      obj[f.key] = raw;
    } else if (gestureFieldVisible(g, f)) {
      delete obj[f.key];
    }
  }
  g.command = JSON.stringify(obj);
}

function onGestureTypePicked(key, value) {
  const g = gestures[key];
  g.type = value;
  if (!value) g.command = "";
  loadGestureFields(key);
}

// picker groups for a gesture's action override: a leading entry for "run
// the tile's own action" (value "") on top of the shared action catalog
const gesturePickerGroups = computed(() => [
  {
    header: t("Gesture"),
    items: [
      { value: "", label: t("Tile action (default)"), icon: "hand-pointer", color: "#4a6a8a" },
    ],
  },
  ...actionGroups.value,
]);

function gestureFieldList(key) {
  return fieldsShapeFor(gestures[key].type);
}

// the tile modes gestures make sense for; slider/knob keep the drag
// surface for the value and display modes have nothing to trigger
// tool tiles (clock/timer/stopwatch/counter) own their taps and holds,
// so custom gestures would fight them
const gesturesAvailable = computed(
  () => ["button", "toggle"].includes(modeEff.value) && !String(form.type || "").startsWith("tool-")
);
const GESTURE_ICONS = {
  "long-press": Timer,
  "double-tap": MousePointerClick,
  "swipe-left": MoveLeft,
  "swipe-right": MoveRight,
};
// one line under an enabled gesture: which action it fires
function gestureSummary(key) {
  const type = gestures[key]?.type;
  if (!type) return t("Tile action (default)");
  const label = catalogEntryFor(type)?.label;
  return label ? t(label) : type;
}

const gesturesActive = computed(() =>
  GESTURE_DEFS.some((def) => gestures[def.key]?.enabled)
);

// gestures ride the options JSON object, so the field cannot simultaneously
// be the plain-string program arguments of the Run Program tile - an
// inline conflict instead of a silent data loss on save
const gestureError = computed(() => {
  if (!gesturesActive.value) return "";
  const raw = String(form.options || "").trim();
  if (!raw) return "";
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) return "";
  } catch {}
  return t('The "Options" field already contains program arguments - gestures require JSON and cannot share this field.');
});

function applyGestures() {
  const enabled = GESTURE_DEFS.filter((def) => gestures[def.key].enabled);
  const raw = String(form.options || "").trim();
  if (!enabled.length) {
    // nothing declared: strip the gesture keys from an options JSON that
    // carries them (a previous configuration), leave any other options
    // byte-identical
    if (!raw) return true;
    try {
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return true;
      if (!("gestures" in parsed) && !("gesture_actions" in parsed)) return true;
      delete parsed.gestures;
      delete parsed.gesture_actions;
      form.options = JSON.stringify(parsed);
    } catch {}
    return true;
  }
  if (gestureError.value) return false;
  let obj = {};
  if (raw) {
    try {
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        return false;
      }
      obj = parsed;
    } catch {
      return false;
    }
  }
  const list = [];
  const actions = {};
  for (const def of enabled) {
    applyGestureFields(def.key);
    list.push(def.key);
    const g = gestures[def.key];
    if (g.type) actions[def.key] = { type: g.type, command: g.command ?? "" };
  }
  obj.gestures = list;
  if (Object.keys(actions).length) obj.gesture_actions = actions;
  else delete obj.gesture_actions;
  form.options = JSON.stringify(obj);
  return true;
}
loadGestures();

// ---- slider default ("double-tap reset") ------------------------------------

// slider/knob tiles may carry a reset value in the options JSON:
//   {"slider_default": 0.5}
// Double-tapping the fader snaps it back there (desktop touch mode and
// tablets alike; the server declares the gesture from the same field).
// Edited as a percent, stored as a 0..1 fraction.
const sliderDefault = ref("");
const sliderDefaultAvailable = computed(() =>
  ["slider", "knob"].includes(form.mode || "button")
);

function loadSliderDefault() {
  sliderDefault.value = "";
  try {
    const parsed = JSON.parse(form.options || "");
    const v =
      parsed && typeof parsed === "object" && !Array.isArray(parsed)
        ? Number(parsed.slider_default)
        : NaN;
    if (Number.isFinite(v) && v >= 0 && v <= 1) {
      sliderDefault.value = String(Math.round(v * 10000) / 100);
    }
  } catch {}
}

// a typed value must parse into 0..100, and the stored options must be a
// JSON object (or empty) - inline conflicts instead of a silent overwrite
// on save, mirroring the gesture editor's rules above
const sliderDefaultError = computed(() => {
  if (!sliderDefaultAvailable.value) return "";
  const raw = String(sliderDefault.value).trim();
  if (!raw) return "";
  const v = Number(raw.replace(",", "."));
  if (!Number.isFinite(v) || v < 0 || v > 100) {
    return t("Default value must be a number 0-100.");
  }
  const stored = String(form.options || "").trim();
  if (!stored) return "";
  try {
    const parsed = JSON.parse(stored);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) return "";
  } catch {}
  return t('The "Options" field already contains non-JSON content - the default value requires JSON.');
});

function applySliderDefault() {
  if (!sliderDefaultAvailable.value) return true;
  if (sliderDefaultError.value) return false;
  const raw = String(sliderDefault.value).trim();
  const stored = String(form.options || "").trim();
  if (!raw) {
    // emptied: strip the key from an options JSON that carries it (a
    // previous configuration), leave any other options byte-identical
    if (!stored) return true;
    try {
      const parsed = JSON.parse(stored);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return true;
      if (!("slider_default" in parsed)) return true;
      delete parsed.slider_default;
      form.options = JSON.stringify(parsed);
    } catch {}
    return true;
  }
  let obj = {};
  if (stored) {
    try {
      const parsed = JSON.parse(stored);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        return false;
      }
      obj = parsed;
    } catch {
      return false;
    }
  }
  const v = Number(raw.replace(",", "."));
  obj.slider_default = Math.round((v / 100) * 10000) / 10000;
  form.options = JSON.stringify(obj);
  return true;
}
loadSliderDefault();

// ---- image -----------------------------------------------------------------

const imageError = ref("");
// img/img2 never enter the dirty snapshot below (they are multi-MB base64
// strings); pickImage is their only writer, so it flips this flag instead
const imagesDirty = ref(false);
async function pickImage(field) {
  const path = await open({
    multiple: false,
    filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp", "gif", "svg"] }],
  });
  if (!path) return;
  try {
    const data = await api.readImageData(path);
    if (data !== form[field]) imagesDirty.value = true;
    form[field] = data;
  } catch (e) {
    imageError.value = String(e);
  }
}

// ---- save / delete ---------------------------------------------------------

// the delete confirmation lives in App.vue's tileDeleted (012 A9), the
// single choke point for both this dialog and the context menu
function removeTile() {
  emit("delete");
}

// ---- dirty guard -----------------------------------------------------------

// Overlay click only closes the dialog when nothing was edited (012 A9):
// an accidental click outside must not discard a configured tile. The
// snapshot covers every writer: the form copy plus the command-mapping
// reactives (fields, steps, boardId) that merge into form.command and the
// gesture states that merge into form.options on save.
// img/img2 are excluded - stringifying up to ~20 MB of base64 on every
// keystroke stutters the dialog (DESK-04); those fields only change
// through pickImage, which flips imagesDirty above.
function formSnapshot() {
  const plain = { ...form };
  delete plain.img;
  delete plain.img2;
  return JSON.stringify({
    form: plain,
    fields: { ...fields },
    steps: steps.value,
    boardId: boardId.value,
    gestures: JSON.parse(JSON.stringify(gestures)),
    sliderDefault: sliderDefault.value,
  });
}
const initialSnapshot = formSnapshot();
const dirty = computed(() => formSnapshot() !== initialSnapshot || imagesDirty.value);

async function requestClose() {
  if (!dirty.value) {
    emit("close");
    return;
  }
  const ok = await ask(t("Close the window and discard unsaved changes?"), {
    title: t("Unsaved changes"),
    okLabel: t("Discard changes"),
    cancelLabel: t("Go back"),
    kind: "warning",
  });
  if (ok) emit("close");
}

function overlayClose() {
  requestClose();
}

function save() {
  // each writer owns the command only for its own type - the others must
  // leave the stored command untouched
  applyFields();
  if (form.type === "board") applyBoardId();
  if (stepConfig.value) applySteps();
  if (!applyGestures()) return; // inline gesture conflict (see gestureError)
  if (!applySliderDefault()) return; // inline slider-default conflict
  emit(isCreate.value ? "create" : "save", { ...form, ...resolveTypeMeta() });
}

// tiles added from outside the catalog keep their stored icon/color
function resolveTypeMeta() {
  const entry = catalogEntry.value;
  if (!entry) return {};
  const out = {};
  if (entry.mode) out.mode = entry.mode;
  if (entry.extInput?.command && !form.command) out.command = entry.extInput.command;
  if (entry.init && !form.command) out.command = entry.init;
  return out;
}

// ---- appearance column -------------------------------------------------------

// the mode that will be saved: fixed-mode catalog entries win (save()
// forces them), free actions use the form's choice
const modeEff = computed(() => catalogEntry.value?.mode || form.mode || "button");

// which toggle state the look controls edit: 1 = base, 2 = second state.
// Only color/icon/img have a second-state twin (color2/icon2/img2); the
// label, shape, text and other colors are shared by both states.
const editState = ref(1);
watch(
  () => showDual.value,
  (dual) => {
    if (!dual) editState.value = 1;
  }
);
function stateKey(base) {
  return editState.value === 2 ? `${base}2` : base;
}
function stateName(s) {
  return s === 2 ? t("State on") : t("State off");
}
const previewStates = computed(() => (showDual.value ? [1, 2] : [1]));
// a state's face the way TileCell draws it: second-state fields fall back
// to the base ones
function face(s) {
  const two = s === 2;
  return {
    color: (two && form.color2) || effColor.value,
    icon: (two && form.icon2) || effIcon.value,
    img: (two && form.img2) || form.img,
  };
}
const boardName = computed(() => props.boards.find((b) => b.id === form.board_id)?.name || "");

const colorRows = computed(() =>
  editState.value === 2
    ? [{ key: "color2", title: "Background 2", short: "Background", fallback: face(1).color }]
    : [
        { key: "color", title: "Background Color", short: "Background", fallback: effColor.value },
        { key: "icon_color", title: "Icon Color", short: "Icon" },
        { key: "title_color", title: "Text Color", short: "Text" },
        { key: "border_color", title: "Border Color", short: "Border" },
        { key: "title_box_color", title: "Text Box Color", short: "Text box" },
      ]
);

// one open popover at a time (icon picker or a color editor)
const openPop = ref(null);
function togglePop(key) {
  openPop.value = openPop.value === key ? null : key;
}
function closePop() {
  openPop.value = null;
}
watch(editState, closePop);

function clearImage(field) {
  if (form[field]) imagesDirty.value = true;
  form[field] = "";
}

// ---- keystroke recorder -------------------------------------------------------

const recording = ref(null); // field key listening for a shortcut
const editingHotkey = ref(null); // field key shown as a text input
const hotkeyInputs = reactive({});
async function focusHotkey(key) {
  await nextTick();
  hotkeyInputs[key]?.focus();
}
function toggleRecord(key) {
  if (recording.value === key) {
    recording.value = null;
    return;
  }
  recording.value = key;
  focusHotkey(key);
}
function editHotkey(key) {
  editingHotkey.value = key;
  focusHotkey(key);
}
function onHotkeyKeydown(event, key) {
  if (recording.value !== key) {
    if (event.key === "Enter") event.preventDefault();
    return;
  }
  // the shortcut is for the target app, not for this dialog
  event.preventDefault();
  event.stopPropagation();
  const combo = hotkeyFromEvent(event);
  if (!combo) return; // modifiers only so far
  fields[key] = combo;
  recording.value = null;
  editingHotkey.value = null;
}
function stopHotkey(key) {
  if (recording.value === key) recording.value = null;
  if (editingHotkey.value === key) editingHotkey.value = null;
}

// searchable picker: curated groups by default, the full shared FA set
// (desktop webfont ∩ Android font) once the user types a query
const iconQuery = ref("");
const iconMatches = computed(() => searchIcons(iconQuery.value));


// preview falls back to the action's own icon/color like the original
const effIcon = computed(() => form.icon || catalogEntry.value?.icon || "");
const effColor = computed(() => form.color || catalogEntry.value?.color || "#2c3e50");
const prevShape = computed(() => (form.shape === 1 ? "50%" : "8px"));
const titlePos = computed(() => form.title_position ?? 0);
function colorOr(val, fallback) {
  return val || fallback;
}
</script>

<template>
  <div class="overlay" @click.self="overlayClose">
    <div class="modal btn-modal">
      <div class="modal-head">
        <span class="head-title">{{ isCreate ? t("New Button") : t("Edit Button") }}</span>
        <span class="crumb tnum">
          <template v-if="boardName">{{ boardName }}<i></i></template>
          {{ form.w || 1 }}×{{ form.h || 1 }}<i></i>{{ t("column {x}, row {y}", { x: (form.x ?? 0) + 1, y: (form.y ?? 0) + 1 }) }}
        </span>
        <button class="head-close" :title="t('Cancel')" @click="requestClose">
          <X :size="18" />
        </button>
      </div>

      <div class="btn-body">
        <!-- left: how the tile looks -->
        <div class="left-col">
          <div class="sec"><Paintbrush :size="14" /> {{ t("Appearance") }}</div>

          <div class="preview" :style="{ '--tint': boardBackground }">
            <button
              v-for="s in previewStates"
              :key="s"
              type="button"
              class="prev-tile"
              :class="{ ghost: showDual && s !== editState, cur: showDual && s === editState }"
              :style="{
                background: face(s).color,
                borderColor: form.border_color || 'transparent',
                borderRadius: prevShape,
              }"
              :title="showDual ? stateName(s) : undefined"
              :tabindex="showDual ? 0 : -1"
              @click="showDual && (editState = s)"
            >
              <img v-if="face(s).img" class="prev-img" :src="face(s).img" alt="" />
              <i
                v-if="face(s).icon"
                class="fas"
                :class="'fa-' + face(s).icon"
                :style="{ color: colorOr(form.icon_color, '#ffffff') }"
              ></i>
              <span
                v-if="form.title"
                class="prev-title"
                :class="`pos-${titlePos}`"
                :style="{
                  color: colorOr(form.title_color, '#ffffff'),
                  background: form.title_box_color || 'transparent',
                }"
              >{{ form.title }}</span>
            </button>
          </div>

          <template v-if="showDual">
            <div class="lbl">{{ t("State you are editing") }}</div>
            <div class="seg">
              <button
                v-for="s in [1, 2]"
                :key="s"
                type="button"
                :class="{ on: editState === s }"
                @click="editState = s"
              >
                <span class="dot" :style="{ background: face(s).color }"></span>{{ stateName(s) }}
              </button>
            </div>
          </template>

          <!-- shared look (both states) -->
          <template v-if="editState === 1">
            <label class="lbl" for="tile-label">{{ t("Label") }}</label>
            <input id="tile-label" v-model="form.title" :placeholder="t('Button label')" @keydown.enter.prevent />
          </template>

          <div class="lbl">{{ t("Icon or image") }}</div>
          <div class="icon-row">
            <div class="pop-anchor grow">
              <button type="button" class="ibtn icon-btn" @click="togglePop('icon')">
                <span class="glyph"><i v-if="face(editState).icon" class="fas" :class="'fa-' + face(editState).icon"></i></span>
                <span class="ival">{{ form[stateKey('icon')] || (editState === 1 ? effIcon || t("N/A") : t("Same as off")) }}</span>
                <ChevronRight :size="14" class="chev" />
              </button>
              <Transition name="pop">
                <div v-if="openPop === 'icon'" class="prop-pop icon-pop">
                  <div class="icon-edit">
                    <input v-model="form[stateKey('icon')]" :placeholder="t('fontawesome 5 icon name')" @keydown.enter.prevent />
                    <i v-if="form[stateKey('icon')]" class="fas big" :class="'fa-' + form[stateKey('icon')]"></i>
                  </div>
                  <input
                    v-model="iconQuery"
                    class="icon-search"
                    type="search"
                    :placeholder="t('Search icons (e.g. mic, arrow, chart)')"
                    @keydown.enter.prevent
                  />
                  <div class="icon-grid">
                    <template v-if="iconQuery.trim()">
                      <p v-if="!iconMatches.length" class="icon-empty">{{ t('No icons match "{q}"', { q: iconQuery.trim() }) }}</p>
                      <button
                        v-for="ic in iconMatches"
                        :key="ic"
                        class="icon-cell"
                        :class="{ on: form[stateKey('icon')] === ic }"
                        :title="ic"
                        @click="form[stateKey('icon')] = ic"
                      >
                        <i class="fas" :class="'fa-' + ic"></i>
                      </button>
                    </template>
                    <template v-else v-for="group in ICON_GROUPS" :key="group.label">
                      <div class="icon-group">{{ group.label }}</div>
                      <button
                        v-for="ic in group.icons"
                        :key="ic"
                        class="icon-cell"
                        :class="{ on: form[stateKey('icon')] === ic }"
                        :title="ic"
                        @click="form[stateKey('icon')] = ic"
                      >
                        <i class="fas" :class="'fa-' + ic"></i>
                      </button>
                    </template>
                  </div>
                </div>
              </Transition>
            </div>
            <button type="button" class="ibtn img-btn" @click="pickImage(stateKey('img'))">
              <ImageIcon :size="15" />
              {{ form[stateKey("img")] ? t("Replace") : t("Image") }}
            </button>
            <button
              v-if="form[stateKey('img')]"
              type="button"
              class="ibtn img-clear"
              :title="t('Remove image')"
              @click="clearImage(stateKey('img'))"
            >
              <X :size="14" />
            </button>
          </div>
          <div v-if="imageError" class="dual-error">{{ imageError }}</div>

          <template v-if="editState === 1">
            <div class="two">
              <div>
                <div class="lbl">{{ t("Shape") }}</div>
                <div class="seg icons">
                  <button type="button" :class="{ on: form.shape !== 1 }" :title="t('Square')" @click="form.shape = 0">
                    <Square :size="15" /><span class="sr">{{ t("Square") }}</span>
                  </button>
                  <button type="button" :class="{ on: form.shape === 1 }" :title="t('Round')" @click="form.shape = 1">
                    <Circle :size="15" /><span class="sr">{{ t("Round") }}</span>
                  </button>
                </div>
              </div>
              <div>
                <div class="lbl">{{ t("Text Position") }}</div>
                <div class="seg icons">
                  <button type="button" :class="{ on: titlePos === 2 }" :title="t('Top')" @click="form.title_position = 2">
                    <ArrowUpToLine :size="15" /><span class="sr">{{ t("Top") }}</span>
                  </button>
                  <button type="button" :class="{ on: titlePos === 1 }" :title="t('Center')" @click="form.title_position = 1">
                    <AlignVerticalJustifyCenter :size="15" /><span class="sr">{{ t("Center") }}</span>
                  </button>
                  <button type="button" :class="{ on: titlePos === 0 }" :title="t('Bottom')" @click="form.title_position = 0">
                    <ArrowDownToLine :size="15" /><span class="sr">{{ t("Bottom") }}</span>
                  </button>
                </div>
              </div>
            </div>
          </template>

          <div class="lbl">{{ t("Colors") }}</div>
          <div class="pop-anchor">
            <div class="swatches" :class="{ single: editState === 2 }">
              <button
                v-for="c in colorRows"
                :key="c.key"
                type="button"
                class="swatch-btn"
                :class="{ on: openPop === c.key }"
                :title="t(c.title)"
                @click="togglePop(c.key)"
              >
                <span
                  class="sw"
                  :class="{ none: !form[c.key] && !c.fallback }"
                  :style="{ background: form[c.key] || c.fallback || undefined }"
                ></span>
                <span class="swl">{{ t(c.short) }}</span>
              </button>
            </div>
            <Transition name="pop">
              <div v-if="colorRows.some((c) => c.key === openPop)" class="prop-pop color-pop">
                <div class="pop-title">{{ t(colorRows.find((c) => c.key === openPop).title) }}</div>
                <div class="color-edit">
                  <input
                    type="color"
                    class="swatch"
                    :value="form[openPop] || '#000000'"
                    @input="form[openPop] = $event.target.value"
                  />
                  <input
                    class="hex"
                    :value="form[openPop]"
                    placeholder="#rrggbb"
                    @input="form[openPop] = $event.target.value"
                    @keydown.enter.prevent
                  />
                  <button class="mini" @click="form[openPop] = ''; closePop()">{{ t("None") }}</button>
                </div>
              </div>
            </Transition>
          </div>

          <p v-if="editState === 2" class="help">
            {{ t("Label, shape, text and the other colors are shared by both states.") }}
          </p>
        </div>

        <!-- right: what the tile does -->
        <div class="right-col">
          <div class="sec"><Zap :size="14" /> {{ t("Behavior") }}</div>

          <div class="lbl first">{{ t("Tile Mode") }}</div>
          <div v-if="catalogEntry?.mode" class="mode-locked">
            <Lock :size="14" />
            {{ t(prettify(catalogEntry.mode)) }}
            <span>{{ t("set by the action") }}</span>
          </div>
          <div v-else class="seg accent mode-seg">
            <button type="button" :class="{ on: modeEff === 'button' }" @click="form.mode = 'button'">
              <MousePointerClick :size="15" />{{ t("Button") }}
            </button>
            <button type="button" :class="{ on: modeEff === 'toggle' }" @click="form.mode = 'toggle'">
              <ToggleRight :size="15" />{{ t("Toggle") }}
            </button>
            <button type="button" :class="{ on: modeEff === 'slider' }" @click="form.mode = 'slider'">
              <SlidersVertical :size="15" />{{ t("Slider") }}
            </button>
          </div>
          <p v-if="modeEff === 'toggle' && !catalogEntry?.mode" class="help">
            {{ t("A toggle remembers its state; each state has its own look (on the left).") }}
          </p>

          <div class="field sel-field">
            <span class="sel-label">{{ t("Action") }}</span>
            <ActionPicker
              :model-value="form.type"
              :groups="actionGroups"
              @update:model-value="onActionPicked"
            />
          </div>

          <!-- board picker -->
          <label v-if="form.type === 'board'" class="field">
            {{ t("Switch to") }}
            <select v-model="boardId">
              <option value="">{{ t("- pick board -") }}</option>
              <option v-for="b in boards.filter((b) => b.id !== form.board_id)" :key="b.id" :value="b.id">
                {{ b.name }}
              </option>
            </select>
          </label>

          <!-- select-style command (vol) -->
          <label v-else-if="catalogEntry?.select" class="field">
            {{ t(catalogEntry.label) }}
            <select :value="form.command || catalogEntry.init" @change="applySelect">
              <option v-for="o in catalogEntry.select" :key="o.value" :value="o.value">
                {{ t(o.label) }}
              </option>
            </select>
          </label>

          <!-- step editor (multiaction / advance keyboard macro) -->
          <template v-else-if="stepConfig">
            <div class="steps">
              <div v-for="(step, i) in steps" :key="i" class="step">
                <select v-model="step.type" class="step-type">
                  <option v-for="st in stepConfig.types" :key="st.value" :value="st.value">
                    {{ t(st.label) }}
                  </option>
                </select>
                <select
                  v-if="stepTypeMeta(step.type)?.board"
                  :value="stepBoardId(step)"
                  @change="stepBoardChange(step, $event)"
                >
                  <option value="">{{ t("- board -") }}</option>
                  <option v-for="b in boards" :key="b.id" :value="b.id">{{ b.name }}</option>
                </select>
                <input
                  v-else
                  v-model="step.command"
                  :inputmode="stepTypeMeta(step.type)?.number ? 'numeric' : undefined"
                  :placeholder="stepTypeMeta(step.type)?.number ? t('milliseconds') : t('value')"
                />
                <button class="step-del" :title="t('Remove step')" @click="removeStep(i)">
                  <i class="fas fa-times"></i>
                </button>
              </div>
              <button class="mini add-step" @click="addStep"><i class="fas fa-plus"></i> {{ t("Add step") }}</button>
            </div>
          </template>

          <!-- structured / generic fields (catalog + extension-declared) -->
          <template v-else-if="catalogFields.length">
            <template v-for="f in catalogFields.filter(fieldVisible)" :key="f.key">
              <!-- select leaves the <label>: clicking a label would forward
                   the click to the first chip / trigger inside -->
              <div v-if="f.kind === 'select'" class="field sel-field">
                <span class="sel-label">{{ t(f.label) }}</span>
                <SelectField v-model="fields[f.key]" :options="f.options" :label="t(f.label)" />
              </div>
              <!-- keystroke: key caps + a recorder that listens for the
                   real shortcut; the text stays editable by hand -->
              <div v-else-if="f.label === 'Keystroke'" class="field sel-field hotkey-field">
                <span class="sel-label">{{ t(f.label) }}</span>
                <div class="hotkey" :class="{ rec: recording === f.key }">
                  <!-- the input always exists (it holds the value); the key
                       caps cover it until it is clicked or recording -->
                  <button
                    v-if="recording !== f.key && editingHotkey !== f.key && hotkeyParts(fields[f.key]).length"
                    type="button"
                    class="caps"
                    :title="t('Edit as text')"
                    @click="editHotkey(f.key)"
                  >
                    <template v-for="(p, i) in hotkeyParts(fields[f.key])" :key="i">
                      <span v-if="i" class="plus">+</span><kbd>{{ p }}</kbd>
                    </template>
                  </button>
                  <input
                    :ref="(el) => (hotkeyInputs[f.key] = el)"
                    v-model="fields[f.key]"
                    :placeholder="recording === f.key ? t('Press the shortcut…') : f.placeholder"
                    :readonly="recording === f.key"
                    @keydown="onHotkeyKeydown($event, f.key)"
                    @blur="stopHotkey(f.key)"
                  />
                  <button type="button" class="rec-btn" @mousedown.prevent @click="toggleRecord(f.key)">
                    <span class="rec-dot"></span>{{ recording === f.key ? t("Listening…") : t("Record") }}
                  </button>
                </div>
              </div>
              <label v-else class="field">
                {{ t(f.label) }}
                <textarea
                  v-if="f.kind === 'textarea'"
                  v-model="fields[f.key]"
                  rows="3"
                  :placeholder="f.placeholder"
                ></textarea>
                <input
                  v-else
                  v-model="fields[f.key]"
                  :inputmode="f.kind === 'number' ? 'numeric' : undefined"
                  :placeholder="f.placeholder"
                  @keydown.enter.prevent
                />
              </label>
            </template>
          </template>

          <!-- extension action without declared options: command is fixed -->
          <div v-else-if="catalogEntry?.extInput" class="no-opts">
            {{ t("This action has no options.") }}
          </div>

          <!-- unknown / custom type -->
          <label v-else class="field">
            {{ t("Command") }}
            <input v-model="form.command" :placeholder="t('command')" @keydown.enter.prevent />
          </label>

          <template v-if="isMediaTile">
            <div class="field sel-field">
              <span class="sel-label">{{ t("App (optional)") }}</span>
              <SelectField
                :model-value="mediaApp"
                :options="[
                  { value: '', label: t('- current session -') },
                  ...mediaApps.map((a) => ({ value: a, label: a })),
                ]"
                :label="t('App')"
                @update:model-value="setMediaApp"
              />
            </div>
          </template>

          <template v-if="isPlanTile">
            <div class="field">
              {{ t("Usage windows") }}
              <div class="win-row">
                <label class="win-check">
                  <input
                    type="checkbox"
                    :checked="planWindows.five"
                    @change="setPlanWindow('five', $event)"
                  />
                  {{ t("5-hour") }}
                </label>
                <label class="win-check">
                  <input
                    type="checkbox"
                    :checked="planWindows.week"
                    @change="setPlanWindow('week', $event)"
                  />
                  {{ t("Weekly") }}
                </label>
              </div>
            </div>
          </template>

          <label v-if="catalogEntry?.options" class="field">
            {{ t("Options (program arguments)") }}
            <input v-model="form.options" :placeholder="t('--flag')" @keydown.enter.prevent />
          </label>

          <!-- slider/knob: the double-tap reset value -->
          <template v-if="sliderDefaultAvailable">
            <div class="sec sub">{{ t("Default value") }}</div>
            <p class="help">
              {{ t("Double-tapping the slider restores this value - e.g. Voicemeeter 0 dB, Spotify 50%. Those two have built-in values; a typed number overrides them, and an empty field disables the reset for the remaining sliders. Works in touch mode and on tablets.") }}
            </p>
            <label class="field">
              {{ t("Default value (% 0-100)") }}
              <input
                v-model="sliderDefault"
                inputmode="decimal"
                :placeholder="t('e.g. 50')"
                @keydown.enter.prevent
              />
            </label>
            <div v-if="sliderDefaultError" class="dual-error">{{ sliderDefaultError }}</div>
          </template>

          <!-- custom gestures (desktop touch mode + tablets); hidden where
               the tile cannot take them (sliders, display and tool tiles) -->
          <template v-if="gesturesAvailable">
            <div class="sec sub"><Hand :size="14" /> {{ t("Gestures") }}</div>
            <p class="help">
              {{ t("On tablets and in touch mode. With no action picked, a gesture runs the tile's own action.") }}
            </p>
            <div class="gestures">
              <div
                v-for="def in GESTURE_DEFS"
                :key="def.key"
                class="gcard"
                :class="{ on: gestures[def.key].enabled }"
              >
                <label class="ghead">
                  <span class="gi"><component :is="GESTURE_ICONS[def.key]" :size="16" /></span>
                  <span class="gtext">
                    <span class="gn">{{ t(def.label) }}</span>
                    <span class="gs">{{ gestures[def.key].enabled ? gestureSummary(def.key) : t("Off") }}</span>
                  </span>
                  <input v-model="gestures[def.key].enabled" type="checkbox" class="switch" />
                </label>
                <div v-if="gestures[def.key].enabled" class="gbody">
                  <div class="field sel-field gesture-action">
                    <span class="sel-label">{{ t("Action") }}</span>
                    <ActionPicker
                      :model-value="gestures[def.key].type"
                      :groups="gesturePickerGroups"
                      @update:model-value="onGestureTypePicked(def.key, $event)"
                    />
                  </div>
                  <template v-for="f in gestureFieldList(def.key).filter((f) => gestureFieldVisible(gestures[def.key], f))" :key="f.key">
                    <div v-if="f.kind === 'select'" class="field sel-field">
                      <span class="sel-label">{{ t(f.label) }}</span>
                      <SelectField v-model="gestures[def.key].fields[f.key]" :options="f.options" :label="t(f.label)" />
                    </div>
                    <label v-else class="field">
                      {{ t(f.label) }}
                      <textarea
                        v-if="f.kind === 'textarea'"
                        v-model="gestures[def.key].fields[f.key]"
                        rows="2"
                        :placeholder="f.placeholder"
                      ></textarea>
                      <input
                        v-else
                        v-model="gestures[def.key].fields[f.key]"
                        :inputmode="f.kind === 'number' ? 'numeric' : undefined"
                        :placeholder="f.placeholder"
                        @keydown.enter.prevent
                      />
                    </label>
                  </template>
                </div>
              </div>
            </div>
            <div v-if="gestureError" class="dual-error">{{ gestureError }}</div>
          </template>
        </div>
      </div>

      <div class="modal-actions">
        <button v-if="!isCreate" class="btn-text quiet-danger" @click="removeTile">
          <Trash2 :size="14" />{{ t("Delete") }}
        </button>
        <span v-if="dirty" class="dirty-note"><i></i>{{ t("Unsaved changes") }}</span>
        <span class="spacer"></span>
        <button class="btn-text" @click="requestClose">{{ t("Cancel") }}</button>
        <button class="btn-text accent" @click="save">{{ isCreate ? t("Add") : t("Save") }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.btn-modal { width: min(900px, 95vw); padding: 0; overflow: hidden; }
.modal-head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 14px 13px 22px;
  border-bottom: 1px solid var(--line);
  flex: none;
}
.crumb {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 400;
  color: var(--ink-3);
  white-space: nowrap;
  overflow: hidden;
}
.crumb i { width: 3px; height: 3px; border-radius: 50%; background: var(--ink-3); flex: none; }
.head-close {
  margin-left: auto;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  color: var(--ink-2);
  display: grid;
  place-items: center;
  flex: none;
  transition: background 120ms ease-out, color 120ms ease-out;
}
.head-close:hover { background: var(--tile-2); color: var(--ink); }

/* two columns, each scrolls on its own so the footer stays in view */
.btn-body {
  display: grid;
  grid-template-columns: 340px minmax(0, 1fr);
  min-height: 0;
  /* fixed height: switching state or mode must not make the dialog jump */
  height: min(560px, calc(88vh - 122px));
}
.left-col,
.right-col { overflow-y: auto; min-height: 0; }
.left-col {
  background: var(--ground);
  border-right: 1px solid var(--line);
  padding: 14px 18px 18px;
}
.right-col { padding: 14px 22px 16px; }

.sec {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 1.1px;
  text-transform: uppercase;
  color: var(--ink-3);
  margin-bottom: 10px;
}
.sec svg { color: var(--accent); }
.sec.sub { border-top: 1px solid var(--line); padding-top: 14px; margin: 6px 0 4px; }
.lbl {
  display: block;
  font-size: 12.5px;
  font-weight: 500;
  color: var(--ink-2);
  margin: 13px 0 6px;
}
.lbl.first { margin-top: 0; }
.help { font-size: 12px; line-height: 1.5; color: var(--ink-3); margin: 6px 0 10px; }
.sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }

/* preview: the tile on its board color; a toggle shows both states and
   the one being edited is the large one */
.preview {
  background: var(--tint);
  height: 144px;
  border-radius: 12px;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 20px;
}
.prev-tile {
  position: relative;
  width: 104px;
  height: 104px;
  border: 2px solid transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.1), 0 1px 0 rgba(0, 0, 0, 0.2),
    0 3px 10px rgba(0, 0, 0, 0.3);
  overflow: hidden;
  font-family: var(--tile-font);
  cursor: default;
  transition: transform 160ms cubic-bezier(0.2, 0, 0, 1), opacity 160ms ease-out;
}
.prev-tile.cur { outline: 2px solid var(--accent); outline-offset: 3px; }
.prev-tile.ghost { transform: scale(0.62); margin: 0 -20px; opacity: 0.6; cursor: pointer; }
.prev-tile.ghost:hover { opacity: 0.85; }
.prev-img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
.prev-tile > i { font-size: 36px; text-shadow: 0 1px 3px rgba(0, 0, 0, 0.3); }
.prev-title {
  position: absolute;
  left: 0;
  right: 0;
  padding: 2px 5px;
  font-size: 12px;
  font-weight: 500;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.45);
}
.prev-title.pos-0 { bottom: 0; }
.prev-title.pos-1 { top: 50%; transform: translateY(-50%); text-shadow: none; }
.prev-title.pos-2 { top: 0; }

/* segmented controls */
.seg {
  display: flex;
  gap: 3px;
  padding: 3px;
  background: var(--well);
  border: 1px solid var(--line);
  border-radius: 9px;
}
.seg > button {
  flex: 1;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  color: var(--ink-2);
  transition: background 120ms ease-out, color 120ms ease-out;
}
.seg > button:hover:not(.on) { color: var(--ink); background: rgba(255, 255, 255, 0.04); }
.seg > button.on { background: var(--tile-2); color: var(--ink); box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3); }
.seg.accent > button.on { background: var(--accent-soft); color: var(--accent); box-shadow: none; }
.dot { width: 9px; height: 9px; border-radius: 3px; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.25); flex: none; }
.two { display: grid; grid-template-columns: 1fr 1.4fr; gap: 10px; }

/* icon / image row */
.icon-row { display: flex; gap: 6px; }
.pop-anchor { position: relative; }
.pop-anchor.grow { flex: 1; min-width: 0; }
.ibtn {
  height: 38px;
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 0 10px;
  border-radius: 8px;
  background: var(--well);
  border: 1px solid var(--line);
  font-size: 13px;
  color: var(--ink-2);
  transition: border-color 120ms ease-out, color 120ms ease-out;
}
.ibtn:hover { border-color: var(--lift); color: var(--ink); }
.icon-btn { width: 100%; }
.glyph {
  width: 26px;
  height: 26px;
  border-radius: 6px;
  background: var(--tile-2);
  display: grid;
  place-items: center;
  font-size: 13px;
  color: var(--ink);
  flex: none;
}
.ival { flex: 1; min-width: 0; text-align: left; color: var(--ink); font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.chev { color: var(--ink-3); flex: none; }
.img-btn { flex: none; }
.img-clear { flex: none; width: 38px; justify-content: center; padding: 0; }

/* color swatches: every value visible, one click to edit */
.swatches { display: grid; grid-template-columns: repeat(5, 1fr); gap: 6px; }
.swatches.single { grid-template-columns: 1fr 1fr 1fr 1fr 1fr; }
.swatch-btn { display: flex; flex-direction: column; align-items: center; gap: 5px; min-width: 0; }
.sw {
  width: 100%;
  height: 30px;
  border-radius: 7px;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  transition: box-shadow 120ms ease-out;
}
.sw.none {
  background: repeating-linear-gradient(135deg, transparent 0 6px, rgba(255, 255, 255, 0.07) 6px 7px);
  box-shadow: inset 0 0 0 1px var(--lift);
}
.swatch-btn:hover .sw { box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.35); }
.swatch-btn.on .sw { box-shadow: 0 0 0 2px var(--ground), 0 0 0 4px var(--accent); }
.swl { font-size: 11px; color: var(--ink-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
.swatch-btn.on .swl { color: var(--ink); }

.prop-pop {
  position: absolute;
  left: 0;
  right: 0;
  top: calc(100% + 6px);
  background: var(--tile-2);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 10px;
  box-shadow: var(--shadow-pop);
  padding: 10px;
  z-index: 6;
}
.icon-pop { width: 304px; right: auto; }
.pop-title { font-size: 12px; color: var(--ink-2); margin-bottom: 8px; }

.icon-edit { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; }
.icon-edit input { flex: 1; padding: 6px 8px; font-size: 13px; }
.icon-edit .big { font-size: 22px; width: 26px; text-align: center; }
.icon-grid {
  display: grid;
  grid-template-columns: repeat(8, 1fr);
  gap: 2px;
  max-height: 232px;
  overflow-y: auto;
}
.icon-search { width: 100%; padding: 6px 8px; font-size: 13px; margin-bottom: 6px; box-sizing: border-box; }
.icon-group {
  grid-column: 1 / -1;
  padding: 8px 2px 2px;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--modal-muted);
}
.icon-group:first-child { padding-top: 2px; }
.icon-empty { grid-column: 1 / -1; margin: 10px 0; font-size: 12.5px; color: var(--modal-muted); text-align: center; }
.icon-cell {
  height: 30px;
  border-radius: 6px;
  font-size: 14px;
  color: var(--ink-2);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 120ms ease-out, color 120ms ease-out;
}
.icon-cell:hover { background: var(--lift); color: var(--ink); }
.icon-cell.on { background: var(--accent); color: var(--accent-ink); }

.color-edit { display: flex; align-items: center; gap: 8px; }
.color-edit .swatch {
  width: 42px;
  height: 34px;
  padding: 2px;
  flex: none;
  cursor: pointer;
}
.color-edit .hex { flex: 1; min-width: 0; padding: 6px 8px; font-size: 13px; }

/* behavior column */
.mode-locked {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 38px;
  padding: 0 12px;
  border-radius: 9px;
  background: var(--well);
  border: 1px solid var(--line);
  font-size: 13px;
  font-weight: 500;
}
.mode-locked svg { color: var(--ink-3); }
.mode-locked span { color: var(--ink-3); font-weight: 400; }
.mode-seg { margin-bottom: 0; }
.right-col .field { margin: 13px 0 0; }
.right-col .field:last-child { margin-bottom: 0; }

.hotkey {
  margin-top: 6px;
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 40px;
  padding: 0 4px 0 0;
  background: var(--well);
  border: 1px solid var(--line);
  border-radius: var(--r-control);
  transition: border-color 120ms ease-out, box-shadow 120ms ease-out;
}
.hotkey:hover { border-color: var(--lift); }
.hotkey.rec { border-color: var(--accent); box-shadow: 0 0 0 3px rgba(26, 188, 156, 0.18); }
.hotkey { position: relative; }
.hotkey input { flex: 1; border: none; background: transparent; box-shadow: none; margin: 0; }
.caps + input { opacity: 0; }
.hotkey input:focus { box-shadow: none; }
.caps {
  position: absolute;
  inset: 0 110px 0 0;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  text-align: left;
  min-width: 0;
  flex-wrap: wrap;
}
kbd {
  font: 500 12px var(--ui-font);
  color: var(--ink);
  background: var(--tile-2);
  border: 1px solid var(--lift);
  border-bottom-width: 2px;
  border-radius: 6px;
  padding: 2px 8px;
}
.plus { color: var(--ink-3); font-size: 12px; }
.rec-btn {
  flex: none;
  display: flex;
  align-items: center;
  gap: 7px;
  height: 30px;
  padding: 0 10px;
  border-radius: 6px;
  background: var(--tile-2);
  font-size: 12.5px;
  color: var(--ink-2);
  transition: background 120ms ease-out, color 120ms ease-out;
}
.rec-btn:hover { background: var(--lift); color: var(--ink); }
.rec-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--danger); }
.hotkey.rec .rec-btn { color: var(--accent); }
.hotkey.rec .rec-dot { animation: pulse 1s ease-in-out infinite; }
@keyframes pulse { 50% { opacity: 0.3; } }

.win-row { display: flex; gap: 16px; margin-top: 6px; }
.win-check { display: flex; align-items: center; gap: 6px; font-size: 13px; color: var(--ink); cursor: pointer; }
.no-opts {
  font-size: 13px;
  color: var(--modal-muted);
  background: var(--modal-field);
  border-radius: 8px;
  padding: 10px 12px;
  margin-top: 13px;
}
.steps { display: flex; flex-direction: column; gap: 6px; margin-top: 13px; }
.step { display: flex; gap: 6px; }
.step-type { max-width: 150px; flex: none; }
.step-del {
  width: 32px;
  flex: none;
  border-radius: 7px;
  color: var(--modal-muted);
  transition: background 120ms ease-out, color 120ms ease-out;
}
.step-del:hover { background: rgba(231, 76, 60, 0.12); color: var(--danger); }
.add-step { align-self: flex-start; }
.dual-error { font-size: 12px; color: var(--danger); margin-top: 6px; overflow-wrap: anywhere; }

/* gestures: one card per row; an enabled card opens its action picker
   in place, so nothing around it moves */
.gestures { display: grid; grid-template-columns: 1fr; gap: 6px; }
.gcard {
  border-radius: 10px;
  background: var(--well);
  border: 1px solid var(--line);
  transition: border-color 120ms ease-out, background 120ms ease-out;
}
.gcard.on {
  border-color: rgba(26, 188, 156, 0.45);
  background: linear-gradient(rgba(26, 188, 156, 0.05), rgba(26, 188, 156, 0.05)), var(--well);
}
.ghead {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 12px;
  cursor: pointer;
}
.gi {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--tile-2);
  color: var(--ink-2);
  display: grid;
  place-items: center;
  flex: none;
}
.gcard.on .gi { background: var(--accent-soft); color: var(--accent); }
.gtext { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.gn { font-size: 13px; font-weight: 500; color: var(--ink); }
.gs { font-size: 11.5px; color: var(--ink-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.gcard.on .gs { color: var(--ink-2); }
.gbody { padding: 0 12px 12px; }
.gbody .field:first-child { margin-top: 0; }
/* a switch instead of the square checkbox */
.switch {
  width: 32px;
  height: 18px;
  border-radius: 9px;
  border: none;
  background: var(--lift);
  position: relative;
  flex: none;
  vertical-align: middle;
  transition: background 140ms ease-out;
}
.switch::after {
  content: "";
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #cfd8dc;
  transition: transform 140ms cubic-bezier(0.2, 0, 0, 1), background 140ms ease-out;
}
.switch:checked { background: var(--accent); background-image: none; }
.switch:checked::after { transform: translateX(14px); background: #fff; }

.modal-actions {
  border-top: 1px solid var(--line);
  padding: 12px 16px;
  flex: none;
}
.quiet-danger {
  display: flex;
  align-items: center;
  gap: 7px;
  background: transparent;
  color: var(--danger-ink);
  padding-left: 8px;
  padding-right: 10px;
}
.quiet-danger:hover { background: var(--danger-soft); }
.dirty-note { display: flex; align-items: center; gap: 6px; margin-left: 8px; font-size: 12px; color: var(--ink-3); }
.dirty-note i { width: 6px; height: 6px; border-radius: 50%; background: #f1c40f; }
.spacer { flex: 1; }
.mini {
  font-size: 12.5px;
  padding: 7px 11px;
  border-radius: 7px;
  background: var(--tile-2);
  transition: background 120ms ease-out;
}
.mini:hover { background: var(--lift); }

.pop-enter-active { transition: opacity 130ms ease-out, transform 130ms cubic-bezier(0.2, 0, 0, 1); }
.pop-leave-active { transition: opacity 90ms ease-out; }
.pop-enter-from { opacity: 0; transform: translateY(-4px); }
.pop-leave-to { opacity: 0; }
</style>
