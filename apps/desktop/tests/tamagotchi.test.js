import { describe, it, expect, beforeEach } from "vitest";
import { calls, handle } from "./tauri.js";
import { LEGACY_KEY, adoptLegacyPet } from "../src/tamagotchi";

// The pet's rules live in crates/tools/src/pet.rs (and its tests); the
// webview keeps only the one-time move of the pre-1.11.1 localStorage pet.

const OLD = { v: 1, created: 1, lastTick: 2, species: "frog", food: 50, generations: 3 };
const adoptCalls = () => calls.filter((c) => c.cmd === "adopt_pet");

beforeEach(() => {
  localStorage.clear();
});

describe("legacy pet adoption", () => {
  it("does nothing without an old save", async () => {
    expect(await adoptLegacyPet()).toBe(false);
    expect(adoptCalls()).toHaveLength(0);
  });

  it("hands the old save to the backend once and drops it", async () => {
    localStorage.setItem(LEGACY_KEY, JSON.stringify(OLD));
    handle("adopt_pet", () => true);
    expect(await adoptLegacyPet()).toBe(true);
    expect(adoptCalls().at(-1).args).toEqual({ saved: OLD });
    expect(localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  it("drops the save when the backend already has a pet", async () => {
    localStorage.setItem(LEGACY_KEY, JSON.stringify(OLD));
    handle("adopt_pet", () => false);
    expect(await adoptLegacyPet()).toBe(false);
    expect(localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  it("keeps the save for the next launch while the backend is down", async () => {
    localStorage.setItem(LEGACY_KEY, JSON.stringify(OLD));
    handle("adopt_pet", () => {
      throw "no backend";
    });
    expect(await adoptLegacyPet()).toBe(false);
    expect(localStorage.getItem(LEGACY_KEY)).not.toBeNull();
  });

  it("drops an unreadable save without asking the backend", async () => {
    localStorage.setItem(LEGACY_KEY, "{not json");
    expect(await adoptLegacyPet()).toBe(false);
    expect(adoptCalls()).toHaveLength(0);
    expect(localStorage.getItem(LEGACY_KEY)).toBeNull();
  });
});
