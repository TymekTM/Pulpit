// The desk pet: a small tamagotchi living in the editor sidebar. Every
// rule is a pure function of (state, now), so tests inject the clock and
// never wait; the panel only renders and forwards button presses. State
// persists in localStorage and advances in fixed one-minute steps, so an
// offline stretch is the same simulation as an idle app (capped, so a
// week of holiday cannot kill the pet - OFFLINE_MAX_MS of it counts).
import { ref } from "vue";

const H = 3_600_000;
const M = 60_000;

/// The panel tick; re-renders stat bars and re-checks moods each second.
export const TICK_MS = 1_000;

/// One simulation step. Stats drain linearly, so a coarse step only
/// blurs event timing by up to a minute - invisible at pet pace, and it
/// keeps the poo schedule, neglect counter and auto-wake in sync without
/// solving piecewise equations.
const STEP_MS = M;

/// Points lost per hour while awake.
export const RATES = {
  food: 22, // full bowl to empty in ~4.5 h
  fun: 16,
  rest: 10, // awake for ~10 h before it must sleep
  clean: 5,
  cleanPerPoo: 7, // each uncleaned poo adds this much
  restGainAsleep: 35, // empty to full overnight in ~3 h
  foodAsleep: 22 * 0.4, // sleep slows, never stops, the drain
  funAsleep: 16 * 0.4,
};

export const HATCH_MS = 3 * M; // egg -> baby
export const HATCHING_MS = 45_000; // the cracking finale of the egg
export const GROW_MS = 24 * H; // baby -> adult
/// Cumulative time with any stat at rock bottom kills the pet. Long
/// enough that a work session never dies, short enough that "not today,
/// pet" has consequences.
export const NEGLECT_DEATH_MS = 12 * H;
/// Decay applied on load for the time the app was closed. The worst-case
/// offline stretch (all stats zero, but under the neglect threshold)
/// survives it, so closing the app never kills - only ignoring an open
/// one does.
export const OFFLINE_MAX_MS = 18 * H;

const POO_MIN_MS = 45 * M;
const POO_MAX_MS = 90 * M;
export const MAX_POOS = 3;
export const PET_COOLDOWN_MS = 30_000;

const KEY = "pulpit_tamagotchi_v1";

/// The English message key for each mood's one-word caption.
export const MOOD_LABEL_KEYS = {
  happy: "Beaming",
  neutral: "Content",
  sad: "Bored",
  hungry: "Hungry",
  sleepy: "Sleepy",
  dirty: "Needs a bath",
  dead: "R.I.P.",
};

/// Species the sprite can draw. Cosmetic only - the rules do not care.
/// "blob" first: it is the fallback and the rng0-deterministic default.
export const SPECIES = ["blob", "cat", "frog", "chick", "axolotl"];
const SPECIES_SET = new Set(SPECIES);
export const SPECIES_LABEL_KEYS = {
  blob: "Blob",
  cat: "Cat",
  frog: "Frog",
  chick: "Chick",
  axolotl: "Axolotl",
};

export function normalizeSpecies(v) {
  return SPECIES_SET.has(v) ? v : "blob";
}

const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

/// A fresh egg. `generations` and `bestAgeMs` survive the previous pet;
/// `species` is drawn at random (injectable for tests) unless given.
export function createEgg(now = Date.now(), generations = 1, bestAgeMs = 0, species = null, rng = Math.random) {
  return {
    v: 1,
    created: now,
    lastTick: now,
    hatchedAt: null,
    diedAt: null,
    species: species ? normalizeSpecies(species) : SPECIES[Math.floor(rng() * SPECIES.length)],
    food: 100,
    fun: 100,
    rest: 100,
    clean: 100,
    poos: 0,
    sleeping: false,
    nextPooAt: now + pooDelay(),
    neglectMs: 0,
    generations,
    bestAgeMs,
    lastPetAt: 0,
    cares: 0,
  };
}

/// Minutes until the next poo, 45-90, injectable for deterministic tests.
export function pooDelay(rng = Math.random) {
  return POO_MIN_MS + rng() * (POO_MAX_MS - POO_MIN_MS);
}

/// The visible life phase, straight from the timestamps.
export function phaseOf(s, now = Date.now()) {
  if (s.diedAt !== null) return "dead";
  if (s.hatchedAt === null) {
    return now >= s.created + HATCH_MS - HATCHING_MS ? "hatching" : "egg";
  }
  return now >= s.hatchedAt + GROW_MS ? "adult" : "baby";
}

/// Which face the sprite pulls. Priority: death, sleep, then the loudest
/// need below 20 points; otherwise mood follows the average.
export function moodOf(s, now = Date.now()) {
  if (s.diedAt !== null) return "dead";
  if (s.sleeping) return "sleepy";
  if (s.clean < 20 || s.poos >= MAX_POOS) return "dirty";
  if (s.food < 20) return "hungry";
  if (s.rest < 20) return "sleepy";
  if (s.fun < 20) return "sad";
  const avg = (s.food + s.fun + s.rest + s.clean) / 4;
  return avg < 45 ? "neutral" : "happy";
}

/// Lived time: from the hatch to death, or to `now` while alive.
export function ageOf(s, now = Date.now()) {
  if (s.hatchedAt === null) return 0;
  return Math.max(0, (s.diedAt ?? now) - s.hatchedAt);
}

/// Everything the sprite needs, in one bag.
export function spriteProps(s, now = Date.now()) {
  return {
    stage: phaseOf(s, now),
    mood: moodOf(s, now),
    activity: s.sleeping ? "sleep" : "idle",
    poos: s.poos,
    species: s.species,
  };
}

/// Run the simulation from `state.lastTick` to `now`. Returns a new
/// state (the input is never mutated) plus the events crossed, oldest
/// first: "hatch", "grow", "poo", "wake", "die". With `opts.offline`
/// the neglect counter stands still - draining a closed app's stats is
/// the pet boarding at a farm, never dying at one.
export function advance(state, now, rng = Math.random, opts = {}) {
  const events = [];
  const s = { ...state };
  let t = s.lastTick;
  let wasBaby = s.hatchedAt !== null && phaseOf(s, t) === "baby";

  while (t < now && s.diedAt === null) {
    const dt = Math.min(STEP_MS, now - t);
    t += dt;

    if (s.hatchedAt === null) {
      // the egg needs no care, only time
      if (t >= s.created + HATCH_MS) {
        s.hatchedAt = s.created + HATCH_MS;
        wasBaby = true;
        events.push("hatch");
      }
      continue;
    }

    if (s.sleeping) {
      s.rest = clamp(s.rest + (RATES.restGainAsleep * dt) / H, 0, 100);
      s.food = Math.max(0, s.food - (RATES.foodAsleep * dt) / H);
      s.fun = Math.max(0, s.fun - (RATES.funAsleep * dt) / H);
      if (s.rest >= 100) {
        s.sleeping = false;
        events.push("wake");
      }
    } else {
      s.food = Math.max(0, s.food - (RATES.food * dt) / H);
      s.fun = Math.max(0, s.fun - (RATES.fun * dt) / H);
      s.rest = Math.max(0, s.rest - (RATES.rest * dt) / H);
      s.clean = Math.max(0, s.clean - ((RATES.clean + s.poos * RATES.cleanPerPoo) * dt) / H);
    }

    if (t >= s.nextPooAt) {
      if (!s.sleeping && s.food > 15 && s.poos < MAX_POOS) {
        s.poos += 1;
        events.push("poo");
      }
      s.nextPooAt = t + pooDelay(rng);
    }

    const grown = phaseOf(s, t) === "adult";
    if (grown && wasBaby) {
      wasBaby = false;
      events.push("grow");
    }

    if (!opts.offline && (s.food <= 0 || s.fun <= 0 || s.rest <= 0 || s.clean <= 0)) {
      s.neglectMs += dt;
      if (s.neglectMs >= NEGLECT_DEATH_MS) {
        s.neglectMs = NEGLECT_DEATH_MS;
        s.diedAt = t;
        s.sleeping = false;
        s.bestAgeMs = Math.max(s.bestAgeMs, t - s.hatchedAt);
        events.push("die");
      }
    }
  }
  s.lastTick = now;
  return { state: s, events };
}

/// Why a button is unavailable, or null when it is not. `newEgg` has no
/// blocked state: the panel only offers it on a dead pet anyway.
export function blockedReason(s, name, now = Date.now()) {
  if (name === "newEgg") return null;
  const phase = phaseOf(s, now);
  if (phase === "dead") return "dead";
  if (phase === "egg" || phase === "hatching") return "egg";
  switch (name) {
    case "feed":
      return s.food >= 92 ? "full" : null;
    case "play":
      if (s.sleeping) return "asleep";
      if (s.food <= 8) return "hungry";
      if (s.rest <= 8) return "tired";
      return null;
    case "pet":
      if (s.sleeping) return "asleep";
      if (now - s.lastPetAt < PET_COOLDOWN_MS) return "cooldown";
      return null;
    case "clean":
      return s.clean >= 100 && s.poos === 0 ? "clean" : null;
    case "sleep":
      return null; // the toggle goes both ways
    default:
      return "unknown";
  }
}

/// Apply one care action. Pure: returns the next state, whether it went
/// through, and the `blockedReason` when it did not.
export function applyAction(state, name, now = Date.now(), rng = Math.random) {
  const reason = blockedReason(state, name, now);
  if (reason) return { state, ok: false, reason };

  if (name === "newEgg") {
    // the next generation hatches a freshly drawn species
    return { state: createEgg(now, state.generations + 1, state.bestAgeMs, null, rng), ok: true };
  }

  const s = { ...state };
  switch (name) {
    case "feed":
      s.food = Math.min(100, s.food + 26);
      s.sleeping = false; // dinner wakes
      s.cares += 1;
      break;
    case "play":
      s.fun = Math.min(100, s.fun + 22);
      s.food = Math.max(0, s.food - 6);
      s.rest = Math.max(0, s.rest - 8);
      s.cares += 1;
      break;
    case "pet":
      s.fun = Math.min(100, s.fun + 8);
      s.lastPetAt = now;
      s.cares += 1;
      break;
    case "clean":
      s.poos = 0;
      s.clean = 100;
      s.fun = Math.min(100, s.fun + 4);
      s.cares += 1;
      break;
    case "sleep":
      s.sleeping = !s.sleeping;
      break;
  }
  return { state: s, ok: true };
}

function sanitize(v) {
  if (!v || typeof v !== "object" || !Number.isFinite(v.created) || !Number.isFinite(v.lastTick)) {
    return null;
  }
  return {
    v: 1,
    created: v.created,
    lastTick: v.lastTick,
    hatchedAt: Number.isFinite(v.hatchedAt) ? v.hatchedAt : null,
    diedAt: Number.isFinite(v.diedAt) ? v.diedAt : null,
    food: clamp(+v.food || 0, 0, 100),
    fun: clamp(+v.fun || 0, 0, 100),
    rest: clamp(+v.rest || 0, 0, 100),
    clean: clamp(+v.clean || 0, 0, 100),
    poos: clamp(Math.floor(+v.poos || 0), 0, MAX_POOS),
    species: normalizeSpecies(v.species),
    sleeping: !!v.sleeping && v.diedAt == null,
    nextPooAt: Number.isFinite(v.nextPooAt) ? v.nextPooAt : v.lastTick + pooDelay(),
    neglectMs: Math.max(0, +v.neglectMs || 0),
    generations: Math.max(1, Math.floor(+v.generations || 1)),
    bestAgeMs: Math.max(0, +v.bestAgeMs || 0),
    lastPetAt: Number.isFinite(v.lastPetAt) ? v.lastPetAt : 0,
    cares: Math.max(0, Math.floor(+v.cares || 0)),
  };
}

export function savePet(state) {
  try {
    localStorage.setItem(KEY, JSON.stringify(state));
  } catch {
    // a full or blocked store must never take the editor down
  }
}

export function loadPet() {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? sanitize(JSON.parse(raw)) : null;
  } catch {
    return null;
  }
}

/// Load (or lay) the pet and advance it to `now`, applying the offline
/// cap. Returns the live state plus the events that happened meanwhile,
/// so the panel can toast "it hatched!" even for the offline hatch.
export function initPet(now = Date.now()) {
  let s = loadPet();
  if (!s) return { state: createEgg(now), events: [] };

  if (now - s.lastTick > OFFLINE_MAX_MS) {
    // only the first OFFLINE_MAX_MS of the absence counts, and the
    // neglect counter stands still for it: the pet comes back drained
    // to zero but never dead
    const r = advance(s, s.lastTick + OFFLINE_MAX_MS, Math.random, { offline: true });
    s = r.state;
    s.lastTick = now;
    return { state: s, events: [...r.events, "missedYou"] };
  }
  const r = advance(s, now);
  return { state: r.state, events: r.events };
}

// ---- reactive shell -----------------------------------------------------
//
// Module-level singleton (the season theme's pattern): the sidebar
// button and the panel share one pet without prop drilling.

export const pet = ref(null);
/// Newest last; the panel keeps an index of what it has shown.
export const petEvents = ref([]);

let petTimer = null;
let ticksSinceSave = 0;

export function startPet(now = Date.now()) {
  stopPet();
  const { state, events } = initPet(now);
  pet.value = state;
  pushEvents(events);
  savePet(state);
  petTimer = setInterval(tickPet, TICK_MS);
  return state;
}

export function stopPet() {
  if (petTimer !== null) {
    clearInterval(petTimer);
    petTimer = null;
  }
}

function tickPet() {
  const { state, events } = advance(pet.value, Date.now());
  pet.value = state;
  if (events.length) pushEvents(events);
  // events always persist at once; the routine drain every ~15 s is
  // cheap to redo from lastTick if the app dies in between
  if (events.length || ++ticksSinceSave >= 15) {
    ticksSinceSave = 0;
    savePet(state);
  }
}

/// A button press from the UI. Applies, stores, returns the result.
export function petDo(name, now = Date.now()) {
  const r = applyAction(pet.value, name, now);
  if (r.ok) {
    pet.value = r.state;
    savePet(r.state);
  }
  return r;
}

function pushEvents(list) {
  // append-only: the panel tracks what it has shown by index, so the
  // queue must never be compacted from the front
  petEvents.value = [...petEvents.value, ...list];
}
