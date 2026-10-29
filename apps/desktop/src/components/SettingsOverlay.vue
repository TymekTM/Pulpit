<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { setUiLang, t, uiLang } from "../i18n";
import DiscordSettings from "./settings/DiscordSettings.vue";
import VoicemeeterSettings from "./settings/VoicemeeterSettings.vue";
import ObsSettings from "./settings/ObsSettings.vue";

const props = defineProps({
  status: { type: Object, required: true },
  // live pushed values (APP_CUSTOM_VALUE mirror): the spotify-auth and
  // spotify-device keys feed the Spotify status line without reopening
  customValues: { type: Object, default: () => ({}) },
});
const emit = defineEmits(["close"]);

const rootEl = ref(null);
const closeBtn = ref(null);

// the wall keeps exactly one expanded tile; everything else dims
const focused = ref("launch");

const autostart = ref(false);
const autostartBusy = ref(false);
const autostartError = ref("");

// start-minimized applies on the next launch: the window is created
// hidden (or not) once, in the backend's setup
const startMinimized = ref(false);
const startMinimizedBusy = ref(false);
const startMinimizedError = ref("");

const hotkey = ref("Ctrl+Alt+D");
const hotkeyDraft = ref("");
const hotkeyError = ref("");
const editingHotkey = ref(false);

const lanAddresses = ref([]);
const activeAddress = ref(0);
const lanLoading = ref(false);

const pairingOffer = ref(null); // {code, expires_in_secs, addresses}
const pairingBusy = ref(false);
const pairingLeft = ref(0);
const pairingError = ref("");
let pairingTimer = null;

// trusted v2 devices (the backend's devices.json, minus token material)
const devices = ref([]);
const devicesBusy = ref(false);

// server port editing; the value applies on the next launch because the
// sockets bind once at startup and tablets aim at host:port explicitly
const portDraft = ref("");
const portError = ref("");
const portPending = ref(null); // stored value that differs from the running one
const portLocked = ref(false); // the PULPIT_PORT env var overrides the store
const portBusy = ref(false);

// in-app revoke confirmation (replaces the native dialog popup)
const confirmRevoke = ref(null); // device awaiting confirmation
const revokeError = ref("");
const cancelBtn = ref(null);

// M8 sideload: adb device list + keep-data APK install
const adbList = ref([]);
const adbBusy = ref(false);
const adbNote = ref("");
const adbNoteBad = ref(false);

// AI usage (Settings): the aidev producer detects the plan-limits rows
// automatically; the user picks which ones the tile shows. An empty
// stored selection means "everything detected" and is materialized into
// checked boxes on load; saving writes [] only while nothing is hidden.
const aidevDetected = ref([]); // [{id, label}] from the producer registry
const aidevShow = ref([]); // working set of checked row ids
const aidevSummary = ref(true);
const aidevRowStyle = ref("name"); // "name" | "logo" row identifier
const aidevBusy = ref(false);
const aidevError = ref("");
const aidevNote = ref("");

// provider slug -> display name of a checkbox group
const AIDEV_PROVIDERS = {
  glm: "GLM",
  codex: "Codex",
  claude: "Claude",
  openrouter: "OpenRouter",
  "anthropic-api": "Anthropic API",
  antigravity: "Antigravity",
  custom: null, // translated where the groups are built
};

const aidevGroups = computed(() => {
  const groups = new Map();
  for (const row of aidevDetected.value) {
    const slug = String(row.id).split(":")[0];
    const name = AIDEV_PROVIDERS[slug] ?? (slug === "custom" ? t("Other") : slug);
    if (!groups.has(name)) groups.set(name, []);
    groups.get(name).push(row);
  }
  return [...groups.entries()].map(([name, rows]) => ({ name, rows }));
});

async function refreshAidev() {
  try {
    const cfg = await api.aidevStatusConfig();
    aidevDetected.value = cfg.detected || [];
    // an absent selection means everything detected is on
    const saved = new Set(cfg.show || []);
    aidevShow.value = saved.size
      ? aidevDetected.value.filter((r) => saved.has(r.id)).map((r) => r.id)
      : aidevDetected.value.map((r) => r.id);
    aidevSummary.value = cfg.summary !== false;
    aidevRowStyle.value = cfg.row_style === "logo" ? "logo" : "name";
  } catch (e) {
    aidevError.value = e ? String(e) : t("Could not load the AI usage settings.");
  }
}

async function saveAidev() {
  aidevBusy.value = true;
  aidevError.value = "";
  try {
    // an all-checked working set stores [] so future detected rows keep
    // appearing automatically
    const all = aidevDetected.value.map((r) => r.id);
    const everything = all.length > 0 && aidevShow.value.length === all.length;
    await api.setAidevStatusConfig(
      everything ? [] : aidevShow.value,
      aidevSummary.value,
      aidevRowStyle.value,
    );
    aidevNote.value = t("Saved - takes effect on the tile's next refresh.");
    setTimeout(() => {
      aidevNote.value = "";
    }, 4000);
  } catch (e) {
    aidevError.value = e ? String(e) : t("Could not save the AI usage settings.");
  } finally {
    aidevBusy.value = false;
  }
}

function aidevChecked(id) {
  return aidevShow.value.includes(id);
}

function toggleAidevRow(id) {
  const next = aidevChecked(id)
    ? aidevShow.value.filter((x) => x !== id)
    : [...aidevShow.value, id];
  if (!next.length) {
    aidevNote.value = t("At least one row must stay selected.");
    setTimeout(() => {
      aidevNote.value = "";
    }, 4000);
    return;
  }
  aidevShow.value = next;
  saveAidev();
}

function toggleAidevSummary() {
  aidevSummary.value = !aidevSummary.value;
  saveAidev();
}

function setAidevRowStyle(style) {
  if (aidevRowStyle.value === style) return;
  aidevRowStyle.value = style;
  saveAidev();
}

// Spotify (Settings): BYO client id + PKCE login (design §2/§5). The
// status line combines the spotify_status command with the live
// customValues pushes (spotify-auth / spotify-device), so an expired
// login flips the line without reopening the panel.
const spotify = ref({
  configured: false,
  clientId: "",
  loggedIn: false,
  user: null,
  product: null,
  redirectUri: "http://127.0.0.1:8502/spotify/callback",
});
const spotifyClientIdDraft = ref("");
const spotifyBusy = ref(false);
const spotifyError = ref("");
const spotifyNote = ref("");
const spotifyHelpOpen = ref(false);

async function refreshSpotify() {
  try {
    const s = await api.spotifyStatus();
    spotify.value = {
      configured: s.configured !== false,
      clientId: s.clientId || "",
      loggedIn: Boolean(s.loggedIn),
      user: s.user || null,
      product: s.product || null,
      redirectUri: s.redirectUri || "http://127.0.0.1:8502/spotify/callback",
    };
    spotifyClientIdDraft.value = s.clientId || "";
  } catch (e) {
    spotifyError.value = e ? String(e) : t("Could not load the Spotify status.");
  }
}

const spotifyAuthLive = computed(() => props.customValues["spotify-auth"] || null);

const spotifyLoggedIn = computed(
  () => spotify.value.configured && (spotifyAuthLive.value === "ok" || (!spotifyAuthLive.value && spotify.value.loggedIn))
);

const spotifyStatusLine = computed(() => {
  if (!spotify.value.configured) {
    return t("Spotify is disabled - the spotify.json file is corrupted.");
  }
  const auth = spotifyAuthLive.value || (spotify.value.loggedIn ? "ok" : "needs-login");
  if (auth === "off") return t("Spotify is inactive - paste a Client ID and log in.");
  if (auth === "needs-login" || !spotifyLoggedIn.value) {
    return t("Login required - the session expired or was never set up.");
  }
  const user = spotify.value.user;
  const plan =
    spotify.value.product === "free"
      ? t("Free - controlling playback requires Premium")
      : "Premium";
  const device = props.customValues["spotify-device"];
  const base = user
    ? t("Logged in as {user} ({plan})", { user, plan })
    : t("Logged in ({plan})", { plan });
  return device ? t("{status} - device: {device}", { status: base, device }) : base;
});

const spotifyChip = computed(() => {
  if (!spotify.value.configured) return { text: t("off"), ok: false };
  const auth = spotifyAuthLive.value || (spotify.value.loggedIn ? "ok" : "needs-login");
  if (auth === "ok") return { text: spotify.value.user || t("logged in"), ok: true };
  if (auth === "needs-login") return { text: t("login required"), ok: false };
  return { text: t("off"), ok: false };
});

function flashSpotifyNote(text) {
  spotifyNote.value = text;
  setTimeout(() => {
    if (spotifyNote.value === text) spotifyNote.value = "";
  }, 4000);
}

async function saveSpotifyClientId() {
  spotifyBusy.value = true;
  spotifyError.value = "";
  try {
    await api.spotifySetClientId(spotifyClientIdDraft.value.trim());
    flashSpotifyNote(t("Client ID saved."));
    await refreshSpotify();
  } catch (e) {
    spotifyError.value = e ? String(e) : t("Could not save the Client ID.");
  } finally {
    spotifyBusy.value = false;
  }
}

async function doSpotifyLogin() {
  spotifyBusy.value = true;
  spotifyError.value = "";
  flashSpotifyNote(t("Opening the browser - log in and come back to Pulpit."));
  try {
    const r = await api.spotifyLogin();
    flashSpotifyNote(r?.user ? t("Logged in as {user}.", { user: r.user }) : t("Logged in."));
    await refreshSpotify();
  } catch (e) {
    spotifyError.value = e ? String(e) : t("Spotify login failed.");
  } finally {
    spotifyBusy.value = false;
  }
}

async function doSpotifyLogout() {
  spotifyBusy.value = true;
  spotifyError.value = "";
  try {
    await api.spotifyLogout();
    flashSpotifyNote(t("Logged out. The Client ID was kept."));
    await refreshSpotify();
  } catch (e) {
    spotifyError.value = e ? String(e) : t("Could not log out.");
  } finally {
    spotifyBusy.value = false;
  }
}

async function copyRedirectUri() {
  try {
    await navigator.clipboard.writeText(spotify.value.redirectUri);
    flashSpotifyNote(t("Redirect URI copied."));
  } catch {
    spotifyError.value = t("Could not copy - select it and copy manually.");
  }
}

async function refreshAdb() {  adbBusy.value = true;
  try {
    adbList.value = await api.adbDevices();
  } catch (e) {
    adbList.value = [];
    adbNote.value = String(e);
    adbNoteBad.value = true;
  } finally {
    adbBusy.value = false;
  }
}

async function installApk() {
  const path = await open({
    multiple: false,
    filters: [{ name: "APK", extensions: ["apk"] }],
  });
  if (!path) return;
  adbBusy.value = true;
  adbNote.value = t("Installing (adb install -r, app data is kept)...");
  adbNoteBad.value = false;
  try {
    adbNote.value = (await api.adbInstallApk(String(path))) || t("Done.");
    await refreshAdb();
  } catch (e) {
    adbNote.value = String(e);
    adbNoteBad.value = true;
  } finally {
    adbBusy.value = false;
  }
}

// Tablet update: the same feed carries the release APK; the backend
// downloads it, checks the sha256 and runs adb install -r on every
// attached tablet that is behind.
async function updateTablet() {
  adbBusy.value = true;
  adbNote.value = t("Downloading the APK from GitHub and installing it on the tablet...");
  adbNoteBad.value = false;
  try {
    adbNote.value = (await api.installTabletUpdate()) || t("Done.");
    await refreshAdb();
  } catch (e) {
    adbNote.value = String(e);
    adbNoteBad.value = true;
  } finally {
    adbBusy.value = false;
  }
}

// M8 updates: the feed check shows what's available; the install button
// downloads, verifies the sha256 and swaps the exe, then the app
// restarts itself
const updateBusy = ref(false);
const updateNote = ref("");
const updateNoteBad = ref(false);
const updateReady = ref(""); // version string when a newer build is staged on the feed

async function checkUpdates() {
  updateBusy.value = true;
  updateNote.value = t("Checking...");
  updateNoteBad.value = false;
  updateReady.value = "";
  try {
    const r = await api.checkForUpdates();
    updateReady.value = r.update_available ? r.latest : "";
    updateNote.value = r.update_available
      ? t("Version {latest} is available (you have {current}).", { latest: r.latest, current: r.current })
      : t("Up to date (v{current}).", { current: r.current });
  } catch (e) {
    updateNote.value = String(e);
    updateNoteBad.value = true;
  } finally {
    updateBusy.value = false;
  }
}

async function installUpdate() {
  updateBusy.value = true;
  updateNote.value = t("Downloading and installing...");
  updateNoteBad.value = false;
  try {
    updateNote.value = await api.installUpdate();
    // the backend schedules app.exit(0) and a fresh launch; keep the
    // note on screen for the ride
  } catch (e) {
    updateNote.value = String(e);
    updateNoteBad.value = true;
    updateBusy.value = false;
  }
}

const version = computed(() => props.status.version || "0.1.1");

function tileCls(id) {
  return { focused: focused.value === id, dim: focused.value !== id };
}

async function refreshLan() {
  lanLoading.value = true;
  try {
    lanAddresses.value = await api.listLanAddresses();
    if (activeAddress.value >= lanAddresses.value.length) activeAddress.value = 0;
  } catch {
    lanAddresses.value = [];
  } finally {
    lanLoading.value = false;
  }
}

function stopPairingClock() {
  if (pairingTimer) {
    clearInterval(pairingTimer);
    pairingTimer = null;
  }
}

async function generatePairing() {
  pairingBusy.value = true;
  pairingError.value = "";
  try {
    pairingOffer.value = await api.createPairingCode();
    pairingLeft.value = pairingOffer.value.expires_in_secs;
    stopPairingClock();
    pairingTimer = setInterval(() => {
      pairingLeft.value -= 1;
      if (pairingLeft.value <= 0) {
        // one-time code burned out: back to the minting state
        pairingOffer.value = null;
        stopPairingClock();
      }
    }, 1000);
  } catch (e) {
    pairingError.value = e
      ? String(e)
      : t("Could not generate a pairing code.");
  } finally {
    pairingBusy.value = false;
  }
}

const pairingClock = computed(() => {
  const s = Math.max(0, pairingLeft.value);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
});

async function refreshDevices() {
  try {
    devices.value = await api.listDevices();
  } catch {
    // v2 stack unavailable: show none rather than stale rows
    devices.value = [];
  }
}

// a pairing can complete while the overlay is open: refetch when the
// devices tile is (re)focused. The AI-usage detection list also grows
// while the overlay sits open, so refetch on (re)focus too.
watch(focused, (tile) => {
  if (tile === "devices") refreshDevices();
  if (tile === "aidev") refreshAidev();
  if (tile === "spotify") refreshSpotify();
});

function revokeDevice(device) {
  revokeError.value = "";
  confirmRevoke.value = device;
}

async function doRevoke() {
  const device = confirmRevoke.value;
  if (!device) return;
  devicesBusy.value = true;
  revokeError.value = "";
  try {
    await api.revokeDevice(device.id);
  } catch (e) {
    // keep the popup open with the reason; the device list is stale anyway
    revokeError.value = e ? String(e) : t("Could not revoke the device.");
    return;
  } finally {
    devicesBusy.value = false;
  }
  confirmRevoke.value = null;
  refreshDevices();
}

// focus lands on Cancel so a reflexive Enter cannot fire the destructive
// action; Escape closes just the popup while it is up
watch(confirmRevoke, (device) => {
  if (device) nextTick(() => cancelBtn.value?.focus());
});

async function savePort() {
  const value = Number(portDraft.value.trim());
  if (!Number.isInteger(value) || value < 1024 || value > 65535) {
    portError.value = t("The port must be a number between 1024 and 65535.");
    return;
  }
  portBusy.value = true;
  try {
    await api.setServerPort(value);
    portError.value = "";
    portPending.value = value === (props.status.port || 8500) ? null : value;
  } catch (e) {
    // the backend message says why the port was refused
    portError.value = e ? String(e) : t("Could not save the port.");
  } finally {
    portBusy.value = false;
  }
}

function lastSeenLabel(unixSecs) {
  if (!unixSecs) return t("never");
  const diff = Math.max(0, Date.now() / 1000 - unixSecs);
  if (diff < 90) return t("now");
  if (diff < 3600) return t("{n} min ago", { n: Math.floor(diff / 60) });
  if (diff < 48 * 3600) return t("{n} h ago", { n: Math.floor(diff / 3600) });
  return new Date(unixSecs * 1000).toLocaleDateString();
}

async function saveHotkey() {
  const combo = hotkeyDraft.value.trim();
  try {
    await api.setHotkey(combo);
    hotkey.value = combo;
    hotkeyError.value = "";
    editingHotkey.value = false;
  } catch (e) {
    // the backend message says why the combo was refused (taken by
    // another app, unparseable) - show it instead of a generic guess
    hotkeyError.value = e
      ? String(e)
      : t("This shortcut is taken or not allowed - pick another combination.");
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
    // flip back and say why - a silent revert leaves the user guessing
    startMinimized.value = !next;
    startMinimizedError.value = e ? String(e) : t("Could not save the start option.");
  } finally {
    startMinimizedBusy.value = false;
  }
}

// the select is v-model bound to uiLang (kept in sync by i18n.js); a
// failed persist is reverted inside setUiLang, so only the error is
// surfaced here - via the title tooltip the next render shows the old choice
async function onLangChange() {
  try {
    await setUiLang(uiLang.value);
  } catch {
    /* the optimistic switch is already reverted */
  }
}

function onKeydown(e) {
  if (e.key === "Escape") {
    if (confirmRevoke.value) {
      confirmRevoke.value = null;
      return;
    }
    emit("close");
    return;
  }
  // light focus loop: the overlay is opaque, so Tab must never wander
  // into the editor behind it
  if (e.key === "Tab" && rootEl.value && !rootEl.value.contains(document.activeElement)) {
    closeBtn.value?.focus();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);
  closeBtn.value?.focus();
  try {
    const settings = await api.getSettings();
    hotkey.value = settings.hotkey;
    startMinimized.value = Boolean(settings.start_minimized);
    portLocked.value = Boolean(settings.port_locked);
    portPending.value =
      settings.port && settings.port !== (props.status.port || 8500)
        ? settings.port
        : null;
    portDraft.value = String(settings.port ?? props.status.port ?? 8500);
  } catch {
    /* keep the defaults when the backend is unreachable */
  }
  autostart.value = await api.getAutostart().catch(() => false);
  refreshLan();
  refreshDevices();
  refreshAidev();
  refreshSpotify();
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  stopPairingClock();
});
</script>

<template>
  <div ref="rootEl" class="settings" role="dialog" aria-modal="true" :aria-label="t('Settings')">
    <header class="bar">
      <h1>{{ t("Settings") }}</h1>
      <select
        v-model="uiLang"
        class="lang"
        :aria-label="t('UI language')"
        @change="onLangChange"
      >
        <option value="auto">{{ t("Auto (system)") }}</option>
        <option value="pl">Polski</option>
        <option value="en">English</option>
      </select>
      <button ref="closeBtn" class="close" :title="t('Close (Esc)')" @click="$emit('close')">
        <i class="fas fa-times" aria-hidden="true"></i>
      </button>
    </header>

    <div class="wall">
      <section class="row" aria-labelledby="sec-general">
        <h2 id="sec-general">{{ t("General") }}</h2>
        <div class="tiles">
          <article class="tile" :class="tileCls('launch')">
            <button
              class="hit"
              :tabindex="focused === 'launch' ? -1 : 0"
              :aria-label="t('Expand: Startup')"
              @click="focused = 'launch'"
            ></button>
            <header class="tile-head">
              <h3>{{ t("Startup") }}</h3>
              <span class="chip" :class="autostart ? 'ok' : ''">
                {{ autostart ? t("autostart on") : t("autostart off") }}
              </span>
            </header>
            <p class="sum">{{ t("How Pulpit behaves after sign-in and when the window is closed.") }}</p>
            <div v-show="focused === 'launch'" class="detail">
              <div class="ctl">
                <div class="ctl-text">
                  <span class="ctl-name">{{ t("Run after sign-in") }}</span>
                  <span class="ctl-note">{{ t("Registers Pulpit in the system autostart.") }}</span>
                </div>
                <button
                  class="switch"
                  role="switch"
                  :aria-checked="autostart"
                  :disabled="autostartBusy"
                  @click="toggleAutostart"
                ><span class="knob"></span></button>
              </div>
              <p v-if="autostartError" class="err">{{ autostartError }}</p>
              <div class="ctl">
                <div class="ctl-text">
                  <span class="ctl-name">{{ t("Start minimized") }}</span>
                  <span class="ctl-note">{{ t("The next launch starts with the window hidden to the tray.") }}</span>
                </div>
                <button
                  class="switch"
                  role="switch"
                  :aria-checked="startMinimized"
                  :disabled="startMinimizedBusy"
                  @click="toggleStartMinimized"
                ><span class="knob"></span></button>
              </div>
              <p v-if="startMinimizedError" class="err">{{ startMinimizedError }}</p>
              <div class="ctl">
                <div class="ctl-text">
                  <span class="ctl-name">{{ t("Close to tray") }}</span>
                  <span class="ctl-note">{{ t("Always on - closing the window hides Pulpit to the tray.") }}</span>
                </div>
                <span class="locked"><i class="fas fa-lock" aria-hidden="true"></i> {{ t("fixed") }}</span>
              </div>
            </div>
          </article>

          <article class="tile" :class="tileCls('hotkey')">
            <button
              class="hit"
              :tabindex="focused === 'hotkey' ? -1 : 0"
              :aria-label="t('Expand: Touch-mode hotkey')"
              @click="focused = 'hotkey'"
            ></button>
            <header class="tile-head">
              <h3>{{ t("Touch-mode hotkey") }}</h3>
            </header>
            <p class="sum">{{ t("Shortcut toggling the editor into full-screen touch mode.") }}</p>
            <div v-show="focused === 'hotkey'" class="detail">
              <div v-if="!editingHotkey" class="combo-line">
                <kbd class="combo">{{ hotkey }}</kbd>
                <button class="act" @click="editingHotkey = true; hotkeyDraft = hotkey">{{ t("Change") }}</button>
              </div>
              <template v-else>
                <div class="combo-line">
                  <input
                    v-model="hotkeyDraft"
                    class="combo-input"
                    placeholder="Ctrl+Alt+D"
                    :aria-label="t('New shortcut')"
                    @keyup.enter="saveHotkey"
                  />
                  <button class="act accent" @click="saveHotkey">{{ t("Set") }}</button>
                  <button class="act" @click="editingHotkey = false">{{ t("Cancel") }}</button>
                </div>
                <p v-if="hotkeyError" class="err">{{ hotkeyError }}</p>
              </template>
            </div>
          </article>
        </div>
      </section>

      <section class="row" aria-labelledby="sec-server">
        <h2 id="sec-server">{{ t("Server") }}</h2>
        <div class="tiles">
          <article class="tile" :class="tileCls('status')">
            <button
              class="hit"
              :tabindex="focused === 'status' ? -1 : 0"
              :aria-label="t('Expand: Server status')"
              @click="focused = 'status'"
            ></button>
            <header class="tile-head">
              <h3>{{ t("Status") }}</h3>
              <span class="chip" :class="status.dbOk ? 'ok' : 'bad'">
                {{ status.dbOk ? t("running") : t("database unavailable") }}
              </span>
            </header>
            <p v-if="status.dbOk" class="sum">
              {{ t("Listening on 0.0.0.0:{port} - legacy and v2 share one port.", { port: status.port }) }}
            </p>
            <p v-else class="sum">
              {{ t("The database is locked or missing. Close the original app and start Pulpit again.") }}
            </p>
            <div v-show="focused === 'status' && status.dbOk" class="detail">
              <dl class="facts">
                <div><dt>{{ t("Address") }}</dt><dd class="mono tnum">0.0.0.0:{{ status.port }}</dd></div>
                <div><dt>{{ t("Clients") }}</dt><dd class="tnum">{{ status.clients }}</dd></div>
                <div><dt>{{ t("Version") }}</dt><dd>Editor v{{ version }}</dd></div>
                <div><dt>{{ t("Logs") }}</dt><dd class="mono">~/pulpitApp/logs</dd></div>
              </dl>
              <div class="ctl" style="margin-top: 12px">
                <div class="ctl-text">
                  <span class="ctl-name">{{ t("Updates") }}</span>
                  <span class="ctl-note">{{ t("Feed from update_url (latest.json in the repo by default); downloads, verifies sha256 and restarts.") }}</span>
                </div>
                <span style="display: flex; gap: 8px; flex: none">
                  <button class="act" :disabled="updateBusy" @click="checkUpdates">{{ t("Check") }}</button>
                  <button
                    v-if="updateReady"
                    class="act accent"
                    :disabled="updateBusy"
                    @click="installUpdate"
                  >{{ t("Install") }} {{ updateReady }}</button>
                </span>
              </div>
              <p v-if="updateNote" :class="updateNoteBad ? 'err' : 'note pending'">{{ updateNote }}</p>
            </div>
          </article>

          <article class="tile" :class="tileCls('port')">
            <button
              class="hit"
              :tabindex="focused === 'port' ? -1 : 0"
              :aria-label="t('Expand: Listen port')"
              @click="focused = 'port'"
            ></button>
            <header class="tile-head">
              <h3>{{ t("Listen port") }}</h3>
            </header>
            <p class="sum">{{ t("One port for both protocols; tablets connect to this address.") }}</p>
            <div v-show="focused === 'port'" class="detail">
              <div class="port-line">
                <span class="big mono tnum">{{ status.port || 8500 }}</span>
                <template v-if="!portLocked">
                  <input
                    v-model="portDraft"
                    class="port-input"
                    inputmode="numeric"
                    autocomplete="off"
                    :aria-label="t('New port')"
                    @keyup.enter="savePort"
                  />
                  <button class="act accent" :disabled="portBusy" @click="savePort">{{ t("Save") }}</button>
                </template>
              </div>
              <p v-if="portPending" class="note pending">
                {{ t("Port {port} saved - it will be used after Pulpit restarts.", { port: portPending }) }}
              </p>
              <p v-if="portLocked" class="note">
                {{ t("The port is dictated by the") }}
                <code class="mono">PULPIT_PORT</code>
                {{ t("environment variable - its value wins over the stored one.") }}
              </p>
              <p v-else class="note">
                {{ t("The allowed range is 1024-65535. A change applies after a restart; tablets aim at a specific") }}
                <span class="mono">host:port</span>
                {{ t("address, so they must be pointed at the new port.") }}
              </p>
              <p v-if="portError" class="err">{{ portError }}</p>
            </div>
          </article>
        </div>
      </section>

      <section class="row" aria-labelledby="sec-tablets">
        <h2 id="sec-tablets">{{ t("Tablets") }}</h2>
        <div class="tiles">
          <article class="tile" :class="[tileCls('legacy'), { dead: !status.dbOk }]">
            <button
              class="hit"
              :tabindex="focused === 'legacy' && status.dbOk ? -1 : 0"
              :aria-label="t('Expand: Stock client')"
              @click="focused = 'legacy'"
            ></button>
            <header class="tile-head">
              <h3>{{ t("Stock client") }}</h3>
              <span v-if="status.dbOk" class="chip" :class="lanAddresses.length ? 'ok' : ''">
                {{ lanAddresses.length ? t("ready to pair") : t("no network") }}
              </span>
              <span v-else class="chip bad">{{ t("server off") }}</span>
            </header>
            <p class="sum">{{ t("The original Deckboard app on Android connects via QR.") }}</p>
            <div v-show="focused === 'legacy' && status.dbOk" class="detail">
              <template v-if="lanAddresses.length">
                <div class="pair">
                  <img
                    class="qr"
                    :src="lanAddresses[activeAddress]?.qr"
                    :alt="t('QR code with the Pulpit address')"
                  />
                  <div class="addr-list">
                    <button
                      v-for="(a, i) in lanAddresses"
                      :key="a.ipv4"
                      class="addr"
                      :class="{ sel: i === activeAddress }"
                      @click="activeAddress = i"
                    >
                      <span class="mono tnum">{{ a.ipv4 }}:{{ status.port }}</span>
                      <span class="addr-name">{{ a.name }}</span>
                    </button>
                    <button class="act" :disabled="lanLoading" @click="refreshLan">
                      <i class="fas fa-sync-alt" aria-hidden="true"></i> {{ t("Refresh") }}
                    </button>
                  </div>
                </div>
                <p class="note">{{ t("Scan the QR in the Deckboard app or type the address manually.") }}</p>
              </template>
              <p v-else class="note">
                {{ t("Pulpit sees no local network - plug in Wi-Fi or Ethernet and refresh.") }}
              </p>
            </div>
          </article>

          <article class="tile" :class="[tileCls('v2'), { dead: !status.dbOk }]">
            <button
              class="hit"
              :tabindex="focused === 'v2' && status.dbOk ? -1 : 0"
              :aria-label="t('Expand: New v2 client')"
              @click="focused = 'v2'"
            ></button>
            <header class="tile-head">
              <h3>{{ t("New client (v2)") }}</h3>
              <span v-if="pairingOffer && status.dbOk" class="chip ok">{{ t("code alive") }} {{ pairingClock }}</span>
            </header>
            <p class="sum">{{ t("The native Pulpit client - a one-time code instead of an open QR.") }}</p>
            <div v-show="focused === 'v2' && status.dbOk" class="detail">
              <template v-if="!pairingOffer">
                <button class="act primary" :disabled="pairingBusy" @click="generatePairing">
                  <i class="fas fa-key" aria-hidden="true"></i> {{ t("Generate pairing code") }}
                </button>
                <p class="note">{{ t("The code works for 5 minutes and burns after the first use.") }}</p>
              </template>
              <template v-else>
                <div class="code mono tnum">{{ pairingOffer.code }}</div>
                <p class="note">{{ t("Type the code into the new client together with the address. Expires in {time}.", { time: pairingClock }) }}</p>
                <button class="act" :disabled="pairingBusy" @click="generatePairing">{{ t("New code") }}</button>
              </template>
              <p v-if="pairingError" class="err">{{ pairingError }}</p>
            </div>
          </article>

          <article class="tile" :class="[tileCls('devices'), { dead: !status.dbOk }]">
            <button
              class="hit"
              :tabindex="focused === 'devices' && status.dbOk ? -1 : 0"
              :aria-label="t('Expand: Trusted devices')"
              @click="focused = 'devices'"
            ></button>
            <header class="tile-head">
              <h3>{{ t("Trusted devices") }}</h3>
              <span v-if="status.dbOk" class="chip" :class="devices.length ? 'ok' : ''">
                {{ devices.length ? t("trusted: {n}", { n: devices.length }) : t("none") }}
              </span>
              <span v-else class="chip bad">{{ t("server off") }}</span>
            </header>
            <p class="sum">
              {{ t("Tablets with a stored token. A new device asks for trust on its first connection.") }}
            </p>
            <div v-show="focused === 'devices' && status.dbOk" class="detail">
              <p v-if="!devices.length" class="note">
                {{ t("No device is trusted yet - generate a pairing code above.") }}
              </p>
              <template v-else>
                <ul class="devices">
                  <li v-for="d in devices" :key="d.id" class="device">
                    <span class="device-name">{{ d.name }}</span>
                    <span class="device-seen">{{ t("seen: {time}", { time: lastSeenLabel(d.last_seen) }) }}</span>
                    <button class="act danger" :disabled="devicesBusy" @click="revokeDevice(d)">
                      <i class="fas fa-user-slash" aria-hidden="true"></i> {{ t("Revoke trust") }}
                    </button>
                  </li>
                </ul>
                <p class="note">
                  {{ t("Revoking removes the device's token and closes its connection. It only comes back through a new pairing. A trust question left unanswered expires together with the pairing code and rejects the tablet.") }}
                </p>
                <div class="ctl" style="margin-top: 10px">
                  <div class="ctl-text">
                    <span class="ctl-name">{{ t("APK install (sideload)") }}</span>
                    <span class="ctl-note">
                      {{ t("A tablet connected over USB with adb; the install keeps app data (adb install -r). \"Update tablet\" downloads the APK from the same feed as the desktop updates (GitHub), verifies sha256 and launches the app after the install. Devices: {devices}", { devices: adbList.length ? adbList.join(", ") : t("none") }) }}
                    </span>
                  </div>
                  <span style="display: flex; gap: 8px; flex: none">
                    <button class="act" :disabled="adbBusy" @click="refreshAdb">{{ t("Refresh") }}</button>
                    <button class="act" :disabled="adbBusy" @click="installApk">{{ t("Install APK…") }}</button>
                    <button class="act accent" :disabled="adbBusy" @click="updateTablet">{{ t("Update tablet") }}</button>
                  </span>
                </div>
                <p v-if="adbNote" :class="adbNoteBad ? 'err' : 'note pending'">{{ adbNote }}</p>
              </template>
            </div>
          </article>
        </div>
      </section>

      <section class="row" aria-labelledby="sec-aidev">
        <h2 id="sec-aidev">AI usage</h2>
        <div class="tiles">
          <article class="tile" :class="tileCls('aidev')">
            <button
              class="hit"
              :tabindex="focused === 'aidev' ? -1 : 0"
              :aria-label="t('Expand: AI usage')"
              @click="focused = 'aidev'"
            ></button>
            <header class="tile-head">
              <h3>AI usage</h3>
              <span class="chip" :class="aidevDetected.length ? 'ok' : ''">
                {{
                  aidevDetected.length
                    ? t("{shown} of {total} rows", { shown: aidevShow.length, total: aidevDetected.length })
                    : t("detecting…")
                }}
              </span>
            </header>
            <p class="sum">
              {{ t("The AI plan-limits tile: which rows to show and whether to have a summary line. Limits are detected automatically (GLM, Claude, Codex, OpenRouter).") }}
            </p>
            <div v-show="focused === 'aidev'" class="detail">
              <div class="ctl">
                <div class="ctl-text">
                  <span class="ctl-name">{{ t("Row identification") }}</span>
                  <span class="ctl-note">{{ t("A text name or the brand logo - never both at once.") }}</span>
                </div>
                <span class="seg">
                  <button
                    class="seg-btn"
                    :class="{ on: aidevRowStyle === 'name' }"
                    :disabled="aidevBusy"
                    @click="setAidevRowStyle('name')"
                  >{{ t("Name") }}</button>
                  <button
                    class="seg-btn"
                    :class="{ on: aidevRowStyle === 'logo' }"
                    :disabled="aidevBusy"
                    @click="setAidevRowStyle('logo')"
                  >{{ t("Logo") }}</button>
                </span>
              </div>
              <div class="ctl">
                <div class="ctl-text">
                  <span class="ctl-name">{{ t("Summary line") }}</span>
                  <span class="ctl-note">{{ t("The row under the list naming the window closest to its limit.") }}</span>
                </div>
                <button
                  class="switch"
                  role="switch"
                  :aria-checked="aidevSummary"
                  :disabled="aidevBusy"
                  @click="toggleAidevSummary"
                ><span class="knob"></span></button>
              </div>
              <template v-if="aidevGroups.length">
                <div v-for="g in aidevGroups" :key="g.name" class="aid-group">
                  <div class="aid-provider">{{ g.name }}</div>
                  <label v-for="row in g.rows" :key="row.id" class="aid-check">
                    <input
                      type="checkbox"
                      :checked="aidevChecked(row.id)"
                      :disabled="aidevBusy"
                      @change="toggleAidevRow(row.id)"
                    />
                    {{ row.label }}
                  </label>
                </div>
              </template>
              <p v-else class="note">
                {{ t("Detection needs one producer cycle - wait a moment and come back to this tab.") }}
              </p>
              <p class="note">
                {{ t("Unchecked rows disappear from the AI plan-limits tile. Changes save themselves and work without an app restart.") }}
              </p>
              <p v-if="aidevError" class="err">{{ aidevError }}</p>
              <p v-else-if="aidevNote" class="note pending">{{ aidevNote }}</p>
            </div>
          </article>
        </div>
      </section>

      <section class="row" aria-labelledby="sec-obs">
        <h2 id="sec-obs">OBS Studio</h2>
        <div class="tiles">
          <ObsSettings :expanded="focused === 'obs'" @focus="focused = 'obs'" />
        </div>
      </section>

      <section class="row" aria-labelledby="sec-spotify">
        <h2 id="sec-spotify">Spotify</h2>
        <div class="tiles">
          <article class="tile" :class="tileCls('spotify')">
            <button
              class="hit"
              :tabindex="focused === 'spotify' ? -1 : 0"
              :aria-label="t('Expand: Spotify')"
              @click="focused = 'spotify'"
            ></button>
            <header class="tile-head">
              <h3>Spotify</h3>
              <span class="chip" :class="spotifyChip.ok ? 'ok' : ''">{{ spotifyChip.text }}</span>
            </header>
            <p class="sum">
              {{ t("Playback control and now-playing tiles. Login through your own Spotify app (PKCE) - the credentials stay on this computer.") }}
            </p>
            <div v-show="focused === 'spotify'" class="detail">
              <div class="ctl">
                <div class="ctl-text">
                  <span class="ctl-name">Client ID</span>
                  <span class="ctl-note">{{ t("From your app on developer.spotify.com/dashboard.") }}</span>
                </div>
                <span class="spotify-id-line">
                  <input
                    v-model="spotifyClientIdDraft"
                    class="combo-input spotify-id-input"
                    placeholder="Client ID"
                    autocomplete="off"
                    spellcheck="false"
                    aria-label="Spotify Client ID"
                    @keyup.enter="saveSpotifyClientId"
                  />
                  <button class="act accent" :disabled="spotifyBusy" @click="saveSpotifyClientId">{{ t("Save") }}</button>
                </span>
              </div>
              <div class="ctl">
                <div class="ctl-text">
                  <span class="ctl-name">Redirect URI</span>
                  <span class="ctl-note mono">{{ spotify.redirectUri }}</span>
                  <span class="ctl-note">{{ t("Enter it exactly like that in your Spotify app's settings.") }}</span>
                </div>
                <button class="act" @click="copyRedirectUri">
                  <i class="fas fa-copy" aria-hidden="true"></i> {{ t("Copy") }}
                </button>
              </div>
              <div class="ctl">
                <div class="ctl-text">
                  <span class="ctl-name">{{ t("Account") }}</span>
                  <span class="ctl-note">{{ spotifyStatusLine }}</span>
                </div>
                <button
                  v-if="spotifyLoggedIn"
                  class="act danger"
                  :disabled="spotifyBusy"
                  @click="doSpotifyLogout"
                >{{ t("Log out") }}</button>
                <button v-else class="act primary" :disabled="spotifyBusy" @click="doSpotifyLogin">
                  {{ t("Log in with Spotify") }}
                </button>
              </div>
              <p v-if="spotifyError" class="err">{{ spotifyError }}</p>
              <p v-else-if="spotifyNote" class="note pending">{{ spotifyNote }}</p>
              <button class="act spotify-help-toggle" @click="spotifyHelpOpen = !spotifyHelpOpen">
                <i class="fas" :class="spotifyHelpOpen ? 'fa-chevron-down' : 'fa-chevron-right'" aria-hidden="true"></i>
                {{ t("How to create a Spotify app") }}
              </button>
              <ol v-if="spotifyHelpOpen" class="spotify-help">
                <li>
                  {{ t("Open") }} <span class="mono">developer.spotify.com/dashboard</span>{{ t(", log in and pick") }}
                  <b>Create app</b>.
                </li>
                <li>
                  {{ t("In the") }} <b>Redirect URI</b> {{ t("field enter exactly") }}
                  <span class="mono">{{ spotify.redirectUri }}</span>, {{ t("and pick") }}
                  <b>Web API</b>.
                </li>
                <li>
                  {{ t("Copy the") }} <b>Client ID</b> {{ t("from the app's settings and paste it into the field above.") }}
                </li>
                <li>
                  {{ t("Click") }} <b>{{ t("Log in with Spotify") }}</b> {{ t("- a browser opens, approve the access and come back to Pulpit. Playback control requires a Premium account; now-playing status works on Free too.") }}
                </li>
              </ol>
            </div>
          </article>
        </div>
      </section>

      <!-- integrations: fully self-contained settings tiles (components/settings) -->
      <section class="row" aria-labelledby="sec-integrations">
        <h2 id="sec-integrations">{{ t("Integrations") }}</h2>
        <div class="tiles">
          <VoicemeeterSettings
            :focused="focused === 'voicemeeter'"
            @expand="focused = 'voicemeeter'"
          />
          <DiscordSettings :focused="focused === 'discord'" @expand="focused = 'discord'" />
        </div>
      </section>
    </div>

    <!-- in-app revoke confirmation; replaces the native dialog popup -->
    <div
      v-if="confirmRevoke"
      class="veil"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="revoke-title"
    >
      <div class="confirm">
        <div class="confirm-icon">
          <i class="fas fa-user-slash" aria-hidden="true"></i>
        </div>
        <h2 id="revoke-title">{{ t("Revoke trust?") }}</h2>
        <p class="confirm-name">{{ confirmRevoke.name }}</p>
        <p class="confirm-warn">
          {{ t("The device's token will be removed and its connection closed immediately. The tablet only comes back through a new pairing.") }}
        </p>
        <p v-if="revokeError" class="err">{{ revokeError }}</p>
        <div class="confirm-actions">
          <button ref="cancelBtn" class="act" @click="confirmRevoke = null">{{ t("Cancel") }}</button>
          <button class="act danger-solid" :disabled="devicesBusy" @click="doRevoke">
            <i class="fas fa-user-slash" aria-hidden="true"></i> {{ t("Revoke device") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  --wall: #171c21;
  --tile: #262e36;
  --tile-2: #313a44;
  --line: #1d2a36;
  --ink: #ecf0f1;
  --ink-2: #95a5a6;
  --ink-3: #8b97a1;
  --ok: var(--accent);
  --bad: #e74c3c;
  --bad-ink: #f19488;
  position: fixed;
  inset: 0;
  z-index: 45;
  background: var(--wall);
  color: var(--ink);
  display: flex;
  flex-direction: column;
  user-select: none;
}

.bar {
  flex: none;
  height: 64px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 34px;
}
.bar h1 {
  font-size: 18px;
  font-weight: 600;
  letter-spacing: 0.2px;
  flex: 1;
}
.close {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  color: var(--ink-2);
  font-size: 15px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 140ms ease-out, color 140ms ease-out;
}
.close:hover { background: var(--tile-2); color: var(--ink); }
.close:active { transform: scale(0.96); }

/* UI language pick: same dark treatment as the editor's inputs */
.lang {
  flex: none;
  padding: 7px 8px;
  font-size: 12.5px;
  background: #171c21;
  border: 1px solid var(--line);
  border-radius: 6px;
  color: var(--ink);
}

.wall {
  flex: 1;
  overflow-y: auto;
  width: 100%;
  max-width: 1120px;
  margin: 0 auto;
  padding: 6px 34px 48px;
}

.row { margin-top: 30px; }
.row > h2 {
  font-size: 12.5px;
  font-weight: 600;
  letter-spacing: 1.6px;
  text-transform: uppercase;
  color: var(--ink-3);
  margin: 0 0 12px;
}

.tiles {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  align-items: stretch;
}

.tile {
  position: relative;
  flex: 1 1 300px;
  min-width: 240px;
  background: var(--tile);
  border: 1px solid var(--line);
  border-radius: 12px;
  padding: 18px 20px;
  transition:
    flex-grow 240ms cubic-bezier(0.2, 0, 0, 1),
    opacity 170ms ease-out,
    border-color 170ms ease-out,
    box-shadow 200ms ease-out;
}
.tile.focused {
  flex-grow: 2.4;
  border-color: color-mix(in srgb, var(--accent) 40%, var(--line));
  box-shadow: 0 14px 40px rgba(0, 0, 0, 0.5);
}
.tile.dim { opacity: 0.42; }
.tile.dim:hover { opacity: 0.62; border-color: #43505c; }
.tile.dead,
.tile.dead:hover { opacity: 0.42; }
.tile.dead .hit { display: none; }

/* the invisible hit zone covers an unexpanded tile so a click anywhere
   moves the focus; once expanded it steps aside for the real controls */
.hit {
  position: absolute;
  inset: 0;
  border-radius: inherit;
}
.tile.focused .hit { display: none; }

.tile-head {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 6px;
}
.tile-head h3 {
  font-size: 15px;
  font-weight: 600;
  flex: 1;
  min-width: 0;
}
.chip {
  flex: none;
  font-size: 11px;
  padding: 3px 9px;
  border-radius: 999px;
  background: var(--tile-2);
  color: var(--ink-2);
  white-space: nowrap;
}
.chip.ok { color: var(--ok); background: rgba(26, 188, 156, 0.14); }
.chip.bad { color: var(--bad-ink); background: rgba(231, 76, 60, 0.14); }

.sum {
  font-size: 12.5px;
  line-height: 1.55;
  color: var(--ink-2);
  margin: 0;
}

.detail { padding-top: 12px; animation: rise 200ms ease-out; }
@keyframes rise {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: none; }
}

.ctl {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 11px 0;
}
.ctl + .ctl { border-top: 1px solid var(--tile-2); }
.ctl-text { min-width: 0; }
.ctl-name { display: block; font-size: 13.5px; font-weight: 500; }
.ctl-note { display: block; font-size: 12px; color: var(--ink-3); margin-top: 2px; line-height: 1.5; }

/* AI usage: auto-detected row checkboxes, grouped per provider */
.aid-group {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px 14px;
  padding: 10px 0;
  border-top: 1px solid var(--tile-2);
}
.aid-provider {
  flex: none;
  min-width: 96px;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.4px;
  text-transform: uppercase;
  color: var(--ink-2);
}
.aid-check {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--ink);
  cursor: pointer;
  user-select: none;
}
.aid-check input { accent-color: var(--accent); width: 14px; height: 14px; cursor: pointer; }
.aid-check:hover { color: var(--ink-2); }

/* Spotify: client id line + the 4-step help */
.spotify-id-line { display: flex; align-items: center; gap: 8px; flex: none; }
.spotify-id-input { width: 260px; }
.spotify-help-toggle { margin-top: 12px; }
.spotify-help {
  margin: 10px 0 0;
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--ink-2);
}
.spotify-help b { color: var(--ink); font-weight: 600; }
.spotify-help .mono { font-size: 11.5px; background: var(--tile-2); padding: 1px 5px; border-radius: 4px; }

/* name | logo segmented pick */
.seg {
  flex: none;
  display: inline-flex;
  border: 1px solid var(--tile-2);
  border-radius: 8px;
  overflow: hidden;
}
.seg-btn {
  border: 0;
  background: transparent;
  color: var(--ink-3);
  font-size: 12px;
  font-weight: 600;
  padding: 6px 12px;
  cursor: pointer;
}
.seg-btn + .seg-btn { border-left: 1px solid var(--tile-2); }
.seg-btn.on { background: var(--tile-2); color: var(--ink); }
.seg-btn:disabled { cursor: default; opacity: 0.7; }
.locked {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  color: var(--ink-3);
}

.switch {
  flex: none;
  position: relative;
  width: 46px;
  height: 26px;
  border-radius: 13px;
  background: var(--tile-2);
  transition: background 160ms ease-out;
}
.switch:hover { background: #3a4753; }
.switch[aria-checked="true"] { background: var(--accent); }
.switch[aria-checked="true"]:hover { background: var(--accent-2); }
.switch .knob {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #f2f4f6;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
  transition: transform 160ms cubic-bezier(0.2, 0, 0, 1);
}
.switch[aria-checked="true"] .knob { transform: translateX(20px); }

.combo-line { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.combo {
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  font-size: 13px;
  background: var(--tile-2);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 7px 12px;
}
.combo-input {
  flex: 1;
  min-width: 120px;
  max-width: 200px;
  padding: 7px 10px;
  font-size: 13px;
  background: #171c21;
  border: 1px solid var(--line);
  border-radius: 6px;
  color: var(--ink);
}
.combo-input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}

.act {
  flex: none;
  font-size: 12.5px;
  padding: 7px 12px;
  border-radius: 6px;
  background: var(--tile-2);
  color: var(--ink);
  transition: background 140ms ease-out;
}
.act:hover { background: #3a4753; }
.act:disabled { opacity: 0.45; }
.act.accent { color: var(--accent); font-weight: 500; }
.act.primary {
  background: var(--accent);
  color: #0c2a24;
  font-weight: 600;
  padding: 9px 14px;
}
.act.primary:hover { background: var(--accent-2); }
.act.danger { color: var(--bad-ink); }
.act.danger:hover { background: rgba(231, 76, 60, 0.18); }

.err { font-size: 12px; color: var(--bad-ink); margin: 8px 0 0; overflow-wrap: anywhere; }

.facts {
  margin: 0;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px 24px;
}
.facts dt { font-size: 11.5px; color: var(--ink-3); margin-bottom: 2px; }
.facts dd { margin: 0; font-size: 13.5px; }

.big { font-size: 30px; font-weight: 700; line-height: 1.1; }
.note { font-size: 12px; line-height: 1.6; color: var(--ink-3); margin: 10px 0 0; }
.note code { padding: 1px 5px; border-radius: 4px; background: var(--tile-2); font-size: 11.5px; }

.pair { display: flex; gap: 14px; align-items: flex-start; }

.devices {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.device {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 7px 9px;
  border-radius: 6px;
  background: var(--tile-2);
}
.device-name {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.device-seen { flex: none; font-size: 11.5px; color: var(--ink-3); }
.qr {
  flex: none;
  width: 108px;
  height: 108px;
  background: #fff;
  border-radius: 8px;
  padding: 6px;
}
.addr-list { display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0; }
.addr {
  text-align: left;
  padding: 6px 9px;
  border-radius: 6px;
  display: flex;
  flex-direction: column;
  font-size: 13px;
  transition: background 140ms ease-out;
}
.addr:hover, .addr.sel { background: var(--tile-2); }
.addr.sel .mono { font-weight: 700; }
.addr-name {
  font-size: 11.5px;
  color: var(--ink-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.code {
  display: inline-block;
  font-size: 30px;
  font-weight: 700;
  letter-spacing: 6px;
  line-height: 1;
  background: #171c21;
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 14px 18px;
}

.mono { font-family: ui-monospace, "Cascadia Mono", Consolas, monospace; }

.port-line { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.port-input {
  width: 110px;
  padding: 7px 10px;
  font-size: 14px;
  background: #171c21;
  border: 1px solid var(--line);
  border-radius: 6px;
  color: var(--ink);
}
.port-input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}
.note.pending { color: var(--ok); }

.veil {
  position: absolute;
  inset: 0;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(10, 14, 17, 0.62);
  backdrop-filter: blur(2px);
  animation: fade 140ms ease-out;
}
@keyframes fade {
  from { opacity: 0; }
  to { opacity: 1; }
}
.confirm {
  width: min(380px, calc(100vw - 80px));
  background: var(--tile);
  border: 1px solid rgba(231, 76, 60, 0.35);
  border-radius: 14px;
  padding: 26px 26px 22px;
  text-align: center;
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
  animation: rise 200ms ease-out;
}
.confirm-icon {
  width: 52px;
  height: 52px;
  margin: 0 auto 12px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  color: var(--bad-ink);
  background: rgba(231, 76, 60, 0.16);
}
.confirm h2 {
  font-size: 17px;
  font-weight: 600;
  margin: 0 0 4px;
}
.confirm-name {
  font-size: 14px;
  font-weight: 600;
  margin: 0;
  overflow-wrap: anywhere;
}
.confirm-warn {
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--ink-2);
  margin: 8px 0 0;
}
.confirm-actions {
  display: flex;
  justify-content: center;
  gap: 10px;
  margin-top: 20px;
}
.act.danger-solid {
  background: var(--bad);
  color: #fff;
  font-weight: 600;
}
.act.danger-solid:hover { background: #c0392b; }
.act.danger-solid:disabled { opacity: 0.55; }
</style>
