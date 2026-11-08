<script setup>
// Ambient particles for the seasonal themes: snow at Christmas, autumn
// leaves at Halloween, spring petals at Easter, hearts rising on
// Valentine's Day. Pure CSS: each particle is one span animating
// `transform` only (composited, no layout per frame), and a hidden
// window stops the animations with it. Positions come from a fixed seed
// so a re-render never reshuffles the sky.
import { computed } from "vue";

const props = defineProps({
  /// The effective season (season.js SEASONS).
  season: { type: String, default: "christmas" },
  /// Particle budget; the touch layer runs sparser than the editor.
  count: { type: Number, default: 36 },
});

// per season: particle shape, colours, size (px), fall time (s), spin
// (deg over one fall), share of the budget and direction
const LOOKS = {
  christmas: { shape: "snow", colors: ["#ffffff"], size: [2, 6], time: [9, 19], spin: 0, share: 1, rise: false },
  halloween: {
    shape: "leaf",
    colors: ["#f08a24", "#d9692a", "#a8552a", "#e3b04b"],
    size: [7, 12],
    time: [10, 18],
    spin: 540,
    share: 0.55,
    rise: false,
  },
  easter: {
    shape: "petal",
    colors: ["#f7c6d9", "#fff3b0", "#cde8b5", "#c9d8f5", "#e2d1f7"],
    size: [5, 9],
    time: [11, 19],
    spin: 360,
    share: 0.7,
    rise: false,
  },
  valentines: {
    shape: "heart",
    colors: ["#f06292", "#ec407a", "#f8bbd0", "#e57399"],
    size: [8, 13],
    time: [12, 20],
    spin: 0,
    share: 0.5,
    rise: true,
  },
};

const look = computed(() => LOOKS[props.season] || LOOKS.christmas);

// mulberry32: tiny deterministic PRNG
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const particles = computed(() => {
  const l = look.value;
  const r = rng(1224);
  const n = Math.max(1, Math.round(props.count * l.share));
  return Array.from({ length: n }, (_, i) => {
    const size = l.size[0] + r() * (l.size[1] - l.size[0]);
    const duration = l.time[0] + r() * (l.time[1] - l.time[0]);
    return {
      key: i,
      style: {
        left: `${(r() * 100).toFixed(2)}%`,
        width: `${size.toFixed(1)}px`,
        height: `${size.toFixed(1)}px`,
        opacity: (0.35 + r() * 0.5).toFixed(2),
        "--c": l.colors[Math.floor(r() * l.colors.length)],
        "--drift": `${Math.round((r() - 0.5) * 80)}px`,
        "--spin": `${Math.round((r() < 0.5 ? -1 : 1) * l.spin * (0.5 + r() * 0.5))}deg`,
        animationDuration: `${duration.toFixed(1)}s`,
        // negative delay: the sky is already full on the first frame
        animationDelay: `${(-r() * duration).toFixed(1)}s`,
      },
    };
  });
});
</script>

<template>
  <div class="season-ambient" aria-hidden="true">
    <span
      v-for="p in particles"
      :key="p.key"
      class="particle"
      :class="[look.shape, { rise: look.rise }]"
      :style="p.style"
    ></span>
  </div>
</template>

<style scoped>
.season-ambient {
  position: absolute;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
  container-type: size;
}
.particle {
  position: absolute;
  top: -14px;
  animation-name: fall;
  animation-timing-function: linear;
  animation-iteration-count: infinite;
  will-change: transform;
}
.particle.rise {
  top: auto;
  bottom: -16px;
  animation-name: rise;
}
.snow {
  border-radius: 50%;
  background: radial-gradient(circle, #fff 0%, rgba(255, 255, 255, 0.85) 45%, rgba(255, 255, 255, 0) 72%);
}
/* a leaf: two opposite rounded corners */
.leaf {
  background: var(--c);
  border-radius: 0 85% 0 85%;
}
/* a petal: a soft teardrop */
.petal {
  background: var(--c);
  border-radius: 60% 0 60% 60%;
}
.heart {
  background: var(--c);
  -webkit-mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 22'%3E%3Cpath d='M12 21.5 10.3 20C4.2 14.5 0 10.8 0 6.3 0 2.7 2.8 0 6.4 0c2 0 4 1 5.6 2.5C13.6 1 15.6 0 17.6 0 21.2 0 24 2.7 24 6.3c0 4.5-4.2 8.2-10.3 13.7z'/%3E%3C/svg%3E") center / contain no-repeat;
  mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 22'%3E%3Cpath d='M12 21.5 10.3 20C4.2 14.5 0 10.8 0 6.3 0 2.7 2.8 0 6.4 0c2 0 4 1 5.6 2.5C13.6 1 15.6 0 17.6 0 21.2 0 24 2.7 24 6.3c0 4.5-4.2 8.2-10.3 13.7z'/%3E%3C/svg%3E") center / contain no-repeat;
}
@keyframes fall {
  from { transform: translate3d(0, 0, 0) rotate(0deg); }
  50% { transform: translate3d(var(--drift), 52cqh, 0) rotate(calc(var(--spin) / 2)); }
  to { transform: translate3d(0, calc(100cqh + 28px), 0) rotate(var(--spin)); }
}
@keyframes rise {
  from { transform: translate3d(0, 0, 0); }
  50% { transform: translate3d(var(--drift), -52cqh, 0); }
  to { transform: translate3d(0, calc(-100cqh - 32px), 0); }
}
@media (prefers-reduced-motion: reduce) {
  .season-ambient { display: none; }
}
</style>
