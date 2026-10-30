<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../../api";
import { ask } from "../../confirm";
import { t } from "../../i18n";

// Everything a tablet needs from this computer: the address to dial
// (plus the stock client's QR), pairing for the native client, the
// trusted-device list and the USB (adb) install path.
const props = defineProps({
  status: { type: Object, required: true },
});

const runningPort = computed(() => props.status.port || 8500);

// ---- addresses ----------------------------------------------------------
const lanAddresses = ref([]);
const activeAddress = ref(0);
const lanLoading = ref(false);

async function refreshLan() {
  lanLoading.value = true;
  try {
    lanAddresses.value = (await api.listLanAddresses()) || [];
  } catch {
    lanAddresses.value = [];
  } finally {
    if (activeAddress.value >= lanAddresses.value.length) activeAddress.value = 0;
    lanLoading.value = false;
  }
}

// ---- port: stored value applies on the next launch -----------------------
const storedPort = ref(null);
const portLocked = ref(false); // PULPIT_PORT env var wins over the store
const editingPort = ref(false);
const portDraft = ref("");
const portError = ref("");
const portBusy = ref(false);

const portPending = computed(() =>
  storedPort.value && storedPort.value !== runningPort.value ? storedPort.value : null,
);

function editPort() {
  portDraft.value = String(storedPort.value ?? runningPort.value);
  portError.value = "";
  editingPort.value = true;
}

async function savePort() {
  const value = Number(String(portDraft.value).trim());
  if (!String(portDraft.value).trim() || !Number.isInteger(value) || value < 1024 || value > 65535) {
    portError.value = t("The port must be a number between 1024 and 65535.");
    return;
  }
  portBusy.value = true;
  try {
    await api.setServerPort(value);
    storedPort.value = value;
    portError.value = "";
    editingPort.value = false;
  } catch (e) {
    portError.value = e ? String(e) : t("Could not save the port.");
  } finally {
    portBusy.value = false;
  }
}

// ---- pairing code for the native client ---------------------------------
const pairingOffer = ref(null); // {code, expires_in_secs}
const pairingBusy = ref(false);
const pairingLeft = ref(0);
const pairingError = ref("");
let pairingTimer = null;

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
    pairingError.value = e ? String(e) : t("Could not generate a pairing code.");
  } finally {
    pairingBusy.value = false;
  }
}

const pairingClock = computed(() => {
  const s = Math.max(0, pairingLeft.value);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
});

// ---- trusted devices ----------------------------------------------------
const devices = ref([]);
const devicesBusy = ref(false);
const revokeError = ref("");
let devicePoll = null;

async function refreshDevices() {
  try {
    devices.value = (await api.listDevices()) || [];
  } catch {
    // v2 stack unavailable: show none rather than stale rows
    devices.value = [];
  }
}

async function revokeDevice(device) {
  revokeError.value = "";
  const ok = await ask(
    t("The device's token will be removed and its connection closed immediately. The tablet only comes back through a new pairing."),
    {
      title: t("Revoke trust for {name}?", { name: device.name }),
      kind: "warning",
      okLabel: t("Revoke"),
      cancelLabel: t("Cancel"),
    },
  );
  if (!ok) return;
  devicesBusy.value = true;
  try {
    await api.revokeDevice(device.id);
  } catch (e) {
    revokeError.value = e ? String(e) : t("Could not revoke the device.");
  } finally {
    devicesBusy.value = false;
  }
  refreshDevices();
}

function lastSeenLabel(unixSecs) {
  if (!unixSecs) return t("never");
  const diff = Math.max(0, Date.now() / 1000 - unixSecs);
  if (diff < 90) return t("now");
  if (diff < 3600) return t("{n} min ago", { n: Math.floor(diff / 60) });
  if (diff < 48 * 3600) return t("{n} h ago", { n: Math.floor(diff / 3600) });
  return new Date(unixSecs * 1000).toLocaleDateString();
}

// ---- USB (adb) ------------------------------------------------------------
const adbList = ref([]);
const adbLoaded = ref(false);
const adbMissing = ref(""); // why the device list could not be read
const adbBusy = ref(false);
const adbNote = ref("");
const adbNoteBad = ref(false);

async function refreshAdb() {
  adbBusy.value = true;
  try {
    adbList.value = (await api.adbDevices()) || [];
    adbMissing.value = "";
  } catch (e) {
    adbList.value = [];
    adbMissing.value = e ? String(e) : "adb";
  } finally {
    adbLoaded.value = true;
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
  adbNote.value = t("Installing (app data is kept)...");
  adbNoteBad.value = false;
  try {
    adbNote.value = (await api.adbInstallApk(String(path))) || t("Done.");
  } catch (e) {
    adbNote.value = String(e);
    adbNoteBad.value = true;
  } finally {
    adbBusy.value = false;
  }
  refreshAdb();
}

// the desktop update feed also carries the release APK; the backend
// downloads it, checks the sha256 and installs it on every attached
// tablet that is behind
async function updateTablet() {
  adbBusy.value = true;
  adbNote.value = t("Downloading the APK from GitHub and installing it on the tablet...");
  adbNoteBad.value = false;
  try {
    adbNote.value = (await api.installTabletUpdate()) || t("Done.");
  } catch (e) {
    adbNote.value = String(e);
    adbNoteBad.value = true;
  } finally {
    adbBusy.value = false;
  }
  refreshAdb();
}

const readyAdbDevices = computed(() => adbList.value.filter((d) => d.endsWith("(device)")));

onMounted(async () => {
  try {
    const settings = await api.getSettings();
    storedPort.value = Number(settings?.port) || null;
    portLocked.value = Boolean(settings?.port_locked);
  } catch {
    /* the running port stands */
  }
  refreshLan();
  refreshDevices();
  refreshAdb();
  // a pairing approved in the popup lands while this page is open
  devicePoll = setInterval(refreshDevices, 4000);
});
onUnmounted(() => {
  stopPairingClock();
  if (devicePoll) clearInterval(devicePoll);
});
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2>{{ t("Tablets") }}</h2>
      <span v-if="!status.dbOk" class="chip bad">{{ t("server off") }}</span>
      <span v-else class="chip" :class="status.clients ? 'ok' : ''">
        {{ t("connected: {n}", { n: status.clients || 0 }) }}
      </span>
    </header>
    <p class="lead">{{ t("Tablets connect to this computer over the local network. Both the Pulpit app and the original Deckboard app work.") }}</p>

    <div v-if="!status.dbOk" class="banner" role="alert">
      <i class="fas fa-exclamation-triangle" aria-hidden="true"></i>
      <span>{{ t("The server is not running - the database is locked or missing. Close the original Deckboard app and start Pulpit again.") }}</span>
    </div>

    <section class="group">
      <h3 class="group-title">{{ t("Address") }}</h3>
      <div class="card">
        <div class="card-body">
          <div v-if="lanAddresses.length" class="pair">
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
                <span class="mono">{{ a.ipv4 }}:{{ runningPort }}</span>
                <span class="addr-name">{{ a.name }}</span>
              </button>
              <p class="note">{{ t("The QR is for the original Deckboard app. In the Pulpit app pick this computer from the list or type the address.") }}</p>
              <button class="act ghost addr-refresh" :disabled="lanLoading" @click="refreshLan">
                <i class="fas fa-sync-alt" aria-hidden="true"></i> {{ t("Refresh") }}
              </button>
            </div>
          </div>
          <template v-else>
            <p class="empty">{{ t("Pulpit sees no local network - connect Wi-Fi or Ethernet and refresh.") }}</p>
            <button class="act" :disabled="lanLoading" @click="refreshLan">
              <i class="fas fa-sync-alt" aria-hidden="true"></i> {{ t("Refresh") }}
            </button>
          </template>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Port") }}</span>
            <span v-if="portLocked" class="row-note">
              {{ t("Set by the PULPIT_PORT environment variable.") }}
            </span>
            <span v-else-if="portPending" class="row-note">
              {{ t("Port {port} is saved and takes effect after Pulpit restarts. Point the tablets at the new port.", { port: portPending }) }}
            </span>
            <span v-else class="row-note">{{ t("Change it only if 8500 is taken by another program.") }}</span>
          </div>
          <div class="row-ctl">
            <template v-if="editingPort">
              <input
                v-model="portDraft"
                class="field narrow mono"
                inputmode="numeric"
                autocomplete="off"
                :aria-label="t('New port')"
                @keyup.enter="savePort"
              />
              <button class="act accent" :disabled="portBusy" @click="savePort">{{ t("Save") }}</button>
              <button class="act ghost" @click="editingPort = false; portError = ''">{{ t("Cancel") }}</button>
            </template>
            <template v-else>
              <span class="mono">{{ runningPort }}</span>
              <button v-if="!portLocked" class="act" @click="editPort">{{ t("Change") }}</button>
            </template>
          </div>
        </div>
      </div>
      <p v-if="portError" class="err">{{ portError }}</p>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Pair the Pulpit app") }}</h3>
      <div class="card">
        <div class="row top">
          <div class="row-text">
            <span class="row-name">{{ t("Pairing code") }}</span>
            <span class="row-note">
              {{ t("Usually not needed: on the tablet pick this computer from the list and confirm the code shown on both screens. If the tablet does not see the computer, generate a code and type it in on the tablet.") }}
            </span>
          </div>
          <div class="row-ctl">
            <button
              v-if="!pairingOffer"
              class="act primary"
              :disabled="pairingBusy || !status.dbOk"
              @click="generatePairing"
            >
              <i class="fas fa-key" aria-hidden="true"></i> {{ t("Generate code") }}
            </button>
          </div>
        </div>
        <div v-if="pairingOffer" class="card-body">
          <div class="row-ctl">
            <span class="code">{{ pairingOffer.code }}</span>
            <span class="chip ok mono">{{ pairingClock }}</span>
            <button class="act ghost" :disabled="pairingBusy" @click="generatePairing">{{ t("New code") }}</button>
          </div>
          <p class="note">{{ t("Works once, for 5 minutes.") }}</p>
        </div>
      </div>
      <p v-if="pairingError" class="err">{{ pairingError }}</p>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Trusted devices") }}</h3>
      <div class="card">
        <ul v-if="devices.length" class="list">
          <li v-for="d in devices" :key="d.id" class="list-item">
            <i class="fas fa-tablet-alt" aria-hidden="true"></i>
            <span class="list-main">
              <span class="list-name">{{ d.name }}</span>
              <span class="list-sub">{{ t("last seen: {time}", { time: lastSeenLabel(d.last_seen) }) }}</span>
            </span>
            <button class="act danger" :disabled="devicesBusy" @click="revokeDevice(d)">{{ t("Revoke") }}</button>
          </li>
        </ul>
        <p v-else class="empty">{{ t("No paired tablets yet. The original Deckboard app connects without pairing and is not listed here.") }}</p>
      </div>
      <p v-if="revokeError" class="err">{{ revokeError }}</p>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Install over USB") }}</h3>
      <div class="card">
        <div class="row top">
          <div class="row-text">
            <span class="row-name">{{ t("Connected over USB") }}</span>
            <span v-if="!adbLoaded" class="row-note">{{ t("Checking...") }}</span>
            <span v-else-if="adbMissing" class="row-note">
              {{ t("adb is not available - install Android platform-tools and add it to PATH.") }}
            </span>
            <span v-else-if="adbList.length" class="row-note mono">{{ adbList.join(", ") }}</span>
            <span v-else class="row-note">{{ t("No tablet. Connect it with a cable and allow USB debugging.") }}</span>
          </div>
          <button class="act ghost" :disabled="adbBusy" @click="refreshAdb">
            <i class="fas fa-sync-alt" aria-hidden="true"></i> {{ t("Refresh") }}
          </button>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Pulpit app on the tablet") }}</span>
            <span class="row-note">{{ t("Installs the latest release from GitHub, or an APK file you pick. Settings and pairing on the tablet are kept.") }}</span>
          </div>
          <div class="row-ctl">
            <button class="act" :disabled="adbBusy || !readyAdbDevices.length" @click="installApk">{{ t("APK file…") }}</button>
            <button class="act accent" :disabled="adbBusy || !readyAdbDevices.length" @click="updateTablet">
              {{ t("Install latest") }}
            </button>
          </div>
        </div>
      </div>
      <p v-if="adbNote" :class="adbNoteBad ? 'err' : 'note ok pre'">{{ adbNote }}</p>
    </section>
  </div>
</template>
