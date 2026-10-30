<script setup>
import { computed, onMounted, ref } from "vue";
import { api } from "../../api";
import { t } from "../../i18n";

// Spotify through the user's own Spotify app (BYO client id + PKCE
// login). The status combines spotify_status with the live customValues
// pushes (spotify-auth / spotify-device), so an expired login flips the
// page without reopening it.
const props = defineProps({
  customValues: { type: Object, default: () => ({}) },
});

const DEFAULT_REDIRECT = "http://127.0.0.1:8502/spotify/callback";

const spotify = ref({
  configured: true,
  clientId: "",
  loggedIn: false,
  user: null,
  product: null,
  redirectUri: DEFAULT_REDIRECT,
});
const clientIdDraft = ref("");
const busy = ref(false);
const error = ref("");
const note = ref("");
const helpOpen = ref(false);

async function refresh() {
  try {
    const s = await api.spotifyStatus();
    spotify.value = {
      configured: s.configured !== false,
      clientId: s.clientId || "",
      loggedIn: Boolean(s.loggedIn),
      user: s.user || null,
      product: s.product || null,
      redirectUri: s.redirectUri || DEFAULT_REDIRECT,
    };
    clientIdDraft.value = s.clientId || "";
    // a first-time setup starts with the steps open
    if (!s.clientId) helpOpen.value = true;
  } catch (e) {
    error.value = e ? String(e) : t("Could not load the Spotify status.");
  }
}

const authLive = computed(() => props.customValues["spotify-auth"] || null);
const auth = computed(() => authLive.value || (spotify.value.loggedIn ? "ok" : "needs-login"));
const loggedIn = computed(() => spotify.value.configured && auth.value === "ok");

const statusLine = computed(() => {
  if (!spotify.value.configured) return t("Spotify is disabled - the spotify.json file is corrupted.");
  if (!spotify.value.clientId) return t("Not set up - follow the steps below.");
  if (auth.value === "off") return t("Inactive - log in to use the Spotify tiles.");
  if (!loggedIn.value) return t("Not logged in, or the login expired.");
  const plan =
    spotify.value.product === "free" ? t("Free - controlling playback requires Premium") : "Premium";
  const base = spotify.value.user
    ? t("Logged in as {user} ({plan})", { user: spotify.value.user, plan })
    : t("Logged in ({plan})", { plan });
  const device = props.customValues["spotify-device"];
  return device ? t("{status} - device: {device}", { status: base, device }) : base;
});

const chip = computed(() => {
  if (!spotify.value.configured) return { text: t("error"), cls: "bad" };
  if (loggedIn.value) return { text: t("logged in"), cls: "ok" };
  if (!spotify.value.clientId) return { text: t("not set up"), cls: "" };
  return { text: t("login required"), cls: "bad" };
});

const clientIdDirty = computed(() => clientIdDraft.value.trim() !== spotify.value.clientId);

function flash(text) {
  note.value = text;
  setTimeout(() => {
    if (note.value === text) note.value = "";
  }, 4000);
}

async function saveClientId() {
  busy.value = true;
  error.value = "";
  try {
    await api.spotifySetClientId(clientIdDraft.value.trim());
    flash(t("Client ID saved."));
    await refresh();
  } catch (e) {
    error.value = e ? String(e) : t("Could not save the Client ID.");
  } finally {
    busy.value = false;
  }
}

async function login() {
  busy.value = true;
  error.value = "";
  try {
    // a pasted but unsaved id would make the login use the old one
    if (clientIdDirty.value) await api.spotifySetClientId(clientIdDraft.value.trim());
    flash(t("Opening the browser - log in and come back to Pulpit."));
    const r = await api.spotifyLogin();
    flash(r?.user ? t("Logged in as {user}.", { user: r.user }) : t("Logged in."));
    await refresh();
  } catch (e) {
    error.value = e ? String(e) : t("Spotify login failed.");
  } finally {
    busy.value = false;
  }
}

async function logout() {
  busy.value = true;
  error.value = "";
  try {
    await api.spotifyLogout();
    flash(t("Logged out. The Client ID was kept."));
    await refresh();
  } catch (e) {
    error.value = e ? String(e) : t("Could not log out.");
  } finally {
    busy.value = false;
  }
}

async function copyRedirectUri() {
  try {
    await navigator.clipboard.writeText(spotify.value.redirectUri);
    flash(t("Redirect URI copied."));
  } catch {
    error.value = t("Could not copy - select it and copy manually.");
  }
}

onMounted(refresh);
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2>Spotify</h2>
      <span class="chip" :class="chip.cls">{{ chip.text }}</span>
    </header>
    <p class="lead">{{ t("Playback control and now-playing tiles. Pulpit logs in through your own free Spotify developer app - the login stays on this computer.") }}</p>

    <section class="group">
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Account") }}</span>
            <span class="row-note">{{ statusLine }}</span>
          </div>
          <button v-if="loggedIn" class="act danger" :disabled="busy" @click="logout">{{ t("Log out") }}</button>
          <button
            v-else
            class="act primary"
            :disabled="busy || !spotify.configured || !clientIdDraft.trim()"
            @click="login"
          >
            {{ t("Log in") }}
          </button>
        </div>
      </div>
      <p v-if="error" class="err">{{ error }}</p>
      <p v-else-if="note" class="note ok">{{ note }}</p>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Spotify app") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">Client ID</span>
            <span class="row-note">{{ t("From your app on developer.spotify.com.") }}</span>
          </div>
          <div class="row-ctl">
            <input
              v-model="clientIdDraft"
              class="field wide mono"
              placeholder="Client ID"
              autocomplete="off"
              spellcheck="false"
              aria-label="Spotify Client ID"
              @keyup.enter="saveClientId"
            />
            <button v-if="clientIdDirty" class="act accent" :disabled="busy" @click="saveClientId">{{ t("Save") }}</button>
          </div>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">Redirect URI</span>
            <span class="row-note mono">{{ spotify.redirectUri }}</span>
          </div>
          <button class="act" @click="copyRedirectUri">
            <i class="fas fa-copy" aria-hidden="true"></i> {{ t("Copy") }}
          </button>
        </div>
      </div>

      <button class="act ghost help-toggle" :aria-expanded="helpOpen" @click="helpOpen = !helpOpen">
        <i class="fas" :class="helpOpen ? 'fa-chevron-down' : 'fa-chevron-right'" aria-hidden="true"></i>
        {{ t("How to create a Spotify app") }}
      </button>
      <ol v-if="helpOpen" class="help">
        <li>{{ t("Open developer.spotify.com/dashboard, log in and click Create app.") }}</li>
        <li>{{ t("Paste the Redirect URI from above, tick Web API and save.") }}</li>
        <li>{{ t("Copy the Client ID from the app's settings into the field above.") }}</li>
        <li>{{ t("Click Log in and approve the access in the browser. Controlling playback needs Premium; the now-playing tile works on Free too.") }}</li>
      </ol>
    </section>
  </div>
</template>
