<script setup>
import { computed, onMounted, ref } from "vue";
import { api } from "../../api";
import { t } from "../../i18n";

// Version + updates, server health and the data folder. Opening the page
// checks the update feed once, so "is there something newer" never needs
// a click.
const props = defineProps({
  status: { type: Object, required: true },
});

const version = computed(() => props.status.version || "");

const updateBusy = ref(false);
const updateNote = ref("");
const updateNoteBad = ref(false);
const updateReady = ref(""); // newer version on the feed
const installing = ref(false);

async function checkUpdates() {
  updateBusy.value = true;
  updateNote.value = t("Checking for updates...");
  updateNoteBad.value = false;
  updateReady.value = "";
  try {
    const r = await api.checkForUpdates();
    updateReady.value = r?.update_available ? r.latest : "";
    updateNote.value = r?.update_available
      ? t("Version {latest} is available.", { latest: r.latest })
      : t("You have the latest version.");
  } catch (e) {
    updateNote.value = e ? String(e) : t("Could not check for updates.");
    updateNoteBad.value = true;
  } finally {
    updateBusy.value = false;
  }
}

async function installUpdate() {
  updateBusy.value = true;
  installing.value = true;
  updateNote.value = t("Downloading and installing - Pulpit restarts by itself...");
  updateNoteBad.value = false;
  try {
    updateNote.value = await api.installUpdate();
    // the backend exits and relaunches; stay busy for the ride
  } catch (e) {
    updateNote.value = e ? String(e) : t("The update failed.");
    updateNoteBad.value = true;
    updateBusy.value = false;
    installing.value = false;
  }
}

const folderError = ref("");

async function openFolder(logs) {
  folderError.value = "";
  try {
    await api.openDataDir(logs);
  } catch (e) {
    folderError.value = e ? String(e) : t("Could not open the folder.");
  }
}

onMounted(checkUpdates);
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2>{{ t("About Pulpit") }}</h2>
    </header>

    <section class="group">
      <h3 class="group-title">{{ t("Version") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">Pulpit <span class="mono">{{ version }}</span></span>
            <span class="row-note" :class="{ bad: updateNoteBad }" role="status">{{ updateNote }}</span>
          </div>
          <div class="row-ctl">
            <button
              v-if="updateReady"
              class="act primary"
              :disabled="updateBusy"
              @click="installUpdate"
            >
              <i class="fas fa-download" aria-hidden="true"></i> {{ t("Install {version}", { version: updateReady }) }}
            </button>
            <button v-else class="act" :disabled="updateBusy" @click="checkUpdates">{{ t("Check again") }}</button>
          </div>
        </div>
      </div>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Server") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Status") }}</span>
            <span class="row-note">
              {{ status.dbOk
                ? t("Listening on port {port}.", { port: status.port })
                : t("The database is locked or missing. Close the original Deckboard app and start Pulpit again.") }}
            </span>
          </div>
          <span class="chip" :class="status.dbOk ? 'ok' : 'bad'">
            {{ status.dbOk ? t("running") : t("database unavailable") }}
          </span>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Connected tablets") }}</span>
          </div>
          <span class="mono">{{ status.clients || 0 }}</span>
        </div>
      </div>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Files") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Data folder") }}</span>
            <span class="row-note mono">~/pulpitApp</span>
          </div>
          <div class="row-ctl">
            <button class="act" @click="openFolder(false)">
              <i class="fas fa-folder-open" aria-hidden="true"></i> {{ t("Open") }}
            </button>
            <button class="act" @click="openFolder(true)">{{ t("Logs") }}</button>
          </div>
        </div>
      </div>
      <p v-if="folderError" class="err">{{ folderError }}</p>
    </section>
  </div>
</template>
