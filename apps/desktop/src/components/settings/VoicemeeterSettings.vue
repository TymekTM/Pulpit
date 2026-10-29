<script setup>
// Voicemeeter settings tile (Integracje). Self-contained like
// DiscordSettings: SettingsOverlay only mounts it and routes the wall's
// one-expanded-tile focus through the focused/expand pair. Status probing
// opens the remote DLL (its login launches Voicemeeter when closed), so
// the panel only polls while focused.
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, refreshVmDevices } from "../../api";
import { t } from "../../i18n";

const props = defineProps({
  focused: { type: Boolean, default: false },
});
const emit = defineEmits(["expand"]);

const emptyStatus = {
  installed: false,
  dll_path: null,
  logged_in: false,
  vm_type: null,
  version: null,
  strip_count: 0,
  bus_count: 0,
  dll_override: null,
};

const status = ref({ ...emptyStatus });
const busy = ref(false);
const error = ref("");
const note = ref("");

// VBVMR_RunVoicemeeter type codes (1 Basic, 2 Banana, 3 Potato); picked
// when Voicemeeter is closed, so the probe cannot have detected it yet
const RUN_TYPES = [
  { value: 1, label: "Voicemeeter" },
  { value: 2, label: "Banana" },
  { value: 3, label: "Potato" },
];
const runType = ref(2);

let pollTimer = null;

const chip = computed(() => {
  if (!status.value.installed) {
    return { text: t("no DLL"), ok: false };
  }
  if (status.value.logged_in) {
    return { text: status.value.vm_type || t("connected"), ok: true };
  }
  return { text: t("not connected"), ok: false };
});

const facts = computed(() => {
  const s = status.value;
  return [
    { dt: "DLL", dd: s.dll_path || t("not found (is Voicemeeter installed?)") },
    { dt: t("Version"), dd: s.version || "—" },
    { dt: t("Type"), dd: s.vm_type || "—" },
    {
      dt: t("Inputs / outputs"),
      dd:
        s.logged_in && s.vm_type
          ? t("{strips} strips / {buses} buses", { strips: s.strip_count, buses: s.bus_count })
          : "—",
    },
  ];
});

function flashNote(text) {
  note.value = text;
  setTimeout(() => {
    if (note.value === text) note.value = "";
  }, 3500);
}

async function refresh() {
  try {
    status.value = (await api.vmStatus()) || { ...emptyStatus };
  } catch (e) {
    error.value = e ? String(e) : t("Could not load the Voicemeeter status.");
  }
}

async function pickDll() {
  const path = await open({
    multiple: false,
    filters: [{ name: "VoicemeeterRemote DLL", extensions: ["dll"] }],
  });
  if (!path) return;
  await setOverride(String(path));
}

async function setOverride(path) {
  busy.value = true;
  error.value = "";
  try {
    await api.vmSetDllOverride(path);
    flashNote(path ? t("DLL path saved.") : t("DLL path cleared."));
    await refresh();
    refreshVmDevices();
  } catch (e) {
    error.value = e ? String(e) : t("Could not save the DLL path.");
  } finally {
    busy.value = false;
  }
}

async function reconnect() {
  busy.value = true;
  error.value = "";
  try {
    await api.vmReconnect();
    flashNote(t("Connected to Voicemeeter."));
    await refresh();
    refreshVmDevices();
  } catch (e) {
    error.value = e ? String(e) : t("Connecting to Voicemeeter failed.");
    await refresh();
  } finally {
    busy.value = false;
  }
}

async function runVoicemeeter() {
  busy.value = true;
  error.value = "";
  try {
    await api.vmRun(runType.value);
    flashNote(t("Starting Voicemeeter - the status refreshes in a moment."));
    // the engine needs a moment to come up before the probe reports it
    setTimeout(() => {
      if (props.focused) refresh();
    }, 2500);
  } catch (e) {
    error.value = e ? String(e) : t("Could not start Voicemeeter.");
  } finally {
    busy.value = false;
  }
}

function startPolling() {
  stopPolling();
  pollTimer = setInterval(refresh, 8000);
}
function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

onMounted(() => {
  refresh();
  if (props.focused) startPolling();
});
watch(
  () => props.focused,
  (open) => {
    if (open) {
      refresh();
      startPolling();
    } else {
      stopPolling();
    }
  }
);
onUnmounted(stopPolling);
</script>

<template>
  <article class="tile" :class="{ focused, dim: !focused }">
    <button
      v-if="!focused"
      class="hit"
      tabindex="0"
      :aria-label="t('Expand: Voicemeeter')"
      @click="emit('expand')"
    ></button>
    <header class="tile-head">
      <h3>Voicemeeter</h3>
      <span class="chip" :class="chip.ok ? 'ok' : ''">{{ chip.text }}</span>
    </header>
    <p class="sum">
      {{ t("Volume, transitions and routing control through Voicemeeter Remote - the Voicemeeter tiles in the editor get live strip and bus labels.") }}
    </p>
    <div v-show="focused" class="detail">
      <dl class="facts">
        <div v-for="f in facts" :key="f.dt">
          <dt>{{ f.dt }}</dt>
          <dd class="fact-dd" :title="f.dd">{{ f.dd }}</dd>
        </div>
      </dl>

      <div class="ctl">
        <div class="ctl-text">
          <span class="ctl-name">{{ t("DLL path") }}</span>
          <span class="ctl-note">
            <template v-if="status.dll_override">
              {{ t("Overridden:") }} <span class="mono">{{ status.dll_override }}</span>
            </template>
            <template v-else>
              {{ t("Auto-detection (Program Files / Program Files (x86)). Point at the file only for a non-standard install.") }}
            </template>
          </span>
        </div>
        <span class="btn-row">
          <button class="act" :disabled="busy" @click="pickDll">{{ t("Browse…") }}</button>
          <button
            v-if="status.dll_override"
            class="act danger"
            :disabled="busy"
            @click="setOverride('')"
          >{{ t("Clear") }}</button>
        </span>
      </div>

      <div class="ctl">
        <div class="ctl-text">
          <span class="ctl-name">{{ t("Connection") }}</span>
          <span class="ctl-note">
            {{ status.logged_in ? t("Logged in to the remote API.") : t("Connect to read the type, version and labels.") }}
          </span>
        </div>
        <span class="btn-row">
          <button class="act" :disabled="busy" @click="reconnect">{{ t("Reconnect") }}</button>
          <template v-if="status.installed && !status.logged_in">
            <select v-model.number="runType" class="run-select" :aria-label="t('Voicemeeter type')">
              <option v-for="rt in RUN_TYPES" :key="rt.value" :value="rt.value">{{ rt.label }}</option>
            </select>
            <button class="act accent" :disabled="busy" @click="runVoicemeeter">
              {{ t("Start Voicemeeter") }}
            </button>
          </template>
        </span>
      </div>

      <p class="note">
        {{ t("Strip and bus counts depend on the edition: Voicemeeter 3/2, Banana 5/5, Potato 8/8. Labels from the Labels tab show in the tile editor once connected.") }}
      </p>
      <p v-if="error" class="err">{{ error }}</p>
      <p v-else-if="note" class="note pending">{{ note }}</p>
    </div>
  </article>
</template>

<style scoped>
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

/* invisible hit zone over an unexpanded tile; steps aside once focused */
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

.facts {
  margin: 0 0 6px;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 24px;
}
.facts dt { font-size: 11.5px; color: var(--ink-3); margin-bottom: 2px; }
.facts dd { margin: 0; font-size: 13px; }
.fact-dd {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.btn-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: none;
}
.run-select {
  padding: 7px 8px;
  font-size: 12.5px;
  background: #171c21;
  border: 1px solid var(--line);
  border-radius: 6px;
  color: var(--ink);
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
.act.danger { color: var(--bad-ink); }
.act.danger:hover { background: rgba(231, 76, 60, 0.18); }

.err { font-size: 12px; color: var(--bad-ink); margin: 8px 0 0; overflow-wrap: anywhere; }
.note { font-size: 12px; line-height: 1.6; color: var(--ink-3); margin: 10px 0 0; }
.note.pending { color: var(--ok); }

.mono {
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  font-size: 11.5px;
  background: var(--tile-2);
  padding: 1px 5px;
  border-radius: 4px;
  overflow-wrap: anywhere;
}
</style>
