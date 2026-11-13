<script setup>
import { computed, onBeforeUnmount } from "vue";
import { api } from "../../api";
import TamagotchiSprite from "../TamagotchiSprite.vue";

// One desk pet tile (catalog type "pet", default 2x2). The pet is
// global: its compact snapshot arrives on the `pet` channel
// (APP_CUSTOM_VALUE here, ext.pet on v2) and the sprite renders straight
// from the pushed fields, so no surface reimplements the rules. Gestures
// in touch mode round-trip through the backend like the tool tiles
// (tap = feed, double-tap = play, long-press = clean, and on a dead pet
// long-press lays a new egg); the backend forwards them as the
// `pet-gesture` event and the webview applies them to the simulation.
const props = defineProps({
  tile: { type: Object, required: true },
  // last compact snapshot from the server; null = nothing pushed yet
  state: { type: Object, default: null },
  touch: { type: Boolean, default: false },
});
const emit = defineEmits(["tap"]);

// egg placeholder before the first push (fresh install, backend down)
const view = computed(() => {
  const s = props.state && typeof props.state === "object" ? props.state : null;
  return {
    species: s?.species || "blob",
    stage: s?.stage || "egg",
    mood: s?.mood || "neutral",
    activity: s?.activity || "idle",
    poos: Number(s?.poos) || 0,
  };
});

const dead = computed(() => view.value.stage === "dead");

const stats = computed(() => {
  const s = props.state || {};
  return [
    { key: "food", value: Number.isFinite(+s.food) ? Math.round(+s.food) : 100 },
    { key: "fun", value: Number.isFinite(+s.fun) ? Math.round(+s.fun) : 100 },
    { key: "rest", value: Number.isFinite(+s.rest) ? Math.round(+s.rest) : 100 },
    { key: "clean", value: Number.isFinite(+s.clean) ? Math.round(+s.clean) : 100 },
  ];
});

// ---- gestures (touch mode only) --------------------------------------------
// Same discriminator as ToolTile: a press held ~500 ms is the
// long-press, a second tap inside the double-tap window turns the first
// tap into a double-tap, everything else is a tap.

const LONG_PRESS_MS = 500;
const DOUBLE_TAP_MS = 300;
const TAP_SLOP_PX = 8;

let pressTimer = null;
let tapTimer = null;
let downAt = 0;
let downY = 0;
let consumed = false;

function clearPending() {
  if (pressTimer) clearTimeout(pressTimer);
  if (tapTimer) clearTimeout(tapTimer);
  pressTimer = null;
  tapTimer = null;
}
onBeforeUnmount(clearPending);

function runGesture(name) {
  api.execButtonGesture(props.tile.id, name).catch((e) =>
    console.warn("pet gesture failed", e),
  );
}

function onPointerDown(event) {
  if (!props.touch) return;
  consumed = false;
  downAt = performance.now();
  downY = event.clientY;
  pressTimer = setTimeout(() => {
    pressTimer = null;
    consumed = true;
    runGesture("long-press");
  }, LONG_PRESS_MS);
}

function onPointerUp(event) {
  if (!props.touch) return;
  if (pressTimer) {
    clearTimeout(pressTimer);
    pressTimer = null;
  }
  if (consumed || Math.abs(event.clientY - downY) > TAP_SLOP_PX) return;
  // wait out the double-tap window before acting on a lone tap
  if (tapTimer) {
    clearTimeout(tapTimer);
    tapTimer = null;
    consumed = true;
    runGesture("double-tap");
    return;
  }
  tapTimer = setTimeout(() => {
    tapTimer = null;
    emit("tap");
  }, DOUBLE_TAP_MS);
}
</script>

<template>
  <div
    class="pet-tile"
    :class="{ dead }"
    @pointerdown="onPointerDown"
    @pointerup="onPointerUp"
  >
    <div class="pet-screen" aria-hidden="true">
      <TamagotchiSprite v-bind="view" />
      <span v-if="dead" class="pet-rip">R.I.P.</span>
    </div>
    <div v-if="!dead" class="pet-bars">
      <span
        v-for="st in stats"
        :key="st.key"
        class="pet-bar"
        :title="st.key"
      >
        <i
          class="fill"
          :class="{ low: st.value < 20 }"
          :style="{ width: Math.max(0, Math.min(100, st.value)) + '%' }"
        ></i>
      </span>
    </div>
  </div>
</template>

<style scoped>
.pet-tile {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  gap: 4px;
  padding: 8% 10% 6%;
  overflow: hidden;
}
.pet-screen {
  position: relative;
  flex: 1;
  width: 78%;
  min-height: 0;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}
.pet-screen :deep(.tam-sprite) {
  width: 100%;
}
.pet-rip {
  position: absolute;
  top: 6%;
  left: 50%;
  transform: translateX(-50%);
  font-size: clamp(10px, 16cqw, 18px);
  letter-spacing: 0.12em;
  color: rgba(255, 255, 255, 0.75);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
}
.pet-bars {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 6%;
  width: 100%;
}
.pet-bar {
  height: 7%;
  min-height: 4px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.28);
  overflow: hidden;
}
.pet-bar .fill {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: #1abc9c;
}
.pet-bar .fill.low {
  background: #f1c40f;
}
</style>
