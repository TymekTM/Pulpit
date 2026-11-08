// Seasonal themes ("tryb świąteczny" and friends,
// .scratch/christmas-mode/spec.md).
// The backend decides the season (pulpit-season): `get_season` gives the
// first paint, and the `season` custom value the season producer pushes
// keeps it live (midnight rollover, a mode switch from Settings). The
// active season lands on <html data-season="..."> so plain CSS can skin
// the chrome; components read `season` for the animated extras.
import { computed, ref } from "vue";
import { api } from "./api.js";

/// Every season the backend can push, in Settings order (pulpit-season
/// `Season::wire_name`).
export const SEASONS = ["christmas", "halloween", "easter", "valentines"];
/// Stored choices: follow the calendar, never, or force one season.
export const SEASON_MODES = ["auto", "off", ...SEASONS];

/// The stored choice in Settings.
export const seasonMode = ref("auto");
/// The effective season: one of SEASONS, or "off".
export const season = ref("off");

/// Whether any seasonal theme is showing.
export const seasonOn = computed(() => season.value !== "off");

/// Normalize anything pushed or returned to a known season name.
export function normalizeSeason(value) {
  return SEASONS.includes(value) ? value : "off";
}

/// Apply an effective season to the reactive state and the <html> flag.
export function applySeason(value) {
  const next = normalizeSeason(value);
  season.value = next;
  if (typeof document !== "undefined") {
    if (next === "off") delete document.documentElement.dataset.season;
    else document.documentElement.dataset.season = next;
  }
}

function applyPayload(payload) {
  if (SEASON_MODES.includes(payload?.mode)) seasonMode.value = payload.mode;
  applySeason(payload?.season);
}

/// First paint: ask the backend. A failure leaves the plain skin.
export async function initSeason() {
  try {
    applyPayload(await api.getSeason());
  } catch {
    applySeason("off");
  }
}

/// Settings -> General. Throws the backend's message on a refused save
/// (the stored choice stays as it was).
export async function setSeasonMode(mode) {
  applyPayload(await api.setSeason(mode));
}

/// Animations stay off when the OS asks for reduced motion.
export function prefersReducedMotion() {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}
