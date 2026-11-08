<script setup>
// Seasonal press effect: a small ring of particles bursts out of the
// pressed point and fades - snowflakes at Christmas, sparks at
// Halloween, eggs at Easter, hearts on Valentine's Day. Each burst lives
// ~700 ms and removes itself; a fixed cap keeps a frantic tapper from
// piling up nodes.
import { ref } from "vue";
import { prefersReducedMotion } from "../../season.js";

const props = defineProps({
  /// The effective season (season.js SEASONS).
  season: { type: String, default: "christmas" },
});

const MAX_BURSTS = 6;
const PARTICLES = 9;
const LIFETIME_MS = 720;

const LOOKS = {
  christmas: { shape: "flake", colors: ["#ffffff"] },
  halloween: { shape: "ember", colors: ["#ff9a3c", "#b57cff", "#ffd36e"] },
  easter: { shape: "egg", colors: ["#f7c6d9", "#fff3b0", "#cde8b5", "#c9d8f5", "#e2d1f7"] },
  valentines: { shape: "heart", colors: ["#f06292", "#ff8fb1", "#f8bbd0"] },
};

const bursts = ref([]);
let nextId = 1;

function particles(look) {
  return Array.from({ length: PARTICLES }, (_, i) => {
    const angle = (i / PARTICLES) * Math.PI * 2 + Math.random() * 0.5;
    const dist = 34 + Math.random() * 30;
    return {
      key: i,
      style: {
        "--dx": `${(Math.cos(angle) * dist).toFixed(1)}px`,
        "--dy": `${(Math.sin(angle) * dist).toFixed(1)}px`,
        "--s": (0.7 + Math.random() * 0.6).toFixed(2),
        "--c": look.colors[i % look.colors.length],
      },
    };
  });
}

/// Burst at viewport coordinates.
function burst(x, y) {
  if (prefersReducedMotion()) return;
  const look = LOOKS[props.season] || LOOKS.christmas;
  const id = nextId++;
  bursts.value = [
    ...bursts.value.slice(-(MAX_BURSTS - 1)),
    { id, x, y, shape: look.shape, parts: particles(look) },
  ];
  setTimeout(() => {
    bursts.value = bursts.value.filter((b) => b.id !== id);
  }, LIFETIME_MS);
}

defineExpose({ burst });
</script>

<template>
  <div class="season-burst" aria-hidden="true">
    <div
      v-for="b in bursts"
      :key="b.id"
      class="burst"
      :style="{ left: b.x + 'px', top: b.y + 'px' }"
    >
      <span v-for="p in b.parts" :key="p.key" class="bit" :class="b.shape" :style="p.style">{{
        b.shape === "flake" ? "❄" : ""
      }}</span>
    </div>
  </div>
</template>

<style scoped>
.season-burst {
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 40;
}
.burst {
  position: absolute;
  width: 0;
  height: 0;
}
.bit {
  position: absolute;
  animation: spark 720ms cubic-bezier(0.2, 0.7, 0.3, 1) forwards;
}
.flake {
  left: -6px;
  top: -8px;
  font-size: 13px;
  line-height: 1;
  color: #fff;
  text-shadow: 0 0 6px rgba(255, 255, 255, 0.7);
}
/* a glowing ember */
.ember {
  left: -3px;
  top: -3px;
  width: 6px;
  height: 6px;
  border-radius: 1px;
  background: var(--c);
  box-shadow: 0 0 8px var(--c);
}
.egg {
  left: -4px;
  top: -5px;
  width: 8px;
  height: 11px;
  border-radius: 50% 50% 50% 50% / 60% 60% 40% 40%;
  background: var(--c);
}
.heart {
  left: -6px;
  top: -5px;
  width: 12px;
  height: 11px;
  background: var(--c);
  -webkit-mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 22'%3E%3Cpath d='M12 21.5 10.3 20C4.2 14.5 0 10.8 0 6.3 0 2.7 2.8 0 6.4 0c2 0 4 1 5.6 2.5C13.6 1 15.6 0 17.6 0 21.2 0 24 2.7 24 6.3c0 4.5-4.2 8.2-10.3 13.7z'/%3E%3C/svg%3E") center / contain no-repeat;
  mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 22'%3E%3Cpath d='M12 21.5 10.3 20C4.2 14.5 0 10.8 0 6.3 0 2.7 2.8 0 6.4 0c2 0 4 1 5.6 2.5C13.6 1 15.6 0 17.6 0 21.2 0 24 2.7 24 6.3c0 4.5-4.2 8.2-10.3 13.7z'/%3E%3C/svg%3E") center / contain no-repeat;
}
@keyframes spark {
  from { transform: translate3d(0, 0, 0) scale(0.4); opacity: 1; }
  to { transform: translate3d(var(--dx), var(--dy), 0) scale(var(--s)) rotate(90deg); opacity: 0; }
}
</style>
