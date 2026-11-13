<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { t } from "../i18n";
import {
  MOOD_LABEL_KEYS,
  SPECIES_LABEL_KEYS,
  ageOf,
  blockedReason,
  pet,
  petDo,
  petEvents,
  phaseOf,
  spriteProps,
} from "../tamagotchi";
import TamagotchiSprite from "./TamagotchiSprite.vue";
import { Egg, Gamepad2, Hand, Moon, Sparkles, Sun, Utensils, X } from "lucide-vue-next";

// The desk pet panel: everything it shows comes from the tamagotchi store
// (pure rules in ../tamagotchi.js); this file only renders, forwards
// button presses and plays the short reaction animations.

const emit = defineEmits(["close"]);

// pet.value is replaced every tick, so every computed here is live and
// `lastTick` doubles as "now" without a second clock of our own.
const now = computed(() => pet.value.lastTick);
const phase = computed(() => phaseOf(pet.value, now.value));
const mood = computed(() => spriteProps(pet.value, now.value).mood);

const sprite = computed(() => {
  const p = spriteProps(pet.value, now.value);
  return { ...p, activity: activity.value || p.activity };
});

const caption = computed(() => {
  if (phase.value === "egg") return t("An egg. Give it a moment.");
  if (phase.value === "hatching") return t("Something is moving inside…");
  if (phase.value === "dead") return t("R.I.P.");
  return t(MOOD_LABEL_KEYS[mood.value]);
});

const stats = computed(() => [
  { key: "food", label: t("Food"), value: Math.round(pet.value.food) },
  { key: "fun", label: t("Fun"), value: Math.round(pet.value.fun) },
  { key: "rest", label: t("Rest"), value: Math.round(pet.value.rest) },
  { key: "clean", label: t("Cleanliness"), value: Math.round(pet.value.clean) },
]);

const REASONS = {
  full: () => t("Too full to eat"),
  hungry: () => t("Too hungry to play"),
  tired: () => t("Too tired to play"),
  asleep: () => t("It is asleep"),
  cooldown: () => t("It wants to be left alone for a bit"),
  clean: () => t("Already spotless"),
  egg: () => t("Still an egg"),
  dead: () => t("It is no more"),
};

const actions = computed(() => {
  const defs = [
    { name: "feed", label: t("Feed"), icon: Utensils },
    { name: "play", label: t("Play"), icon: Gamepad2 },
    { name: "pet", label: t("Pet"), icon: Hand },
    { name: "clean", label: t("Clean"), icon: Sparkles },
    pet.value.sleeping
      ? { name: "sleep", label: t("Wake up"), icon: Sun }
      : { name: "sleep", label: t("Sleep"), icon: Moon },
  ];
  return defs.map((a) => ({
    ...a,
    blocked: blockedReason(pet.value, a.name, now.value),
  }));
});

function reasonText(reason) {
  return REASONS[reason]?.() || "";
}

// ---- reactions ----------------------------------------------------------

const toast = ref("");
const activity = ref(null); // "eat" | "play" | "pet" for ~2 s after a press
let toastTimer = 0;
let activityTimer = 0;

function showToast(text) {
  if (!text) return;
  toast.value = text;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.value = ""), 2600);
}

function bumpActivity(kind) {
  activity.value = kind;
  clearTimeout(activityTimer);
  activityTimer = setTimeout(() => (activity.value = null), 1900);
}

const EVENT_TOASTS = {
  hatch: () => t("It hatched!"),
  grow: () => t("It grew up!"),
  poo: () => t("It pooped."),
  wake: () => t("It woke up."),
  die: () => t("It died…"),
  missedYou: () => t("It missed you."),
};

let seenEvents = petEvents.value.length;
watch(
  () => petEvents.value.length,
  (n) => {
    for (; seenEvents < n; seenEvents++) {
      showToast(EVENT_TOASTS[petEvents.value[seenEvents]]?.());
    }
  },
);

function act(name) {
  const r = petDo(name);
  if (!r.ok) {
    showToast(reasonText(r.reason));
    return;
  }
  if (name === "feed") bumpActivity("eat");
  else if (name === "play") bumpActivity("play");
  else if (name === "pet") bumpActivity("pet");
  else if (name === "sleep") showToast(pet.value.sleeping ? t("Zzz…") : t("It woke up."));
  else if (name === "newEgg") showToast(t("A new egg!"));
}

// ---- age ----------------------------------------------------------------

function partsOf(ms) {
  const m = Math.floor(ms / 60_000);
  const h = Math.floor(m / 60);
  const d = Math.floor(h / 24);
  const parts = [];
  if (d) {
    parts.push(t("{count} day | {count} days", { count: d }, d));
    parts.push(t("{count} hour | {count} hours", { count: h % 24 }, h % 24));
  } else if (h) {
    parts.push(t("{count} hour | {count} hours", { count: h }, h));
    parts.push(t("{count} minute | {count} minutes", { count: m % 60 }, m % 60));
  } else {
    parts.push(t("{count} minute | {count} minutes", { count: m }, m));
  }
  return parts.filter(Boolean).join(" ");
}

const ageText = computed(() => partsOf(ageOf(pet.value, now.value)));
const bestText = computed(() => (pet.value.bestAgeMs ? partsOf(pet.value.bestAgeMs) : ""));

// ---- shell: focus, keys, teardown ----------------------------------------

const rootEl = ref(null);
const closeBtn = ref(null);
let returnFocus = null;

function onKeydown(e) {
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopImmediatePropagation();
    emit("close");
    return;
  }
  // nothing behind the panel may react while it is up (Delete would edit
  // the board behind the veil)
  e.stopImmediatePropagation();
  if (e.key === "Tab") {
    const focusables = [...rootEl.value.querySelectorAll("button:not([disabled])")];
    if (!focusables.length) return;
    const idx = focusables.indexOf(document.activeElement);
    const next = e.shiftKey ? focusables[(idx - 1 + focusables.length) % focusables.length] : focusables[(idx + 1) % focusables.length];
    next.focus();
  }
}

onMounted(() => {
  returnFocus = document.activeElement;
  window.addEventListener("keydown", onKeydown, true);
  closeBtn.value?.focus();
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown, true);
  clearTimeout(toastTimer);
  clearTimeout(activityTimer);
  if (returnFocus?.isConnected) returnFocus.focus?.();
  returnFocus = null;
});
</script>

<template>
  <div class="veil" @mousedown.self="emit('close')">
    <div
      ref="rootEl"
      class="tam"
      role="dialog"
      aria-modal="true"
      aria-labelledby="tam-title"
    >
      <header class="tam-head">
        <h2 id="tam-title">{{ t("Desk pet") }}</h2>
        <button
          ref="closeBtn"
          class="tam-close"
          :title="t('Close (Esc)')"
          :aria-label="t('Close (Esc)')"
          @click="emit('close')"
        >
          <X :size="16" />
        </button>
      </header>

      <div class="tam-screen" :class="{ rip: phase === 'dead' }">
        <div class="tam-sprite-box" aria-hidden="true">
          <TamagotchiSprite v-bind="sprite" />
        </div>
        <Transition name="pop">
          <span v-if="toast" class="tam-toast" role="status">{{ toast }}</span>
        </Transition>
      </div>
      <p class="tam-caption" :class="{ dead: phase === 'dead' }">{{ caption }}</p>

      <template v-if="phase === 'baby' || phase === 'adult'">
        <div class="tam-stats">
          <div v-for="st in stats" :key="st.key" class="tam-stat">
            <span class="tam-stat-label">{{ st.label }}</span>
            <span class="tam-bar">
              <span class="fill" :class="{ low: st.value < 20 }" :style="{ width: st.value + '%' }"></span>
            </span>
            <span class="tam-stat-num tnum">{{ st.value }}</span>
          </div>
        </div>

        <div class="tam-actions">
          <button
            v-for="a in actions"
            :key="a.name"
            class="tam-act"
            :disabled="!!a.blocked"
            :title="a.blocked ? reasonText(a.blocked) : a.label"
            @click="act(a.name)"
          >
            <component :is="a.icon" :size="17" />
            <span>{{ a.label }}</span>
          </button>
        </div>
      </template>

      <div v-else-if="phase === 'dead'" class="tam-rip">
        <p>
          {{ t("It died of neglect after {age}. Hatch a new egg when you feel ready.", { age: ageText }) }}
        </p>
        <button class="tam-egg-btn" @click="act('newEgg')">
          <Egg :size="14" />
          {{ t("New egg") }}
        </button>
      </div>

      <footer class="tam-meta">
        <span>{{ t(SPECIES_LABEL_KEYS[pet.species]) }}</span>
        <span>{{ t("Age") }}: <b class="tnum">{{ ageText }}</b></span>
        <span>{{ t("Generation {n}", { n: pet.generations }) }}</span>
        <span v-if="bestText">{{ t("Best: {age}", { age: bestText }) }}</span>
        <span class="tam-cares tnum" :title="t('Care actions given')">{{ pet.cares }}×</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.veil {
  position: fixed;
  inset: 0;
  /* above editor modals, under the confirm popup and settings */
  z-index: 110;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(10, 14, 17, 0.55);
  backdrop-filter: blur(2px);
  animation: tam-fade 140ms ease-out;
}
@keyframes tam-fade {
  from { opacity: 0; }
  to { opacity: 1; }
}
.tam {
  width: min(370px, calc(100vw - 60px));
  background: #262e36;
  border: 1px solid rgba(26, 188, 156, 0.3);
  border-radius: 14px;
  padding: 18px 20px 16px;
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
  animation: tam-rise 200ms ease-out;
}
@keyframes tam-rise {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: none; }
}

.tam-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.tam-head h2 {
  font-size: 15px;
  font-weight: 600;
  color: var(--ink, #ecf0f1);
  margin: 0;
}
.tam-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 6px;
  color: var(--ink-2, #a3b0b8);
  background: transparent;
  transition: background 140ms ease-out, color 140ms ease-out;
}
.tam-close:hover { background: #313a44; color: var(--ink, #ecf0f1); }
.tam-close:focus-visible { outline: 2px solid var(--accent, #1abc9c); }

/* the pet's little screen: an LCD-ish well it lives in */
.tam-screen {
  position: relative;
  border-radius: 12px;
  background:
    radial-gradient(rgba(26, 188, 156, 0.07) 1px, transparent 1.6px) 0 0 / 14px 14px,
    #141a1f;
  border: 1px solid rgba(26, 188, 156, 0.16);
  padding: 18px 18px 10px;
  overflow: hidden;
}
.tam-screen.rip {
  border-color: rgba(231, 76, 60, 0.3);
  background:
    radial-gradient(rgba(231, 76, 60, 0.06) 1px, transparent 1.6px) 0 0 / 14px 14px,
    #161417;
}
/* not .tam-sprite: that class is the sprite root's own */
.tam-sprite-box {
  width: 172px;
  margin: 0 auto;
}
.tam-toast {
  position: absolute;
  top: 10px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 11.5px;
  color: var(--ink, #ecf0f1);
  background: rgba(20, 26, 31, 0.92);
  border: 1px solid rgba(26, 188, 156, 0.3);
  border-radius: 999px;
  padding: 4px 12px;
  white-space: nowrap;
  pointer-events: none;
}
.tam-toast.pop-enter-active,
.tam-toast.pop-leave-active { transition: opacity 160ms ease-out, transform 160ms ease-out; }
.tam-toast.pop-enter-from,
.tam-toast.pop-leave-to { opacity: 0; transform: translateX(-50%) translateY(-4px); }

.tam-caption {
  margin: 10px 2px 12px;
  font-size: 12.5px;
  color: var(--ink-2, #a3b0b8);
  text-align: center;
}
.tam-caption.dead { color: var(--danger-ink, #f19488); }

.tam-stats {
  display: grid;
  gap: 7px;
  margin-bottom: 14px;
}
.tam-stat {
  display: grid;
  grid-template-columns: 92px 1fr 26px;
  align-items: center;
  gap: 10px;
}
.tam-stat-label {
  font-size: 11.5px;
  color: var(--ink-2, #a3b0b8);
}
.tam-bar {
  height: 8px;
  border-radius: 999px;
  background: #1a2026;
  overflow: hidden;
}
.tam-bar .fill {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: var(--accent, #1abc9c);
  transition: width 400ms ease-out, background 300ms ease-out;
}
.tam-bar .fill.low { background: #f1c40f; }
.tam-stat-num {
  font-size: 11px;
  color: var(--ink-3, #8b97a1);
  text-align: right;
}

.tam-actions {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 8px;
}
.tam-act {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 5px;
  padding: 10px 4px 8px;
  border-radius: 9px;
  background: #313a44;
  color: var(--ink, #ecf0f1);
  font-size: 11px;
  transition: background 140ms ease-out, transform 100ms ease-out;
}
.tam-act:hover { background: #3a4753; }
.tam-act:active { transform: translateY(1px); }
.tam-act:focus-visible { outline: 2px solid var(--accent, #1abc9c); outline-offset: 2px; }
.tam-act:disabled { opacity: 0.42; cursor: default; }
.tam-act:disabled:hover { background: #313a44; }

.tam-rip {
  text-align: center;
  margin: 4px 0 12px;
}
.tam-rip p {
  margin: 0 0 12px;
  font-size: 12.5px;
  line-height: 1.55;
  color: var(--ink-2, #a3b0b8);
}
.tam-egg-btn {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12.5px;
  font-weight: 600;
  padding: 9px 18px;
  border-radius: 7px;
  background: var(--accent, #1abc9c);
  color: var(--accent-ink, #0b2a23);
  transition: background 140ms ease-out;
}
.tam-egg-btn:hover { background: var(--accent-2, #16a085); }
.tam-egg-btn:focus-visible { outline: 2px solid var(--accent, #1abc9c); outline-offset: 2px; }

.tam-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 14px;
  justify-content: space-between;
  margin-top: 14px;
  padding-top: 11px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 11px;
  color: var(--ink-3, #8b97a1);
}
.tam-meta b { color: var(--ink-2, #a3b0b8); font-weight: 600; }
</style>
