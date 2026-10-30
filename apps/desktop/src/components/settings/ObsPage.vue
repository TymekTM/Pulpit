<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api } from "../../api";
import { t } from "../../i18n";

// OBS Studio over obs-websocket v5. The enable switch applies at once;
// the connection fields apply on Save. The password goes to the backend
// once and never comes back - the status only says whether one is stored.
const status = ref({
  loaded: false,
  enabled: false,
  connected: false,
  authFailed: false,
  version: null,
  host: "127.0.0.1",
  port: 4455,
  hasPassword: false,
});

const hostDraft = ref("127.0.0.1");
const portDraft = ref("4455");
const passwordDraft = ref("");

const busy = ref(false);
const error = ref("");
const note = ref("");
const noteBad = ref(false);

function flash(text, bad = false) {
  note.value = text;
  noteBad.value = bad;
  if (!bad) {
    setTimeout(() => {
      if (note.value === text) note.value = "";
    }, 4000);
  }
}

async function refreshStatus() {
  try {
    const s = await api.obsStatus();
    const first = !status.value.loaded;
    status.value = { ...s, loaded: true };
    error.value = "";
    if (first) {
      hostDraft.value = s.host || "127.0.0.1";
      portDraft.value = String(s.port ?? 4455);
    }
  } catch (e) {
    error.value = e ? String(e) : t("Could not load the OBS status.");
  }
}

const chip = computed(() => {
  if (!status.value.enabled) return { text: t("off"), cls: "" };
  if (status.value.connected) return { text: t("connected"), cls: "ok" };
  if (status.value.authFailed) return { text: t("wrong password"), cls: "bad" };
  return { text: t("no connection"), cls: "bad" };
});

const statusLine = computed(() => {
  const s = status.value;
  if (!s.enabled) return t("Off - Pulpit does not connect to OBS and OBS tiles do nothing.");
  if (s.connected) {
    return s.version
      ? t("Connected to OBS {version}.", { version: s.version })
      : t("Connected to OBS.");
  }
  if (s.authFailed) return t("OBS rejected the password. Copy it again from Tools → WebSocket Server Settings.");
  return t("Cannot reach OBS at {host}:{port}. Is OBS running with the WebSocket server enabled?", {
    host: s.host,
    port: s.port,
  });
});

function parsedPort() {
  const port = Number(String(portDraft.value).trim());
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw t("The port must be a number between 1 and 65535.");
  }
  return port;
}

async function toggleEnabled() {
  busy.value = true;
  error.value = "";
  const next = !status.value.enabled;
  try {
    // the stored connection stays as is; null keeps the stored password
    await api.obsApplyConfig(next, status.value.host, status.value.port, null);
    status.value = { ...status.value, enabled: next };
    await refreshStatus();
  } catch (e) {
    error.value = e ? String(e) : t("Could not save the OBS settings.");
  } finally {
    busy.value = false;
  }
}

async function save() {
  busy.value = true;
  error.value = "";
  try {
    const port = parsedPort();
    // empty field keeps the stored password; "Remove" clears it
    const password = passwordDraft.value ? passwordDraft.value : null;
    await api.obsApplyConfig(status.value.enabled, hostDraft.value.trim(), port, password);
    passwordDraft.value = "";
    flash(t("Saved."));
    await refreshStatus();
  } catch (e) {
    error.value = e ? String(e) : t("Could not save the OBS settings.");
  } finally {
    busy.value = false;
  }
}

async function clearPassword() {
  busy.value = true;
  error.value = "";
  try {
    await api.obsApplyConfig(status.value.enabled, status.value.host, status.value.port, "");
    flash(t("Password removed."));
    await refreshStatus();
  } catch (e) {
    error.value = e ? String(e) : t("Could not save the OBS settings.");
  } finally {
    busy.value = false;
  }
}

async function testConnection() {
  busy.value = true;
  error.value = "";
  flash(t("Connecting…"));
  try {
    const port = parsedPort();
    const password = passwordDraft.value ? passwordDraft.value : null;
    const version = await api.obsTestConnection(hostDraft.value.trim(), port, password);
    flash(t("Works - OBS {version} answered.", { version }));
  } catch (e) {
    flash(e ? String(e) : t("The connection failed."), true);
  } finally {
    busy.value = false;
  }
}

// the worker flips connected / auth-failed on its own: follow it live
let poll = null;
onMounted(() => {
  refreshStatus();
  poll = setInterval(refreshStatus, 3000);
});
onUnmounted(() => {
  if (poll) clearInterval(poll);
});
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2>OBS Studio</h2>
      <span class="chip" :class="chip.cls">{{ chip.text }}</span>
    </header>
    <p class="lead">{{ t("Scenes, sources, audio, recording and streaming from tiles. Needs OBS 28 or newer.") }}</p>

    <section class="group">
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Connect to OBS") }}</span>
            <span class="row-note">{{ statusLine }}</span>
          </div>
          <button
            class="switch"
            role="switch"
            :aria-checked="status.enabled"
            :aria-label="t('Connect to OBS')"
            :disabled="busy || !status.loaded"
            @click="toggleEnabled"
          ><span class="knob"></span></button>
        </div>
      </div>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Connection") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Address") }}</span>
            <span class="row-note">{{ t("127.0.0.1 when OBS runs on this computer.") }}</span>
          </div>
          <div class="row-ctl">
            <input
              v-model="hostDraft"
              class="field mono"
              placeholder="127.0.0.1"
              autocomplete="off"
              spellcheck="false"
              :aria-label="t('OBS address')"
            />
            <input
              v-model="portDraft"
              class="field narrow mono"
              inputmode="numeric"
              placeholder="4455"
              autocomplete="off"
              :aria-label="t('OBS port')"
            />
          </div>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Password") }}</span>
            <span class="row-note">
              {{ status.hasPassword
                ? t("A password is saved - type a new one to replace it.")
                : t("In OBS: Tools → WebSocket Server Settings → Show Connect Info.") }}
            </span>
          </div>
          <div class="row-ctl">
            <input
              v-model="passwordDraft"
              type="password"
              class="field"
              :placeholder="status.hasPassword ? '••••••••' : t('none')"
              autocomplete="new-password"
              :aria-label="t('OBS password')"
              @keyup.enter="save"
            />
            <button v-if="status.hasPassword" class="act ghost" :disabled="busy" @click="clearPassword">
              {{ t("Remove") }}
            </button>
          </div>
        </div>
        <div class="row">
          <div class="row-text"></div>
          <div class="row-ctl">
            <button class="act" :disabled="busy" @click="testConnection">{{ t("Test") }}</button>
            <button class="act accent" :disabled="busy" @click="save">{{ t("Save") }}</button>
          </div>
        </div>
      </div>
      <p v-if="error" class="err">{{ error }}</p>
      <p v-else-if="note" :class="noteBad ? 'err' : 'note ok'">{{ note }}</p>
    </section>
  </div>
</template>
