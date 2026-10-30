<script>
// The page the overlay reopens on: remembered for the session, so
// closing Settings to try a change and coming back lands on the same page.
let lastPage = "general";
</script>

<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { t } from "../i18n";
import "./settings/settings.css";
import GeneralPage from "./settings/GeneralPage.vue";
import TabletsPage from "./settings/TabletsPage.vue";
import AboutPage from "./settings/AboutPage.vue";
import ObsPage from "./settings/ObsPage.vue";
import SpotifyPage from "./settings/SpotifyPage.vue";
import DiscordPage from "./settings/DiscordPage.vue";
import VoicemeeterPage from "./settings/VoicemeeterPage.vue";
import AidevPage from "./settings/AidevPage.vue";

// Full-screen settings: a side nav of pages, one page shown at a time.
// Pages are mounted only while shown, so an integration's status probe
// (Discord's pipe, the Voicemeeter DLL login, the OBS poll) runs only
// when the user actually opens that page.
const props = defineProps({
  status: { type: Object, required: true },
  // live pushed values (APP_CUSTOM_VALUE mirror) - Spotify's auth state
  customValues: { type: Object, default: () => ({}) },
  // opens on this page instead of the remembered one
  initialPage: { type: String, default: null },
});
const emit = defineEmits(["close"]);

const PAGES = [
  { id: "general", icon: "fas fa-sliders-h", label: () => t("General"), component: GeneralPage },
  { id: "tablets", icon: "fas fa-tablet-alt", label: () => t("Tablets"), component: TabletsPage },
  { id: "obs", icon: "fas fa-video", label: () => "OBS Studio", component: ObsPage, integration: true },
  { id: "spotify", icon: "fas fa-music", label: () => "Spotify", component: SpotifyPage, integration: true },
  { id: "discord", icon: "fas fa-headset", label: () => "Discord", component: DiscordPage, integration: true },
  { id: "voicemeeter", icon: "fas fa-wave-square", label: () => "Voicemeeter", component: VoicemeeterPage, integration: true },
  { id: "aidev", icon: "fas fa-robot", label: () => t("AI usage"), component: AidevPage, integration: true },
  { id: "about", icon: "fas fa-info-circle", label: () => t("About Pulpit"), component: AboutPage },
];

const corePages = PAGES.filter((p) => !p.integration && p.id !== "about");
const integrationPages = PAGES.filter((p) => p.integration);
const aboutPage = PAGES.find((p) => p.id === "about");

const known = (id) => PAGES.some((p) => p.id === id);
const page = ref(known(props.initialPage) ? props.initialPage : lastPage);
lastPage = page.value;
watch(page, (id) => {
  lastPage = id;
});

const current = computed(() => PAGES.find((p) => p.id === page.value) || PAGES[0]);

// only the props a page declares, so nothing falls through as a DOM attribute
const pageProps = computed(() => {
  switch (current.value.id) {
    case "tablets":
    case "about":
      return { status: props.status };
    case "spotify":
      return { customValues: props.customValues };
    default:
      return {};
  }
});

const rootEl = ref(null);
const closeBtn = ref(null);
const scrollEl = ref(null);

function go(id) {
  if (page.value === id) return;
  page.value = id;
  if (scrollEl.value) scrollEl.value.scrollTop = 0;
}

function onKeydown(e) {
  if (e.key === "Escape") {
    emit("close");
    return;
  }
  // light focus loop: the overlay is opaque, so Tab must never wander
  // into the editor behind it
  if (e.key === "Tab" && rootEl.value && !rootEl.value.contains(document.activeElement)) {
    closeBtn.value?.focus();
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  closeBtn.value?.focus();
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div ref="rootEl" class="settings" role="dialog" aria-modal="true" :aria-label="t('Settings')">
    <header class="bar">
      <h1>{{ t("Settings") }}</h1>
      <button ref="closeBtn" class="close" :title="t('Close (Esc)')" :aria-label="t('Close (Esc)')" @click="emit('close')">
        <i class="fas fa-times" aria-hidden="true"></i>
      </button>
    </header>

    <div class="body">
      <nav class="nav" :aria-label="t('Settings sections')">
        <button
          v-for="p in corePages"
          :key="p.id"
          class="nav-item"
          :data-page="p.id"
          :aria-current="page === p.id ? 'page' : undefined"
          @click="go(p.id)"
        >
          <i :class="p.icon" aria-hidden="true"></i>
          <span>{{ p.label() }}</span>
          <span v-if="p.id === 'tablets' && !status.dbOk" class="nav-alert" :title="t('Server off')"></span>
        </button>

        <div class="nav-label">{{ t("Integrations") }}</div>
        <button
          v-for="p in integrationPages"
          :key="p.id"
          class="nav-item"
          :data-page="p.id"
          :aria-current="page === p.id ? 'page' : undefined"
          @click="go(p.id)"
        >
          <i :class="p.icon" aria-hidden="true"></i>
          <span>{{ p.label() }}</span>
        </button>

        <div class="nav-spacer" aria-hidden="true"></div>
        <button
          class="nav-item"
          :data-page="aboutPage.id"
          :aria-current="page === aboutPage.id ? 'page' : undefined"
          @click="go(aboutPage.id)"
        >
          <i :class="aboutPage.icon" aria-hidden="true"></i>
          <span>{{ aboutPage.label() }}</span>
        </button>
      </nav>

      <main ref="scrollEl" class="scroll">
        <component
          :is="current.component"
          :key="current.id"
          v-bind="pageProps"
        />
      </main>
    </div>
  </div>
</template>
