<script setup>
import { computed, onMounted, ref } from "vue";
import { api } from "../../api";
import { t } from "../../i18n";

// Discord through the local client's RPC with the user's own Discord app.
// Secrets never come back from the backend - only a "stored" flag - so an
// empty secret field means "keep the saved one".
const DEVELOPERS_URL = "https://discord.com/developers/applications";
const REDIRECT_URI = "https://discord.com";

const status = ref({
  configured: false,
  clientId: "",
  hasSecret: false,
  statusType: "not_configured",
  username: null,
  error: null,
});
const loaded = ref(false);

const clientIdDraft = ref("");
const secretDraft = ref("");
const showSecret = ref(false);
const busy = ref(false);
const authorizing = ref(false);
const error = ref("");
const note = ref("");
const helpOpen = ref(false);

async function refresh() {
  try {
    const res = await api.discordStatus();
    status.value = { ...status.value, ...res };
    clientIdDraft.value = res.clientId || "";
    if (!res.clientId) helpOpen.value = true;
  } catch (e) {
    error.value = e ? String(e) : t("Could not load the Discord status.");
  } finally {
    loaded.value = true;
  }
}

const connected = computed(() => status.value.statusType === "connected");

const chip = computed(() => {
  if (!loaded.value) return { text: t("checking…"), cls: "" };
  switch (status.value.statusType) {
    case "connected":
      return { text: t("connected"), cls: "ok" };
    case "needs_auth":
      return { text: t("not authorized"), cls: "bad" };
    case "not_running":
      return { text: t("Discord closed"), cls: "" };
    case "error":
      return { text: t("error"), cls: "bad" };
    default:
      return { text: t("not set up"), cls: "" };
  }
});

const statusLine = computed(() => {
  if (authorizing.value) return t("Approve Pulpit in the Discord window...");
  if (!loaded.value) return t("Checking the Discord client...");
  const s = status.value;
  switch (s.statusType) {
    case "connected":
      return s.username ? t("Connected as {user}.", { user: s.username }) : t("Connected.");
    case "needs_auth":
      return t("The app is saved - click Connect and approve it in Discord.");
    case "not_running":
      return t("Discord is not running on this computer. Start it and refresh.");
    case "error":
      return s.error ? t("Error: {message}", { message: s.error }) : t("Discord reported an error.");
    default:
      return t("Not set up - follow the steps below.");
  }
});

const dirty = computed(
  () => clientIdDraft.value.trim() !== status.value.clientId || secretDraft.value.trim() !== "",
);
const canConnect = computed(
  () => clientIdDraft.value.trim() && (status.value.hasSecret || secretDraft.value.trim()),
);

function flash(text) {
  note.value = text;
  setTimeout(() => {
    if (note.value === text) note.value = "";
  }, 4000);
}

async function saveCredentials() {
  if (!clientIdDraft.value.trim()) {
    error.value = t("Enter the Client ID.");
    return false;
  }
  busy.value = true;
  error.value = "";
  try {
    await api.discordSaveConfig(clientIdDraft.value.trim(), secretDraft.value.trim());
    secretDraft.value = "";
    flash(t("Saved."));
    await refresh();
    return true;
  } catch (e) {
    error.value = e ? String(e) : t("Could not save the Discord configuration.");
    return false;
  } finally {
    busy.value = false;
  }
}

async function connect() {
  // unsaved fields would make the authorization use the old app
  if (dirty.value && !(await saveCredentials())) return;
  busy.value = true;
  authorizing.value = true;
  error.value = "";
  try {
    const res = await api.discordAuthorize();
    status.value = { ...status.value, statusType: res?.statusType || "connected", username: res?.username || null };
    flash(t("Connected."));
  } catch (e) {
    error.value = e ? String(e) : t("Authorization in the Discord app failed.");
    await refresh();
  } finally {
    authorizing.value = false;
    busy.value = false;
  }
}

async function disconnect() {
  busy.value = true;
  error.value = "";
  try {
    await api.discordDisconnect();
    flash(t("Disconnected. The app's Client ID and Secret were kept."));
    await refresh();
  } catch (e) {
    error.value = e ? String(e) : t("Could not disconnect Discord.");
  } finally {
    busy.value = false;
  }
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    flash(t("Copied."));
  } catch {
    error.value = t("Could not copy - copy it manually.");
  }
}

async function recheck() {
  busy.value = true;
  error.value = "";
  loaded.value = false;
  await refresh();
  busy.value = false;
}

onMounted(refresh);
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2>Discord</h2>
      <span class="chip" :class="chip.cls">{{ chip.text }}</span>
    </header>
    <p class="lead">{{ t("Mute, deafen and voice-channel tiles, through the Discord client running on this computer.") }}</p>

    <section class="group">
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Connection") }}</span>
            <span class="row-note">{{ statusLine }}</span>
          </div>
          <div class="row-ctl">
            <button class="act ghost" :disabled="busy" :title="t('Refresh')" :aria-label="t('Refresh')" @click="recheck">
              <i class="fas fa-sync-alt" aria-hidden="true"></i>
            </button>
            <button v-if="connected" class="act danger" :disabled="busy" @click="disconnect">{{ t("Disconnect") }}</button>
            <button v-else class="act primary" :disabled="busy || !canConnect" @click="connect">
              {{ t("Connect") }}
            </button>
          </div>
        </div>
      </div>
      <p v-if="error" class="err">{{ error }}</p>
      <p v-else-if="note" class="note ok">{{ note }}</p>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Discord app") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">Client ID</span>
          </div>
          <input
            v-model="clientIdDraft"
            class="field wide mono"
            :placeholder="t('e.g. 123456789012345678')"
            autocomplete="off"
            spellcheck="false"
            aria-label="Discord Client ID"
          />
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">Client Secret</span>
            <span v-if="status.hasSecret" class="row-note">{{ t("Saved - type a new one to replace it.") }}</span>
          </div>
          <div class="row-ctl">
            <input
              v-model="secretDraft"
              :type="showSecret ? 'text' : 'password'"
              class="field mono"
              :placeholder="status.hasSecret ? '••••••••' : 'Client Secret'"
              autocomplete="off"
              spellcheck="false"
              aria-label="Discord Client Secret"
            />
            <button class="act ghost" :aria-label="showSecret ? t('Hide') : t('Show')" @click="showSecret = !showSecret">
              <i class="fas" :class="showSecret ? 'fa-eye-slash' : 'fa-eye'" aria-hidden="true"></i>
            </button>
          </div>
        </div>
        <div v-if="dirty" class="row">
          <div class="row-text"></div>
          <button class="act accent" :disabled="busy" @click="saveCredentials">{{ t("Save") }}</button>
        </div>
      </div>

      <button class="act ghost help-toggle" :aria-expanded="helpOpen" @click="helpOpen = !helpOpen">
        <i class="fas" :class="helpOpen ? 'fa-chevron-down' : 'fa-chevron-right'" aria-hidden="true"></i>
        {{ t("How to create a Discord app") }}
      </button>
      <ol v-if="helpOpen" class="help">
        <li>
          {{ t("Open the Discord developer portal and click New Application (any name, e.g. Pulpit).") }}
          <button class="act ghost" @click="copyText(DEVELOPERS_URL)">{{ t("Copy link") }}</button>
        </li>
        <li>
          {{ t("In the OAuth2 tab add the redirect {uri}.", { uri: REDIRECT_URI }) }}
          <button class="act ghost" @click="copyText(REDIRECT_URI)">{{ t("Copy") }}</button>
        </li>
        <li>{{ t("Copy the Client ID and Client Secret from the OAuth2 tab into the fields above.") }}</li>
        <li>{{ t("With Discord running, click Connect and approve Pulpit in the Discord window.") }}</li>
      </ol>
    </section>
  </div>
</template>
