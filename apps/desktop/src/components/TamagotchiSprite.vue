<script setup>
// Pulpit pet - pure presentation leaf. A hand-drawn inline-SVG creature with
// CSS-only animation. No state, no timers, no i18n, no assets: the panel that
// owns the game logic just passes the props below.
//
//   stage    egg | hatching | baby | adult | dead     (unknown values -> baby)
//   mood     happy | neutral | sad | hungry | sleepy | dirty | sick | dead
//            (face only; unknown values -> neutral)
//   activity idle | eat | play | pet | sleep
//   poos     0..3 piles on the ground (creature stages only)
//   species  blob | cat | frog | chick | axolotl      (unknown values -> blob)
//            shapes the body (ears / gills / wings / tail) and face details;
//            eggs are tinted with the species accent, the ghost's halo too.
import { computed } from "vue";

const props = defineProps({
  stage: { type: String, default: "baby" },
  mood: { type: String, default: "neutral" },
  activity: { type: String, default: "idle" },
  poos: { type: Number, default: 0 },
  species: { type: String, default: "blob" },
});

// Ground positions for the poop piles (viewBox x). Right side first, then
// left; the third pile hides slightly behind the first, so draw it first.
const POO_X = [176, 24, 189];
const POO_ORDER = [3, 2, 1];

const SPECIES = ["blob", "cat", "frog", "chick", "axolotl"];
const sp = computed(() => (SPECIES.includes(props.species) ? props.species : "blob"));

// speckle / speckle-dark / hatch-hole colors: the egg hints who is inside
const ACCENT = {
  blob: ["#1abc9c", "#16a085", "#12806b"],
  cat: ["#8fa6bd", "#6f89a3", "#52697f"],
  frog: ["#7cb342", "#5f9732", "#456f25"],
  chick: ["#f39c12", "#d68910", "#a8680a"],
  axolotl: ["#e06a92", "#c24f78", "#963a5a"],
};
const eggStyle = computed(() => {
  const a = ACCENT[sp.value];
  return { "--acc": a[0], "--acc2": a[1], "--acc3": a[2] };
});
const haloStyle = computed(() =>
  sp.value === "blob" ? {} : { "--tam-halo": ACCENT[sp.value][0] },
);

// Body silhouettes share one footprint (ground at y=0, body y -84..-4) so the
// face coordinates carry over; the frog is squatter, the axolotl flatter.
const BODY = {
  blob: "M-47,-40 C-47,-70 -27,-84 0,-84 C27,-84 47,-70 47,-40 C47,-14 40,-4 23,-4 L-23,-4 C-40,-4 -47,-14 -47,-40 Z",
  cat: "M-47,-40 C-47,-70 -27,-84 0,-84 C27,-84 47,-70 47,-40 C47,-14 40,-4 23,-4 L-23,-4 C-40,-4 -47,-14 -47,-40 Z",
  frog: "M-52,-30 C-52,-56 -34,-70 0,-70 C34,-70 52,-56 52,-30 C52,-12 44,-4 26,-4 L-26,-4 C-44,-4 -52,-12 -52,-30 Z",
  chick:
    "M-44,-42 C-44,-70 -26,-84 0,-84 C26,-84 44,-70 44,-42 C44,-16 31,-4 0,-4 C-31,-4 -44,-16 -44,-42 Z",
  axolotl:
    "M-48,-34 C-48,-60 -31,-74 0,-74 C31,-74 48,-60 48,-34 C48,-14 40,-4 23,-4 L-23,-4 C-40,-4 -48,-14 -48,-34 Z",
};
const BELLY = {
  blob: [0, -20, 27, 12.5],
  cat: [0, -20, 26, 12],
  frog: [0, -21, 34, 14],
  chick: [0, -22, 26, 14],
  axolotl: [0, -20, 30, 12.5],
};
const HIGHLIGHT = {
  blob: [-27, -68],
  cat: [-27, -68],
  frog: [-30, -56],
  chick: [-26, -68],
  axolotl: [-28, -60],
};
const SHADOW_RX = {
  blob: [50, 66],
  cat: [50, 66],
  frog: [58, 76],
  chick: [48, 63],
  axolotl: [54, 70],
};
const body = computed(() => BODY[sp.value]);
const belly = computed(() => BELLY[sp.value]);
const hi = computed(() => HIGHLIGHT[sp.value]);
const shadowRx = computed(() => SHADOW_RX[sp.value][props.stage === "adult" ? 1 : 0]);

// the frog's eyes ride up on the head bumps; cat/frog mouths sit a touch off
const eyesTf = computed(() => (sp.value === "frog" ? "translate(0 -22)" : undefined));
const mouthTf = computed(() =>
  sp.value === "cat" ? "translate(0 2)" : sp.value === "frog" ? "translate(0 -2)" : undefined,
);

// axolotl: three gill fronds per side (attach point at the head edge + angle)
const GILL_BASE = [
  { x: 37, y: -63, r: -56 },
  { x: 42, y: -52, r: -4 },
  { x: 41, y: -41, r: 42 },
];
const GILLS = GILL_BASE.flatMap((g, i) =>
  [-1, 1].map((s) => ({
    key: `${s}:${i}`,
    tf: `translate(${s * g.x} ${g.y}) scale(${s} 1) rotate(${g.r})`,
    d: `${(-0.5 * i - (s < 0 ? 0.8 : 0)).toFixed(2)}s`,
  })),
);
</script>

<template>
  <svg
    viewBox="0 0 200 200"
    class="tam-sprite"
    :data-species="sp"
    aria-hidden="true"
    focusable="false"
    xmlns="http://www.w3.org/2000/svg"
  >
    <!-- ============================ EGG ============================ -->
    <g
      v-if="stage === 'egg'"
      class="tam-egg"
      :style="eggStyle"
      transform="translate(100 172)"
    >
      <ellipse class="tam-shadow" cx="0" cy="2" rx="50" ry="7" />
      <g class="tam-enter">
        <g class="tam-egg-wobble">
          <path
            class="tam-egg-shade"
            d="M0,-150 C36,-150 60,-96 60,-58 C60,-22 34,-2 0,-2 C-34,-2 -60,-22 -60,-58 C-60,-96 -36,-150 0,-150 Z"
          />
          <path
            class="tam-egg-light"
            d="M0,-150 C36,-150 60,-96 60,-58 C60,-22 34,-2 0,-2 C-34,-2 -60,-22 -60,-58 C-60,-96 -36,-150 0,-150 Z"
            transform="translate(-4 -3) scale(.93 .96)"
          />
          <!-- speckles -->
          <g>
            <circle cx="-22" cy="-102" r="3.4" class="tam-speck-a" />
            <circle cx="16" cy="-122" r="2.6" class="tam-speck-b" />
            <circle cx="30" cy="-88" r="3.8" class="tam-speck-a" />
            <circle cx="-38" cy="-68" r="3" class="tam-speck-b" />
            <circle cx="4" cy="-78" r="2.2" class="tam-speck-a" />
            <circle cx="-12" cy="-46" r="3.6" class="tam-speck-a" />
            <circle cx="30" cy="-44" r="2.8" class="tam-speck-b" />
            <circle cx="-32" cy="-30" r="2.4" class="tam-speck-b" />
            <circle cx="12" cy="-20" r="2.2" class="tam-speck-a" />
            <circle cx="-4" cy="-132" r="1.8" class="tam-speck-b" />
          </g>
          <ellipse
            cx="-30"
            cy="-100"
            rx="9"
            ry="17"
            transform="rotate(24 -30 -100)"
            fill="#fff"
            opacity=".55"
          />
        </g>
      </g>
    </g>

    <!-- ========================= HATCHING ========================= -->
    <g
      v-else-if="stage === 'hatching'"
      class="tam-egg"
      :style="eggStyle"
      transform="translate(100 172)"
    >
      <ellipse class="tam-shadow" cx="0" cy="2" rx="50" ry="7" />
      <g class="tam-enter">
        <g class="tam-hatch-wobble">
          <path
            class="tam-egg-shade"
            d="M0,-150 C36,-150 60,-96 60,-58 C60,-22 34,-2 0,-2 C-34,-2 -60,-22 -60,-58 C-60,-96 -36,-150 0,-150 Z"
          />
          <path
            class="tam-egg-light"
            d="M0,-150 C36,-150 60,-96 60,-58 C60,-22 34,-2 0,-2 C-34,-2 -60,-22 -60,-58 C-60,-96 -36,-150 0,-150 Z"
            transform="translate(-4 -3) scale(.93 .96)"
          />
          <g>
            <circle cx="-22" cy="-110" r="3.2" class="tam-speck-a" />
            <circle cx="22" cy="-120" r="2.4" class="tam-speck-b" />
            <circle cx="-40" cy="-60" r="3" class="tam-speck-b" />
            <circle cx="-14" cy="-44" r="3.4" class="tam-speck-a" />
            <circle cx="32" cy="-46" r="2.8" class="tam-speck-b" />
            <circle cx="-30" cy="-28" r="2.4" class="tam-speck-b" />
            <circle cx="12" cy="-22" r="2.2" class="tam-speck-a" />
          </g>
          <ellipse
            cx="-32"
            cy="-112"
            rx="8"
            ry="14"
            transform="rotate(24 -32 -112)"
            fill="#fff"
            opacity=".5"
          />
          <!-- the hole the little one peeks through -->
          <path
            class="tam-hole"
            d="M-32,-84 L-23,-95 L-11,-83 L0,-97 L11,-83 L23,-95 L32,-84 L28,-72 Q0,-60 -28,-72 Z"
          />
          <g class="tam-peek">
            <circle cx="-10" cy="-77" r="4.2" fill="#fdfdf5" />
            <circle cx="10" cy="-77" r="4.2" fill="#fdfdf5" />
            <circle class="tam-ink" cx="-9.4" cy="-76.4" r="2.2" />
            <circle class="tam-ink" cx="9.4" cy="-76.4" r="2.2" />
          </g>
          <!-- cracks -->
          <g class="tam-crack">
            <path d="M-32,-84 L-40,-76 L-35,-68 L-44,-58" />
            <path d="M32,-84 L41,-78 L36,-70" />
            <path d="M0,-97 L5,-108 L-2,-116 L4,-127" />
            <path d="M-23,-95 L-27,-104" />
            <path d="M-18,-40 L-11,-49 L-16,-56" />
          </g>
        </g>
      </g>
    </g>

    <!-- ======================= DEAD (GHOST) ======================= -->
    <g
      v-else-if="stage === 'dead'"
      class="tam-ghost"
      :style="haloStyle"
      transform="translate(100 172)"
    >
      <ellipse class="tam-shadow tam-shadow-ghost" cx="0" cy="2" rx="40" ry="6" />
      <g class="tam-enter">
        <g transform="scale(1.3)">
          <g class="tam-ghost-actor">
            <!-- halo -->
            <ellipse class="tam-halo" cx="0" cy="-96" rx="16" ry="4.6" />
            <g class="tam-ghost-body">
              <ellipse
                class="tam-ghost-shade"
                cx="-46"
                cy="-40"
                rx="5.5"
                ry="8.5"
                transform="rotate(-32 -46 -40)"
              />
              <ellipse
                class="tam-ghost-shade"
                cx="46"
                cy="-40"
                rx="5.5"
                ry="8.5"
                transform="rotate(32 46 -40)"
              />
              <path
                class="tam-ghost-shade"
                d="M-47,-40 C-47,-70 -27,-84 0,-84 C27,-84 47,-70 47,-40 L47,-10 Q31.3,4 15.7,-10 Q0,4 -15.7,-10 Q-31.3,4 -47,-10 Z"
              />
              <path
                class="tam-ghost-light"
                d="M-47,-40 C-47,-70 -27,-84 0,-84 C27,-84 47,-70 47,-40 L47,-10 Q31.3,4 15.7,-10 Q0,4 -15.7,-10 Q-31.3,4 -47,-10 Z"
                transform="translate(-3 -2) scale(.94 .95)"
              />
              <ellipse
                cx="-27"
                cy="-68"
                rx="9"
                ry="4.6"
                transform="rotate(-38 -27 -68)"
                fill="#fff"
                opacity=".3"
              />
            </g>
            <!-- face: X eyes + small "oh" mouth -->
            <g class="tam-line tam-ghost-ink">
              <path d="M-22,-59 L-12,-49 M-12,-59 L-22,-49" />
              <path d="M12,-59 L22,-49 M22,-59 L12,-49" />
            </g>
            <ellipse class="tam-ghost-mouth" cx="0" cy="-41" rx="3.2" ry="3.9" />
          </g>
        </g>
      </g>
    </g>

    <!-- ===================== CREATURE (baby / adult) ===================== -->
    <g v-else>
      <!-- poop piles on the ground, beside the creature -->
      <template v-for="n in POO_ORDER" :key="n">
        <g
          v-if="poos >= n"
          :transform="`translate(${POO_X[n - 1]} 173) scale(${n === 3 ? 0.82 : 1})`"
        >
          <g class="tam-poo" :style="{ '--d': n * -0.9 + 's' }">
            <ellipse cx="0" cy="-4.5" rx="11.5" ry="5.2" fill="#6e5636" />
            <ellipse cx="0" cy="-5.6" rx="10.3" ry="4.4" fill="#8b6f47" />
            <ellipse cx="0" cy="-11" rx="8.4" ry="4.4" fill="#6e5636" />
            <ellipse cx="-0.3" cy="-12" rx="7.4" ry="3.7" fill="#8b6f47" />
            <ellipse cx="0" cy="-16.5" rx="5.6" ry="3.4" fill="#6e5636" />
            <ellipse cx="-0.2" cy="-17.4" rx="4.7" ry="2.8" fill="#8b6f47" />
            <path
              class="tam-poo-tip"
              d="M-2.4,-19 Q-1.5,-26 3.2,-24.5 Q0.4,-22.6 1.8,-19.4 Z"
            />
            <path class="tam-poo-shine" d="M-8,-7.5 Q-6.4,-10 -3.8,-10.2" />
            <circle class="tam-ink" cx="-3.8" cy="-4.6" r="1.2" />
            <circle class="tam-ink" cx="3.8" cy="-4.6" r="1.2" />
            <path class="tam-poo-smile" d="M-1.8,-2.4 Q0,-1 1.8,-2.4" />
          </g>
        </g>
      </template>

      <g
        :class="[stage === 'adult' ? 'tam-adult' : 'tam-baby', 'tam-sp-' + sp]"
        transform="translate(100 172)"
      >
        <ellipse
          class="tam-shadow"
          :class="{ 'tam-shadow-play': activity === 'play' }"
          cx="0"
          cy="2"
          :rx="shadowRx"
          ry="7"
        />
        <g class="tam-enter">
          <g :transform="stage === 'adult' ? 'scale(1.5)' : 'scale(1.1)'">
            <g class="tam-actor" :class="'tam-act-' + activity">
              <!-- ===== behind the body: per-species silhouette parts ===== -->

              <!-- blob (adult): tiny leaf sprout -->
              <g v-if="sp === 'blob' && stage === 'adult'" class="tam-sprout">
                <path class="tam-sprout-stem" d="M0,-82 C0,-88 1,-92 3,-95" />
                <path
                  class="tam-sprout-leaf"
                  d="M3,-95 C0,-103 8,-108 14,-105 C14,-98 9,-93 3,-95 Z"
                />
                <path class="tam-sprout-vein" d="M5,-97 Q8.5,-100 11.5,-102" />
              </g>

              <!-- cat: swaying tail + pointed ears with inner-ear detail -->
              <template v-if="sp === 'cat'">
                <g class="tam-tail tam-tail-cat">
                  <path class="tam-tail-out" d="M36,-12 C58,-10 64,-30 57,-50" />
                  <path class="tam-tail-in" d="M36,-12 C58,-10 64,-30 57,-50" />
                  <path class="tam-tail-tip" d="M60,-43 Q58.6,-46.6 57,-50" />
                </g>
                <g class="tam-ear tam-ear-l">
                  <path
                    class="tam-limb"
                    d="M-45,-56 L-39,-103 Q-38,-105.5 -35.5,-104 L-9,-79 Z"
                  />
                  <path class="tam-ear-in" d="M-38,-71 L-36,-93 L-21,-79 Z" />
                </g>
                <g class="tam-ear tam-ear-r">
                  <path
                    class="tam-limb"
                    d="M45,-56 L39,-103 Q38,-105.5 35.5,-104 L9,-79 Z"
                  />
                  <path class="tam-ear-in" d="M38,-71 L36,-93 L21,-79 Z" />
                </g>
              </template>

              <!-- frog: fat hind legs + big webbed feet -->
              <template v-else-if="sp === 'frog'">
                <ellipse class="tam-limb" cx="-40" cy="-16" rx="15" ry="12" />
                <ellipse class="tam-limb" cx="40" cy="-16" rx="15" ry="12" />
                <ellipse class="tam-limb" cx="-32" cy="-3" rx="15" ry="4.4" />
                <ellipse class="tam-limb" cx="32" cy="-3" rx="15" ry="4.4" />
              </template>

              <!-- chick: orange feet; adult has a single hair curl -->
              <template v-else-if="sp === 'chick'">
                <g v-if="stage === 'adult'" class="tam-sprout">
                  <path
                    class="tam-curl"
                    d="M0,-83 C-2,-93 9,-97 10,-90 C11,-85 5,-85 5,-89"
                  />
                </g>
                <path
                  v-for="s in [-1, 1]"
                  :key="'ft' + s"
                  class="tam-foot"
                  d="M-10,0.5 Q-10,-6 -5,-6.5 Q0,-7.5 5,-6.5 Q10,-6 10,0.5 Q7,2.5 3.5,0.5 Q0,3 -3.5,0.5 Q-7,2.5 -10,0.5 Z"
                  :transform="`translate(${s * 17} -1)`"
                />
              </template>

              <!-- axolotl: three gill fronds each side (+ fin tail on adults) -->
              <template v-else-if="sp === 'axolotl'">
                <g v-if="stage === 'adult'" class="tam-tail tam-tail-axo">
                  <path
                    class="tam-gill-shape"
                    d="M32,-8 C48,-4 60,-12 59,-26 C58,-33 50,-26 32,-22 Z"
                  />
                  <path class="tam-fin-rib" d="M38,-14 C46,-12 53,-16 55,-23" />
                </g>
                <g v-for="g in GILLS" :key="g.key" :transform="g.tf">
                  <g class="tam-gill" :style="{ '--d': g.d }">
                    <ellipse class="tam-gill-shape" cx="11" cy="0" rx="11.5" ry="6" />
                    <ellipse class="tam-gill-hi" cx="11.5" cy="-0.6" rx="6.5" ry="2" />
                  </g>
                </g>
              </template>

              <!-- arms + feet sit behind the body (blob, cat, axolotl) -->
              <template v-if="sp === 'blob' || sp === 'cat' || sp === 'axolotl'">
                <ellipse
                  class="tam-limb"
                  cx="-46"
                  cy="-34"
                  rx="6"
                  ry="9.5"
                  transform="rotate(18 -46 -34)"
                />
                <ellipse
                  class="tam-limb"
                  cx="46"
                  cy="-34"
                  rx="6"
                  ry="9.5"
                  transform="rotate(-18 46 -34)"
                />
                <ellipse class="tam-limb" cx="-21" cy="-3" rx="11" ry="4.2" />
                <ellipse class="tam-limb" cx="21" cy="-3" rx="11" ry="4.2" />
              </template>

              <!-- body: dark base + lighter, shifted copy = 2-tone shading -->
              <path class="tam-body-shade" :d="body" />
              <path
                class="tam-body-light"
                :d="body"
                transform="translate(-3 -2) scale(.94 .95)"
              />
              <ellipse
                class="tam-belly"
                :cx="belly[0]"
                :cy="belly[1]"
                :rx="belly[2]"
                :ry="belly[3]"
              />
              <ellipse
                :cx="hi[0]"
                :cy="hi[1]"
                rx="9"
                ry="4.6"
                :transform="`rotate(-38 ${hi[0]} ${hi[1]})`"
                fill="#fff"
                opacity=".28"
              />

              <!-- ===== on the body: per-species markings (static) ===== -->

              <!-- cat: chest ruff (adult), muzzle pads, nose -->
              <template v-if="sp === 'cat'">
                <path
                  v-if="stage === 'adult'"
                  class="tam-ruff"
                  d="M-26,-38 C-31,-28 -23,-17 -15,-23 C-13,-14 -3,-12 0,-21 C3,-12 13,-14 15,-23 C23,-17 31,-28 26,-38 Q0,-30 -26,-38 Z"
                />
                <ellipse class="tam-muzzle" cx="-6.5" cy="-41" rx="9" ry="7.5" />
                <ellipse class="tam-muzzle" cx="6.5" cy="-41" rx="9" ry="7.5" />
                <path class="tam-nose" d="M-3.8,-48.6 H3.8 L0,-44.6 Z" />
              </template>

              <!-- frog: back spots (adult), front feet -->
              <template v-else-if="sp === 'frog'">
                <g v-if="stage === 'adult'" class="tam-spots">
                  <circle cx="-38" cy="-44" r="5.5" />
                  <circle cx="-27" cy="-55" r="4" />
                  <circle cx="38" cy="-50" r="6" />
                  <circle cx="31" cy="-37" r="3.6" />
                </g>
                <ellipse class="tam-limb" cx="-15" cy="-6" rx="9" ry="5" />
                <ellipse class="tam-limb" cx="15" cy="-6" rx="9" ry="5" />
              </template>

              <!-- chick: stubby wings with white patches (they flap) -->
              <template v-else-if="sp === 'chick'">
                <g
                  v-for="s in [-1, 1]"
                  :key="'wg' + s"
                  :class="s < 0 ? 'tam-wing-l' : 'tam-wing-r'"
                >
                  <g :transform="s > 0 ? 'scale(-1 1)' : null">
                    <path
                      class="tam-wing"
                      d="M-40,-56 C-52,-54 -58,-38 -52,-26 C-48,-20 -40,-24 -37,-30 C-33,-40 -34,-52 -40,-56 Z"
                    />
                    <path
                      class="tam-wing-patch"
                      d="M-45,-47 C-50,-42 -51,-34 -47,-30 C-43,-33 -42,-42 -45,-47 Z"
                    />
                  </g>
                </g>
              </template>

              <!-- frog: bump eyes ON TOP of the head -->
              <template v-if="sp === 'frog'">
                <template v-for="s in [-1, 1]" :key="'bp' + s">
                  <circle class="tam-bump" :cx="s * 18" cy="-76" r="13.5" />
                  <circle class="tam-sclera" :cx="s * 18" cy="-76" r="9.6" />
                </template>
              </template>

              <!-- chick / axolotl: cheeks are always on -->
              <g
                v-if="(sp === 'chick' || sp === 'axolotl') && mood !== 'sick' && mood !== 'dead'"
                class="tam-blush"
                :class="{ 'tam-blush-soft': sp === 'axolotl' }"
              >
                <ellipse
                  v-for="s in [-1, 1]"
                  :key="'ck' + s"
                  :cx="s * (sp === 'chick' ? 30 : 34)"
                  cy="-45"
                  rx="6.5"
                  ry="3.8"
                />
              </g>

              <!-- dirty: grime smudges on the body -->
              <g v-if="mood === 'dirty'" class="tam-grime">
                <ellipse cx="-30" cy="-24" rx="5" ry="3.2" />
                <ellipse cx="31" cy="-60" rx="4" ry="2.6" />
                <ellipse cx="22" cy="-18" rx="3" ry="2" />
              </g>

              <!-- ---------------- face (fades between moods) ---------------- -->
              <Transition name="tam-fade" mode="out-in">
                <g :key="mood + ':' + (activity === 'sleep')" class="tam-face">
                  <!-- eyes (frog: lifted onto the head bumps, with eyelids) -->
                  <g :transform="eyesTf">
                    <g
                      v-if="sp === 'frog' && activity === 'sleep' && mood !== 'dead'"
                    >
                      <circle class="tam-lid tam-lid-l tam-lid-shut" cx="-18" cy="-54" r="12.6" />
                      <circle class="tam-lid tam-lid-r tam-lid-shut" cx="18" cy="-54" r="12.6" />
                    </g>

                    <g v-if="mood === 'dead'" class="tam-line">
                      <path d="M-22,-59 L-12,-49 M-12,-59 L-22,-49" />
                      <path d="M12,-59 L22,-49 M22,-59 L12,-49" />
                    </g>
                    <g v-else-if="activity === 'sleep'" class="tam-line">
                      <path d="M-23,-54 Q-17,-48.5 -11,-54" />
                      <path d="M11,-54 Q17,-48.5 23,-54" />
                    </g>
                    <g v-else-if="mood === 'happy'" class="tam-line">
                      <path d="M-23,-52 Q-17,-61 -11,-52" />
                      <path d="M11,-52 Q17,-61 23,-52" />
                    </g>
                    <g v-else-if="mood === 'sad'">
                      <g class="tam-blink">
                        <circle class="tam-ink" cx="-17" cy="-53" r="4.8" />
                        <circle class="tam-ink" cx="17" cy="-53" r="4.8" />
                        <circle class="tam-glint" cx="-15.4" cy="-54.6" r="1.5" />
                        <circle class="tam-glint" cx="18.6" cy="-54.6" r="1.5" />
                      </g>
                      <g class="tam-line tam-line-thin">
                        <path d="M-25,-61 L-12,-65" />
                        <path d="M12,-65 L25,-61" />
                      </g>
                    </g>
                    <g v-else-if="mood === 'hungry'">
                      <g class="tam-blink">
                        <!-- big pleading eyes with a squinted (flat) top -->
                        <path
                          class="tam-ink"
                          d="M-22.2,-57 A6,6 0 1 0 -11.8,-57 Z"
                        />
                        <path
                          class="tam-ink"
                          d="M11.8,-57 A6,6 0 1 0 22.2,-57 Z"
                        />
                        <circle class="tam-glint" cx="-14.8" cy="-55.2" r="2.1" />
                        <circle class="tam-glint" cx="19.2" cy="-55.2" r="2.1" />
                        <circle class="tam-glint" cx="-19.6" cy="-51.6" r="1" />
                        <circle class="tam-glint" cx="14.4" cy="-51.6" r="1" />
                      </g>
                      <g class="tam-line tam-line-thin">
                        <path d="M-24,-62 L-12,-65.5" />
                        <path d="M12,-65.5 L24,-62" />
                      </g>
                    </g>
                    <g v-else-if="mood === 'sleepy'">
                      <path class="tam-ink" d="M-22,-54 A5,5 0 0 0 -12,-54 Z" />
                      <path class="tam-ink" d="M12,-54 A5,5 0 0 0 22,-54 Z" />
                      <g class="tam-line tam-line-thin">
                        <path d="M-23.5,-54 H-10.5" />
                        <path d="M10.5,-54 H23.5" />
                      </g>
                    </g>
                    <g v-else-if="mood === 'dirty'" class="tam-line">
                      <path d="M-23,-54 H-11" />
                      <path d="M11,-54 H23" />
                    </g>
                    <g v-else-if="mood === 'sick'">
                      <g transform="translate(-17 -54)">
                        <path
                          class="tam-line tam-line-thin tam-spin"
                          d="M0,0 A1.5,1.5 0 0 1 3,0 A3,3 0 0 1 -3,0 A4.5,4.5 0 0 1 6,0 A6,6 0 0 1 -6,0"
                        />
                      </g>
                      <g transform="translate(17 -54)">
                        <path
                          class="tam-line tam-line-thin tam-spin tam-spin-rev"
                          d="M0,0 A1.5,1.5 0 0 1 3,0 A3,3 0 0 1 -3,0 A4.5,4.5 0 0 1 6,0 A6,6 0 0 1 -6,0"
                        />
                      </g>
                    </g>
                    <g v-else class="tam-blink">
                      <circle class="tam-ink" cx="-17" cy="-54" r="5.2" />
                      <circle class="tam-ink" cx="17" cy="-54" r="5.2" />
                      <circle class="tam-glint" cx="-15.2" cy="-55.8" r="1.6" />
                      <circle class="tam-glint" cx="18.8" cy="-55.8" r="1.6" />
                    </g>

                    <!-- frog eyelids: blink by default, half shut when sleepy -->
                    <g
                      v-if="sp === 'frog' && activity !== 'sleep' && mood !== 'dead'"
                    >
                      <circle
                        class="tam-lid tam-lid-l"
                        :class="mood === 'sleepy' ? 'tam-lid-half' : 'tam-lid-blink'"
                        cx="-18"
                        cy="-54"
                        r="12.6"
                      />
                      <circle
                        class="tam-lid tam-lid-r"
                        :class="mood === 'sleepy' ? 'tam-lid-half' : 'tam-lid-blink'"
                        cx="18"
                        cy="-54"
                        r="12.6"
                      />
                    </g>
                  </g>

                  <!-- mouth (scales open/shut while eating) -->
                  <g :transform="mouthTf">
                    <g :class="{ 'tam-chew': activity === 'eat' }">
                      <!-- chick: the beak IS the mouth (opens when happy / hungry) -->
                      <g v-if="sp === 'chick'">
                        <template v-if="mood === 'happy' || mood === 'hungry'">
                          <ellipse class="tam-ink" cx="0" cy="-43.4" rx="3.8" ry="2.6" />
                          <path
                            class="tam-beak"
                            d="M-6.5,-47 Q0,-52.5 6.5,-47 Q3,-44 0,-44 Q-3,-44 -6.5,-47 Z"
                          />
                          <path
                            class="tam-beak"
                            d="M-4.6,-42.6 Q0,-41.6 4.6,-42.6 Q3.4,-37.4 0,-37.4 Q-3.4,-37.4 -4.6,-42.6 Z"
                          />
                        </template>
                        <path
                          v-else
                          class="tam-beak"
                          d="M-6.5,-47 Q0,-52.5 6.5,-47 Q4.5,-41.5 0,-40.5 Q-4.5,-41.5 -6.5,-47 Z"
                        />
                      </g>

                      <!-- frog: wide mouth line across the whole head -->
                      <g v-else-if="sp === 'frog'">
                        <g v-if="mood === 'dead'" class="tam-line">
                          <path d="M-14,-41 H14" />
                        </g>
                        <g v-else-if="mood === 'happy'">
                          <path class="tam-ink" d="M-19,-46 Q0,-26 19,-46 Z" />
                          <ellipse class="tam-tongue" cx="0" cy="-39.4" rx="5.5" ry="2.4" />
                        </g>
                        <g v-else-if="mood === 'sad'" class="tam-line">
                          <path d="M-22,-39 Q0,-48 22,-39" />
                        </g>
                        <ellipse
                          v-else-if="mood === 'hungry'"
                          class="tam-ink"
                          cx="0"
                          cy="-41"
                          rx="9"
                          ry="5"
                        />
                        <g v-else-if="mood === 'sleepy'" class="tam-line">
                          <path d="M-20,-43 Q0,-39.5 20,-43" />
                        </g>
                        <g v-else-if="mood === 'dirty'" class="tam-line">
                          <path d="M-20,-42 q5,-4 10,0 t10,0 t10,0 t10,0" />
                        </g>
                        <g v-else-if="mood === 'sick'" class="tam-line">
                          <path d="M-17,-42 q4.25,-3.5 8.5,0 t8.5,0 t8.5,0 t8.5,0" />
                        </g>
                        <g v-else class="tam-line">
                          <path d="M-27,-45 Q0,-36 27,-45" />
                        </g>
                      </g>

                      <!-- blob, cat, axolotl -->
                      <template v-else>
                        <g v-if="mood === 'dead'" class="tam-line">
                          <path d="M-6,-41 H6" />
                        </g>
                        <g v-else-if="mood === 'happy' && sp === 'axolotl'">
                          <path class="tam-ink" d="M-14,-47 Q0,-28 14,-47 Z" />
                          <ellipse class="tam-tongue" cx="0" cy="-40" rx="5" ry="2.3" />
                        </g>
                        <g v-else-if="mood === 'happy'">
                          <path class="tam-ink" d="M-9,-46 Q0,-32 9,-46 Z" />
                          <ellipse class="tam-tongue" cx="0" cy="-41.2" rx="3.6" ry="1.8" />
                        </g>
                        <g v-else-if="mood === 'sad'" class="tam-line">
                          <path d="M-7,-39 Q0,-46 7,-39" />
                        </g>
                        <ellipse
                          v-else-if="mood === 'hungry'"
                          class="tam-ink"
                          cx="0"
                          cy="-42"
                          rx="3.4"
                          ry="3.2"
                        />
                        <g
                          v-else-if="mood === 'sleepy' && sp === 'axolotl'"
                          class="tam-line"
                        >
                          <path d="M-9,-44 Q0,-39.5 9,-44" />
                        </g>
                        <ellipse
                          v-else-if="mood === 'sleepy'"
                          class="tam-ink"
                          cx="0"
                          cy="-42"
                          rx="3"
                          ry="2.6"
                        />
                        <g v-else-if="mood === 'dirty'" class="tam-line">
                          <path d="M-9,-42 q3,-3 6,0 t6,0 t6,0" />
                        </g>
                        <g v-else-if="mood === 'sick'" class="tam-line">
                          <path d="M-7,-42 q2.3,-3 4.6,0 t4.6,0 t4.6,0" />
                        </g>
                        <!-- neutral: axolotls always look pleased -->
                        <g v-else-if="sp === 'axolotl'" class="tam-line">
                          <path d="M-15,-46 Q0,-33 15,-46" />
                        </g>
                        <g v-else-if="sp === 'cat'" class="tam-line">
                          <path d="M-8.5,-42 Q-4.2,-37.5 0,-42.5 Q4.2,-37.5 8.5,-42" />
                        </g>
                        <g v-else class="tam-line">
                          <path d="M-6,-43 Q0,-38 6,-43" />
                        </g>
                      </template>
                    </g>
                  </g>

                  <!-- mood extras -->
                  <g
                    v-if="mood === 'happy' && sp !== 'chick' && sp !== 'axolotl'"
                    class="tam-blush"
                  >
                    <ellipse cx="-31" cy="-45" rx="6" ry="3.5" />
                    <ellipse cx="31" cy="-45" rx="6" ry="3.5" />
                  </g>
                  <g v-else-if="mood === 'sad'" :transform="eyesTf">
                    <g transform="translate(-24 -47)">
                      <g class="tam-drop-fall">
                        <path
                          class="tam-drop"
                          d="M0,0 C0,0 -3.6,5 -3.6,7.6 A3.6,3.6 0 0 0 3.6,7.6 C3.6,5 0,0 0,0 Z"
                        />
                      </g>
                    </g>
                  </g>
                  <g v-else-if="mood === 'hungry'">
                    <g transform="translate(5.2 -39.5) scale(.55)">
                      <g class="tam-drool">
                        <path
                          class="tam-drop"
                          d="M0,0 C0,0 -3.6,5 -3.6,7.6 A3.6,3.6 0 0 0 3.6,7.6 C3.6,5 0,0 0,0 Z"
                        />
                      </g>
                    </g>
                    <path
                      class="tam-rumble"
                      d="M-10,-20 q2.5,-3.5 5,0 t5,0 t5,0 t5,0"
                    />
                  </g>
                  <g v-else-if="mood === 'sick'">
                    <ellipse class="tam-patch" cx="29" cy="-46" rx="11" ry="8" />
                    <ellipse class="tam-patch" cx="-30" cy="-44" rx="6.5" ry="4.5" />
                    <g transform="translate(33 -70) scale(.6)">
                      <g class="tam-drop-fall tam-sweat">
                        <path
                          class="tam-drop"
                          d="M0,0 C0,0 -3.6,5 -3.6,7.6 A3.6,3.6 0 0 0 3.6,7.6 C3.6,5 0,0 0,0 Z"
                        />
                      </g>
                    </g>
                  </g>
                </g>
              </Transition>

              <!-- cat: chunky whiskers, 2 per side, poking past the silhouette -->
              <g v-if="sp === 'cat'" class="tam-whiskers">
                <path d="M-23,-44 L-53,-48" />
                <path d="M-23,-38 L-52,-34" />
                <path d="M23,-44 L53,-48" />
                <path d="M23,-38 L52,-34" />
              </g>

              <!-- dirty: stink lines rising -->
              <g v-if="mood === 'dirty'">
                <g transform="translate(-30 -80)">
                  <g class="tam-stink" style="--d: 0s">
                    <path class="tam-stink-line" d="M0,0 q-4,-4 0,-8 t0,-8" />
                  </g>
                </g>
                <g transform="translate(-16 -86)">
                  <g class="tam-stink" style="--d: 0.9s">
                    <path class="tam-stink-line" d="M0,0 q-4,-4 0,-8 t0,-8" />
                  </g>
                </g>
                <g transform="translate(30 -80)">
                  <g class="tam-stink" style="--d: 1.7s">
                    <path class="tam-stink-line" d="M0,0 q-4,-4 0,-8 t0,-8" />
                  </g>
                </g>
              </g>

              <!-- eat: crumbs pop near the mouth and fall -->
              <g v-if="activity === 'eat'">
                <g transform="translate(-9 -41)">
                  <circle class="tam-crumb" r="3" style="--dx: -7px; --d: 0s" />
                </g>
                <g transform="translate(10 -42)">
                  <circle
                    class="tam-crumb tam-crumb-dark"
                    r="2.6"
                    style="--dx: 8px; --d: 0.4s"
                  />
                </g>
                <g transform="translate(1 -37)">
                  <circle class="tam-crumb" r="2.2" style="--dx: 2px; --d: 0.8s" />
                </g>
              </g>

              <!-- pet: two hearts rise -->
              <g v-if="activity === 'pet'">
                <g transform="translate(-38 -78)">
                  <g class="tam-heart" style="--dx: -4px; --d: 0s">
                    <path
                      transform="scale(.85)"
                      d="M0,5 C-10,-1 -8,-9 -4,-9 C-2,-9 0,-7 0,-6 C0,-7 2,-9 4,-9 C8,-9 10,-1 0,5 Z"
                    />
                  </g>
                </g>
                <g transform="translate(38 -70)">
                  <g class="tam-heart" style="--dx: 4px; --d: 1.1s">
                    <path
                      transform="scale(.85)"
                      d="M0,5 C-10,-1 -8,-9 -4,-9 C-2,-9 0,-7 0,-6 C0,-7 2,-9 4,-9 C8,-9 10,-1 0,5 Z"
                    />
                  </g>
                </g>
              </g>

              <!-- sleep: drifting Z shapes (paths, not text) -->
              <g v-if="activity === 'sleep'">
                <g transform="translate(28 -74)">
                  <g class="tam-z" style="--s: 1.7; --d: 0s">
                    <path class="tam-z-line" d="M-4,-4 H4 L-4,4 H4" />
                  </g>
                </g>
                <g transform="translate(28 -74)">
                  <g class="tam-z" style="--s: 1.3; --d: 1.2s">
                    <path class="tam-z-line" d="M-4,-4 H4 L-4,4 H4" />
                  </g>
                </g>
                <g transform="translate(28 -74)">
                  <g class="tam-z" style="--s: 1; --d: 2.4s">
                    <path class="tam-z-line" d="M-4,-4 H4 L-4,4 H4" />
                  </g>
                </g>
              </g>
            </g>
          </g>
        </g>
      </g>
    </g>
  </svg>
</template>

<style scoped>
.tam-sprite {
  width: 100%;
  height: auto;
  display: block;
  /* hearts, Z's and the leaf may swing a little past the frame */
  overflow: visible;
  /* creature palette (blob); species classes below override it */
  --c-shade: #16a085;
  --c-light: #1abc9c;
  --c-stroke: #0e7a65;
  --c-belly: #dff6f0;
}
.tam-sp-cat {
  --c-shade: #7189a2;
  --c-light: #8fa6bd;
  --c-stroke: #4f667d;
  --c-belly: #efe0c4;
}
.tam-sp-frog {
  --c-shade: #5f9732;
  --c-light: #7cb342;
  --c-stroke: #3f6f22;
  --c-belly: #f6f0a8;
}
.tam-sp-chick {
  --c-shade: #e9b937;
  --c-light: #f7d154;
  --c-stroke: #b88a14;
  --c-belly: #fff4c2;
}
.tam-sp-axolotl {
  --c-shade: #dd7d9b;
  --c-light: #f19bb4;
  --c-stroke: #b6527a;
  --c-belly: #e9dffa;
}

/* -------- shared ink -------- */
.tam-baby {
  --tam-sw: 2.6px;
}
.tam-adult {
  --tam-sw: 1.9px;
}
.tam-egg {
  --tam-sw: 2.6px;
}
.tam-ghost {
  --tam-sw: 2.2px;
}

.tam-ink {
  fill: #22313a;
}
.tam-glint {
  fill: #fff;
}
.tam-line {
  fill: none;
  stroke: #22313a;
  stroke-width: var(--tam-sw);
  stroke-linecap: round;
  stroke-linejoin: round;
}
.tam-line-thin {
  stroke-width: calc(var(--tam-sw) * 0.7);
}

.tam-shadow {
  fill: #000;
  opacity: 0.3;
}
.tam-shadow-ghost {
  opacity: 0.18;
}

/* -------- egg -------- */
.tam-egg-shade {
  fill: #e2d8ba;
  stroke: #b0a37f;
  stroke-width: var(--tam-sw);
  stroke-linejoin: round;
}
.tam-egg-light {
  fill: #f7f2e3;
}
.tam-speck-a {
  fill: var(--acc, #1abc9c);
}
.tam-speck-b {
  fill: var(--acc2, #16a085);
}
.tam-hole {
  fill: var(--acc3, #12806b);
  stroke: #8a7d5c;
  stroke-width: calc(var(--tam-sw) * 0.85);
  stroke-linejoin: round;
}
.tam-crack {
  fill: none;
  stroke: #8a7d5c;
  stroke-width: calc(var(--tam-sw) * 0.85);
  stroke-linecap: round;
  stroke-linejoin: round;
}

/* -------- creature -------- */
.tam-body-shade,
.tam-limb {
  fill: var(--c-shade);
  stroke: var(--c-stroke);
  stroke-width: var(--tam-sw);
  stroke-linejoin: round;
}
.tam-body-light {
  fill: var(--c-light);
}
.tam-belly {
  fill: var(--c-belly);
}
.tam-grime {
  fill: #8b6f47;
  opacity: 0.5;
}
.tam-blush {
  fill: #ff8fa3;
  opacity: 0.75;
}
.tam-tongue {
  fill: #ff8fa3;
}
.tam-drop {
  fill: #8fd8f5;
}
.tam-patch {
  fill: #b8e08f;
  opacity: 0.62;
}
.tam-rumble {
  fill: none;
  stroke: var(--c-shade);
  stroke-width: calc(var(--tam-sw) * 0.65);
  stroke-linecap: round;
  stroke-linejoin: round;
}
.tam-stink-line {
  fill: none;
  stroke: #8bc34a;
  stroke-width: calc(var(--tam-sw) * 0.85);
  stroke-linecap: round;
}
.tam-sprout-stem {
  fill: none;
  stroke: #0e7a65;
  stroke-width: var(--tam-sw);
  stroke-linecap: round;
}
.tam-sprout-leaf {
  fill: #86e0a0;
  stroke: #0e7a65;
  stroke-width: var(--tam-sw);
  stroke-linejoin: round;
}
.tam-sprout-vein {
  fill: none;
  stroke: #3fae6a;
  stroke-width: calc(var(--tam-sw) * 0.5);
  stroke-linecap: round;
}
.tam-crumb {
  fill: #e8b36b;
}
.tam-crumb-dark {
  fill: #c98f45;
}
.tam-heart {
  fill: #ff6b8b;
}
.tam-z-line {
  fill: none;
  stroke: #cfe8f7;
  stroke-width: calc(var(--tam-sw) * 0.8);
  stroke-linecap: round;
  stroke-linejoin: round;
}

/* -------- species parts -------- */
.tam-blush-soft {
  opacity: 0.55;
}
/* cat */
.tam-tail-out,
.tam-tail-in,
.tam-tail-tip {
  fill: none;
  stroke-linecap: round;
}
.tam-tail-out {
  stroke: var(--c-stroke);
  stroke-width: 11;
}
.tam-tail-in {
  stroke: var(--c-shade);
  stroke-width: 6.6;
}
.tam-tail-tip {
  stroke: #f6ead7;
  stroke-width: 6.6;
}
.tam-ear-in {
  fill: #f4b6c2;
  stroke: #f4b6c2;
  stroke-width: 3;
  stroke-linejoin: round;
}
.tam-muzzle {
  fill: #f6ead7;
}
.tam-nose {
  fill: #e88ea0;
  stroke: #e88ea0;
  stroke-width: 2.4;
  stroke-linejoin: round;
}
.tam-ruff {
  fill: #fffaf0;
  stroke: #e6d5b0;
  stroke-width: var(--tam-sw);
  stroke-linejoin: round;
}
.tam-whiskers {
  fill: none;
  stroke: #33475b;
  stroke-width: 3;
  stroke-linecap: round;
}
/* frog */
.tam-bump {
  fill: #9ccc65;
  stroke: var(--c-stroke);
  stroke-width: var(--tam-sw);
}
.tam-sclera {
  fill: #f4f8da;
}
.tam-lid {
  fill: var(--c-shade);
  transform: scaleY(0);
}
.tam-lid-l {
  transform-origin: -18px -66.6px;
}
.tam-lid-r {
  transform-origin: 18px -66.6px;
}
.tam-lid-half {
  transform: scaleY(0.5);
}
.tam-lid-shut {
  transform: scaleY(1);
}
.tam-spots {
  fill: #3f6f22;
  opacity: 0.45;
}
/* chick */
.tam-beak {
  fill: #f39c12;
  stroke: #c97a06;
  stroke-width: 2;
  stroke-linejoin: round;
}
.tam-foot {
  fill: #f39c12;
  stroke: #c97a06;
  stroke-width: var(--tam-sw);
  stroke-linejoin: round;
}
.tam-wing {
  fill: var(--c-shade);
  stroke: var(--c-stroke);
  stroke-width: var(--tam-sw);
  stroke-linejoin: round;
}
.tam-wing-patch {
  fill: #fffbe6;
}
.tam-curl {
  fill: none;
  stroke: #d9a521;
  stroke-width: 3.4;
  stroke-linecap: round;
}
/* axolotl */
.tam-gill-shape {
  fill: #e06a92;
  stroke: #b6527a;
  stroke-width: var(--tam-sw);
  stroke-linejoin: round;
}
.tam-gill-hi {
  fill: #f6a0bd;
}
.tam-fin-rib {
  fill: none;
  stroke: #f6a0bd;
  stroke-width: 3;
  stroke-linecap: round;
}

/* -------- poop -------- */
.tam-poo-tip {
  fill: #8b6f47;
  stroke: #6e5636;
  stroke-width: 1;
  stroke-linejoin: round;
}
.tam-poo-shine {
  fill: none;
  stroke: #c9a97a;
  stroke-width: 1.4;
  stroke-linecap: round;
  opacity: 0.75;
}
.tam-poo-smile {
  fill: none;
  stroke: #22313a;
  stroke-width: 1;
  stroke-linecap: round;
}

/* -------- ghost -------- */
.tam-ghost-body {
  opacity: 0.8;
}
.tam-ghost-shade {
  fill: #8e99a1;
  stroke: #6d7880;
  stroke-width: var(--tam-sw);
  stroke-linejoin: round;
}
.tam-ghost-light {
  fill: #c4cdd3;
}
.tam-ghost-ink {
  stroke: #3a464e;
}
.tam-ghost-mouth {
  fill: #3a464e;
}
.tam-halo {
  fill: none;
  stroke: var(--tam-halo, #e9d98a);
  stroke-width: var(--tam-sw);
}

/* ====================== animation ====================== */
/* Every SVG transform below pivots in the local user space of its group:
   (0,0) is the point where the creature touches the ground. */

.tam-enter {
  transform-origin: 0px 0px;
  animation: tam-pop 0.5s cubic-bezier(0.2, 0.9, 0.3, 1.3) both;
}

.tam-actor {
  transform-origin: 0px 0px;
}
.tam-act-idle {
  animation: tam-bob 3s ease-in-out infinite;
}
.tam-act-eat {
  animation: tam-munch 0.6s ease-in-out infinite;
}
.tam-act-play {
  animation: tam-bounce 1.1s ease-in-out infinite;
}
.tam-act-pet {
  animation: tam-sway 1.8s ease-in-out infinite;
}
.tam-act-sleep {
  /* lying down: tilted and squashed, breathing slowly */
  transform: rotate(-4deg) scale(1.04, 0.93);
  animation: tam-breathe 3.6s ease-in-out infinite;
}

.tam-shadow-play {
  transform-origin: 0px 2px;
  animation: tam-shadow-play 1.1s ease-in-out infinite;
}

.tam-egg-wobble {
  transform-origin: 0px -2px;
  animation: tam-egg-wobble 4s ease-in-out infinite;
}
.tam-hatch-wobble {
  transform-origin: 0px -2px;
  animation: tam-hatch-wobble 0.8s ease-in-out infinite;
}
.tam-peek {
  transform-origin: 0px -77px;
  animation: tam-blink 3.2s ease-in-out infinite;
}

.tam-blink {
  transform-origin: 0px -54px;
  animation: tam-blink 4.8s ease-in-out infinite;
}
.tam-chew {
  transform-origin: 0px -42px;
  animation: tam-chew 0.6s ease-in-out infinite;
}
.tam-spin {
  transform-origin: 0px 0px;
  animation: tam-spin 1.6s linear infinite;
}
.tam-spin-rev {
  animation-direction: reverse;
}
.tam-sprout {
  transform-origin: 0px -84px;
  animation: tam-sprout 3.4s ease-in-out infinite alternate;
}

.tam-crumb {
  animation: tam-crumb 1.2s ease-in infinite;
  animation-delay: var(--d);
  animation-fill-mode: backwards;
}
.tam-heart {
  transform-origin: 0px 0px;
  animation: tam-heart 2.2s ease-out infinite;
  animation-delay: var(--d);
  animation-fill-mode: backwards;
}
.tam-z {
  transform-origin: 0px 0px;
  animation: tam-z 3.6s ease-out infinite;
  animation-delay: var(--d);
  animation-fill-mode: backwards;
}
.tam-stink {
  animation: tam-stink 2.6s ease-in-out infinite;
  animation-delay: var(--d);
  animation-fill-mode: backwards;
}
.tam-drop-fall {
  transform-origin: 0px 0px;
  animation: tam-tear 2.4s ease-in infinite;
}
.tam-sweat {
  animation-duration: 3s;
}
.tam-drool {
  transform-origin: 0px 0px;
  animation: tam-drool 1.8s ease-in infinite;
}
.tam-rumble {
  animation: tam-rumble 0.45s ease-in-out infinite alternate;
}

.tam-poo {
  transform-origin: 0px 0px;
  animation: tam-poo 2.6s ease-in-out infinite;
  animation-delay: var(--d);
}

.tam-ghost-actor {
  transform: translateY(-16px);
  animation: tam-ghost-float 4s ease-in-out infinite;
}
.tam-shadow-ghost {
  transform-origin: 0px 2px;
  animation: tam-ghost-shadow 4s ease-in-out infinite;
}
.tam-halo {
  transform-origin: 0px -96px;
  animation: tam-halo 3s ease-in-out infinite alternate;
}

/* ---- species motion ---- */
/* cat + axolotl tails */
.tam-tail {
  animation: tam-tail 2.6s ease-in-out infinite alternate;
}
.tam-tail-cat {
  transform-origin: 36px -12px;
}
.tam-tail-axo {
  transform-origin: 32px -14px;
}
.tam-act-play .tam-tail {
  animation-duration: 0.5s;
}
.tam-act-pet .tam-tail {
  animation-duration: 0.9s;
}
.tam-act-sleep .tam-tail {
  animation-duration: 6s;
}
/* cat ears twitch now and then */
.tam-ear-l {
  transform-origin: -27px -76px;
  animation: tam-ear-l 7s ease-in-out infinite;
}
.tam-ear-r {
  transform-origin: 27px -76px;
  animation: tam-ear-r 5.5s ease-in-out 1.2s infinite;
}
/* frog eyelids */
.tam-lid-blink {
  animation: tam-lid 4.8s ease-in-out infinite;
}
/* chick wings: tiny idle flutter, real flapping while playing / petted */
.tam-wing-l {
  transform-origin: -41px -54px;
  animation: tam-wing-l 4s ease-in-out infinite;
}
.tam-wing-r {
  transform-origin: 41px -54px;
  animation: tam-wing-r 4s ease-in-out infinite;
}
.tam-act-play .tam-wing-l {
  animation: tam-flap-l 0.34s ease-in-out infinite alternate;
}
.tam-act-play .tam-wing-r {
  animation: tam-flap-r 0.34s ease-in-out infinite alternate;
}
.tam-act-pet .tam-wing-l {
  animation: tam-flap-l 0.5s ease-in-out infinite alternate;
}
.tam-act-pet .tam-wing-r {
  animation: tam-flap-r 0.5s ease-in-out infinite alternate;
}
.tam-act-sleep .tam-wing-l,
.tam-act-sleep .tam-wing-r {
  animation: none;
}
/* axolotl gills pulse and wiggle */
.tam-gill {
  transform-origin: 0px 0px;
  animation: tam-gill 2.4s ease-in-out infinite alternate;
  animation-delay: var(--d);
}
.tam-act-play .tam-gill {
  animation-duration: 0.6s;
}
.tam-act-pet .tam-gill {
  animation-duration: 1.2s;
}
.tam-act-sleep .tam-gill {
  animation-duration: 5s;
}

/* face swaps fade in/out instead of snapping */
.tam-fade-enter-active {
  transition: opacity 0.22s ease;
}
.tam-fade-leave-active {
  transition: opacity 0.15s ease;
}
.tam-fade-enter-from,
.tam-fade-leave-to {
  opacity: 0;
}

@keyframes tam-pop {
  0% {
    transform: scale(0.55);
    opacity: 0;
  }
  55% {
    transform: scale(1.07);
    opacity: 1;
  }
  100% {
    transform: scale(1);
    opacity: 1;
  }
}

@keyframes tam-bob {
  0%,
  100% {
    transform: translateY(0) scale(1, 1);
  }
  50% {
    transform: translateY(-3px) scale(0.985, 1.025);
  }
}
@keyframes tam-munch {
  0%,
  100% {
    transform: translateY(0) rotate(0) scale(1, 1);
  }
  25% {
    transform: translateY(0.5px) rotate(-1.5deg) scale(1.02, 0.97);
  }
  50% {
    transform: translateY(1px) rotate(0) scale(1.03, 0.95);
  }
  75% {
    transform: translateY(0.5px) rotate(1.5deg) scale(1.02, 0.97);
  }
}
@keyframes tam-bounce {
  0%,
  50%,
  100% {
    transform: translateY(0) rotate(0) scale(1.07, 0.9);
  }
  25% {
    transform: translateY(-12px) rotate(-7deg) scale(0.96, 1.06);
  }
  75% {
    transform: translateY(-12px) rotate(7deg) scale(0.96, 1.06);
  }
}
@keyframes tam-shadow-play {
  0%,
  50%,
  100% {
    transform: scale(1);
    opacity: 0.3;
  }
  25%,
  75% {
    transform: scale(0.8);
    opacity: 0.18;
  }
}
@keyframes tam-sway {
  0%,
  100% {
    transform: rotate(-2.5deg) scale(1.02, 0.98);
  }
  50% {
    transform: rotate(2.5deg) scale(0.99, 1.02);
  }
}
@keyframes tam-breathe {
  0%,
  100% {
    transform: rotate(-4deg) scale(1.04, 0.93);
  }
  50% {
    transform: rotate(-4deg) scale(1.02, 0.96);
  }
}

@keyframes tam-egg-wobble {
  0%,
  62%,
  100% {
    transform: rotate(0);
  }
  68% {
    transform: rotate(-4deg);
  }
  74% {
    transform: rotate(3.5deg);
  }
  80% {
    transform: rotate(-2.5deg);
  }
  86% {
    transform: rotate(1.5deg);
  }
  92% {
    transform: rotate(0);
  }
}
@keyframes tam-hatch-wobble {
  0%,
  100% {
    transform: rotate(0);
  }
  15% {
    transform: rotate(-8deg);
  }
  30% {
    transform: rotate(7deg);
  }
  45% {
    transform: rotate(-5deg);
  }
  60% {
    transform: rotate(4deg);
  }
  75% {
    transform: rotate(-1.5deg);
  }
}

@keyframes tam-blink {
  0%,
  90%,
  100% {
    transform: scaleY(1);
  }
  94% {
    transform: scaleY(0.1);
  }
  97% {
    transform: scaleY(1);
  }
}
@keyframes tam-chew {
  0%,
  100% {
    transform: scale(1, 1);
  }
  50% {
    transform: scale(1.15, 1.6);
  }
}
@keyframes tam-spin {
  to {
    transform: rotate(360deg);
  }
}
@keyframes tam-sprout {
  from {
    transform: rotate(-4deg);
  }
  to {
    transform: rotate(5deg);
  }
}

@keyframes tam-crumb {
  0% {
    transform: translate(0, 0) scale(0.3);
    opacity: 0;
  }
  15% {
    transform: translate(calc(var(--dx) * 0.4), -6px) scale(1);
    opacity: 1;
  }
  100% {
    transform: translate(var(--dx), 26px) scale(0.8);
    opacity: 0;
  }
}
@keyframes tam-heart {
  0% {
    transform: translate(0, 0) scale(0.4);
    opacity: 0;
  }
  20% {
    transform: translate(0, -4px) scale(1);
    opacity: 1;
  }
  100% {
    transform: translate(var(--dx), -24px) scale(0.9);
    opacity: 0;
  }
}
@keyframes tam-z {
  0% {
    transform: translate(0, 0) scale(calc(var(--s) * 0.6));
    opacity: 0;
  }
  15% {
    opacity: 1;
  }
  100% {
    transform: translate(16px, -28px) scale(var(--s));
    opacity: 0;
  }
}
@keyframes tam-stink {
  0% {
    transform: translateY(4px);
    opacity: 0;
  }
  30% {
    opacity: 0.9;
  }
  100% {
    transform: translateY(-6px);
    opacity: 0;
  }
}
@keyframes tam-tear {
  0%,
  15% {
    transform: translateY(0) scale(0.6);
    opacity: 0;
  }
  25% {
    transform: translateY(0) scale(1);
    opacity: 1;
  }
  80% {
    transform: translateY(14px) scale(1);
    opacity: 1;
  }
  100% {
    transform: translateY(18px) scale(1);
    opacity: 0;
  }
}
@keyframes tam-drool {
  0% {
    transform: translateY(0) scale(1, 0.3);
    opacity: 1;
  }
  45% {
    transform: translateY(0) scale(1, 1);
    opacity: 1;
  }
  100% {
    transform: translateY(12px) scale(1, 1);
    opacity: 0;
  }
}
@keyframes tam-rumble {
  from {
    transform: translateX(-1.4px);
  }
  to {
    transform: translateX(1.4px);
  }
}
@keyframes tam-poo {
  0%,
  100% {
    transform: scale(1, 1);
  }
  50% {
    transform: scale(1.03, 0.95);
  }
}

@keyframes tam-tail {
  from {
    transform: rotate(-7deg);
  }
  to {
    transform: rotate(8deg);
  }
}
@keyframes tam-ear-l {
  0%,
  90%,
  100% {
    transform: rotate(0);
  }
  93% {
    transform: rotate(-8deg);
  }
  96% {
    transform: rotate(2deg);
  }
}
@keyframes tam-ear-r {
  0%,
  88%,
  100% {
    transform: rotate(0);
  }
  91% {
    transform: rotate(8deg);
  }
  95% {
    transform: rotate(-2deg);
  }
}
@keyframes tam-lid {
  0%,
  90%,
  100% {
    transform: scaleY(0);
  }
  94% {
    transform: scaleY(1);
  }
  97% {
    transform: scaleY(0);
  }
}
@keyframes tam-wing-l {
  0%,
  100% {
    transform: rotate(0);
  }
  50% {
    transform: rotate(4deg);
  }
}
@keyframes tam-wing-r {
  0%,
  100% {
    transform: rotate(0);
  }
  50% {
    transform: rotate(-4deg);
  }
}
@keyframes tam-flap-l {
  from {
    transform: rotate(0);
  }
  to {
    transform: rotate(34deg);
  }
}
@keyframes tam-flap-r {
  from {
    transform: rotate(0);
  }
  to {
    transform: rotate(-34deg);
  }
}
@keyframes tam-gill {
  from {
    transform: rotate(-9deg) scale(1, 1);
  }
  to {
    transform: rotate(9deg) scale(1.1, 1.14);
  }
}

@keyframes tam-ghost-float {
  0%,
  100% {
    transform: translateY(-12px);
  }
  50% {
    transform: translateY(-22px);
  }
}
@keyframes tam-ghost-shadow {
  0%,
  100% {
    transform: scale(1);
    opacity: 0.18;
  }
  50% {
    transform: scale(0.82);
    opacity: 0.1;
  }
}
@keyframes tam-halo {
  from {
    transform: rotate(-5deg) translateY(0);
  }
  to {
    transform: rotate(5deg) translateY(-1.5px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .tam-sprite,
  .tam-sprite * {
    animation: none !important;
    transition: none !important;
  }
}
</style>
