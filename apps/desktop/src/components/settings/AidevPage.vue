<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api } from "../../api";
import { t } from "../../i18n";

// The AI plan-limits tile: the aidev producer detects the rows on its
// own; this page picks which ones the tile shows. An empty stored
// selection means "everything detected" (new rows appear by themselves);
// saving writes [] while nothing is hidden. Every change saves at once
// and lands on the tile's next refresh, no restart.
const detected = ref([]); // [{id, label}]
const show = ref([]); // checked row ids
const summary = ref(true);
const rowStyle = ref("name"); // "name" | "logo"
const loaded = ref(false);
const busy = ref(false);
const error = ref("");
const note = ref("");

// provider slug -> group heading
const PROVIDERS = {
  glm: "GLM",
  codex: "Codex",
  claude: "Claude",
  openrouter: "OpenRouter",
  "anthropic-api": "Anthropic API",
  antigravity: "Antigravity",
};

const groups = computed(() => {
  const out = new Map();
  for (const row of detected.value) {
    const slug = String(row.id).split(":")[0];
    const name = PROVIDERS[slug] ?? (slug === "custom" ? t("Other") : slug);
    if (!out.has(name)) out.set(name, []);
    out.get(name).push(row);
  }
  return [...out.entries()].map(([name, rows]) => ({ name, rows }));
});

async function refresh() {
  try {
    const cfg = await api.aidevStatusConfig();
    detected.value = cfg?.detected || [];
    // an absent selection means everything detected is on
    const saved = new Set(cfg?.show || []);
    show.value = saved.size
      ? detected.value.filter((r) => saved.has(r.id)).map((r) => r.id)
      : detected.value.map((r) => r.id);
    summary.value = cfg?.summary !== false;
    rowStyle.value = cfg?.row_style === "logo" ? "logo" : "name";
    error.value = "";
  } catch (e) {
    error.value = e ? String(e) : t("Could not load the AI usage settings.");
  } finally {
    loaded.value = true;
  }
}

function flash(text) {
  note.value = text;
  setTimeout(() => {
    if (note.value === text) note.value = "";
  }, 3000);
}

async function save() {
  busy.value = true;
  error.value = "";
  try {
    const all = detected.value.map((r) => r.id);
    const everything = all.length > 0 && show.value.length === all.length;
    await api.setAidevStatusConfig(everything ? [] : show.value, summary.value, rowStyle.value);
    flash(t("Saved - the tile updates on its next refresh."));
  } catch (e) {
    error.value = e ? String(e) : t("Could not save the AI usage settings.");
  } finally {
    busy.value = false;
  }
}

function checked(id) {
  return show.value.includes(id);
}

function toggleRow(id) {
  const next = checked(id) ? show.value.filter((x) => x !== id) : [...show.value, id];
  if (!next.length) {
    flash(t("At least one row must stay selected."));
    return;
  }
  show.value = next;
  save();
}

function toggleSummary() {
  summary.value = !summary.value;
  save();
}

function setRowStyle(style) {
  if (rowStyle.value === style) return;
  rowStyle.value = style;
  save();
}

// detection fills up as the producer cycles: re-read while nothing is
// detected yet, so the list appears without leaving the page
let poll = null;
onMounted(async () => {
  await refresh();
  poll = setInterval(() => {
    if (!detected.value.length) refresh();
  }, 5000);
});
onUnmounted(() => {
  if (poll) clearInterval(poll);
});
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h2>{{ t("AI usage") }}</h2>
      <span v-if="detected.length" class="chip">
        {{ t("{shown} of {total} rows", { shown: show.length, total: detected.length }) }}
      </span>
    </header>
    <p class="lead">{{ t("What the AI plan-limits tile shows. Limits are detected automatically for GLM, Claude, Codex and OpenRouter.") }}</p>

    <section class="group">
      <h3 class="group-title">{{ t("Rows on the tile") }}</h3>
      <div class="card">
        <template v-if="groups.length">
          <div v-for="g in groups" :key="g.name" class="provider">
            <span class="provider-name">{{ g.name }}</span>
            <label v-for="row in g.rows" :key="row.id" class="check">
              <input type="checkbox" :checked="checked(row.id)" :disabled="busy" @change="toggleRow(row.id)" />
              {{ row.label }}
            </label>
          </div>
        </template>
        <p v-else class="empty">
          {{ loaded ? t("Nothing detected yet - detection takes a moment after Pulpit starts.") : t("Checking...") }}
        </p>
      </div>
    </section>

    <section class="group">
      <h3 class="group-title">{{ t("Look") }}</h3>
      <div class="card">
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Row label") }}</span>
            <span class="row-note">{{ t("Provider name or its logo.") }}</span>
          </div>
          <span class="seg" role="group" :aria-label="t('Row label')">
            <button class="seg-btn" :class="{ on: rowStyle === 'name' }" :aria-pressed="rowStyle === 'name'" :disabled="busy" @click="setRowStyle('name')">{{ t("Name") }}</button>
            <button class="seg-btn" :class="{ on: rowStyle === 'logo' }" :aria-pressed="rowStyle === 'logo'" :disabled="busy" @click="setRowStyle('logo')">{{ t("Logo") }}</button>
          </span>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-name">{{ t("Summary line") }}</span>
            <span class="row-note">{{ t("A line under the list naming the limit closest to running out.") }}</span>
          </div>
          <button
            class="switch"
            role="switch"
            :aria-checked="summary"
            :aria-label="t('Summary line')"
            :disabled="busy"
            @click="toggleSummary"
          ><span class="knob"></span></button>
        </div>
      </div>
      <p v-if="error" class="err">{{ error }}</p>
      <p v-else-if="note" class="note ok">{{ note }}</p>
    </section>
  </div>
</template>
