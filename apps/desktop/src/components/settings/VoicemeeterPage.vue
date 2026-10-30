<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, refreshVmDevices } from "../../api";
import { t } from "../../i18n";

// Voicemeeter through its Remote API DLL. The status query logs into the
// DLL, so it only runs (and polls) while this page is open.
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
const loaded = ref(false);
const busy = ref(false);
const error = ref("");
const note = ref("");
const advancedOpen = ref(false);

// VBVMR_RunVoicemeeter type codes; picked while Voicemeeter is closed,
// so the probe cannot have detected the edition yet
const RUN_TYPES = [
  { value: 1, label: "Voicemeeter" },
  { value: 2, label: "Banana" },
  { value: 3, label: "Potato" },
];
const runType = ref(2);

const chip = computed(() => {
  if (!loaded.value) return { text: t("checking…"), cls: "" };
  if (!status.value.installed) return { text: t("not installed"), cls: "" };
  if (status.value.logged_in && status.value.vm_type) return { text: t("connected"), cls: "ok" };
  return { text: t("not running"), cls: "" };
});

const statusLine = computed(() => {
  const s = status.value;
  if (!loaded.value) return t("Checking...");
  if (!s.installed) return t("Voicemeeter was not found on this computer. Installed somewhere unusual? Point at the DLL below.");
  if (s.logged_in && s.vm_type) {
    return t("{type} {version} - {strips} inputs, {buses} outputs.", {
      type: s.vm_type,
      version: s.version || "",
      strips: s.strip_count,
      buses: s.bus_count,
    });
  }
  return t("Voicemeeter is installed but not running.");
});

function flash(text) {
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
  } finally {
    loaded.value = true;
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
    flash(path ? t("DLL path saved.") : t("Back to automatic detection."));
  } catch (e) {
    error.value = e ? String(e) : t("Could not save the DLL path.");
  } finally {
    await refresh();
    refreshVmDevices();
    busy.value = false;
  }
}

async function reconnect() {
  busy.value = true;
  error.value = "";
  try {
    await api.vmReconnect();
    flash(t("Connected to Voicemeeter."));
  } catch (e) {
    error.value = e ? String(e) : t("Connecting to Voicemeeter failed.");
  } finally {
    await refresh();
    refreshVmDevices();
    busy.value = false;
  }
}

let startTimer = null;

async function runVoicemeeter() {
  busy.value = true;
  error.value = "";
  try {
    await api.vmRun(runType.value);
    flash(t("Starting Voicemeeter..."));
    // the engine needs a moment before the probe can see it
    startTimer = setTimeout(() => {
      startTimer = null;
      refresh().then(refreshVmDevices);
    }, 2500);
  } catch (e) {
    error.value = e ? String(e) : t("Could not start Voicemeeter.");
  } finally {
    busy.value = false;
  }
}

let poll = null;
onMounted(() => {
  refresh();
  poll = setInterval(refresh, 8000);
});
onUnmounted(() => {
  if (poll) clearInterval(poll);
  if (startTimer) clearTimeout(startTimer);
});
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2>Voicemeeter</h2>
      <span class="chip" :class="chip.cls">{{ chip.text }}</span>
    </header>
    <p class="lead">{{ t("Volume, mute and routing tiles for Voicemeeter strips and buses. Once connected, the tile editor shows your own strip and bus names.") }}</p>

    <section class="group">
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Status") }}</span>
            <span class="row-note">{{ statusLine }}</span>
          </div>
          <div class="row-ctl">
            <template v-if="status.installed && !status.logged_in">
              <select v-model.number="runType" class="select" :aria-label="t('Voicemeeter edition')">
                <option v-for="rt in RUN_TYPES" :key="rt.value" :value="rt.value">{{ rt.label }}</option>
              </select>
              <button class="act accent" :disabled="busy" @click="runVoicemeeter">{{ t("Start") }}</button>
            </template>
            <button v-if="status.installed" class="act" :disabled="busy" @click="reconnect">{{ t("Reconnect") }}</button>
          </div>
        </div>
      </div>
      <p v-if="error" class="err">{{ error }}</p>
      <p v-else-if="note" class="note ok">{{ note }}</p>
    </section>

    <section class="group">
      <button class="act ghost" :aria-expanded="advancedOpen || !status.installed" @click="advancedOpen = !advancedOpen">
        <i class="fas" :class="advancedOpen || !status.installed ? 'fa-chevron-down' : 'fa-chevron-right'" aria-hidden="true"></i>
        {{ t("Advanced") }}
      </button>
      <div v-if="advancedOpen || !status.installed" class="card" style="margin-top: 10px">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Remote API DLL") }}</span>
            <span class="row-note mono">{{ status.dll_path || t("not found") }}</span>
            <span class="row-note">
              {{ status.dll_override ? t("Chosen by hand.") : t("Found automatically in Program Files.") }}
            </span>
          </div>
          <div class="row-ctl">
            <button class="act" :disabled="busy" @click="pickDll">{{ t("Choose…") }}</button>
            <button v-if="status.dll_override" class="act ghost" :disabled="busy" @click="setOverride('')">
              {{ t("Automatic") }}
            </button>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>
