// The desk pet lives in the backend (crates/tools/src/pet.rs): the
// simulation, care gestures and persistence (`~/pulpitApp/pet.json`) all
// run there, so the pet keeps living with the editor window closed. The
// editor only renders the snapshot pushed on the `pet` channel, like
// every tablet. What is left here is the one-time move of a pre-1.11.1
// desktop pet, which the webview used to simulate and save to
// localStorage.
import { api } from "./api.js";

export const LEGACY_KEY = "pulpit_tamagotchi_v1";

/// Hand the old localStorage pet to the backend once. The backend takes
/// it only while its own pet is a fresh, uncared-for egg; the save is
/// dropped either way after a definite answer, so it never comes back.
export async function adoptLegacyPet() {
  let raw = null;
  try {
    raw = localStorage.getItem(LEGACY_KEY);
  } catch {
    return false;
  }
  if (!raw) return false;
  let saved;
  try {
    saved = JSON.parse(raw);
  } catch {
    saved = null;
  }
  let adopted = false;
  if (saved && typeof saved === "object") {
    try {
      adopted = await api.adoptPet(saved);
    } catch {
      // backend down: keep the save for the next launch
      return false;
    }
  }
  try {
    localStorage.removeItem(LEGACY_KEY);
  } catch {}
  return adopted;
}
