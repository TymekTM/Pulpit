import { describe, it, expect, afterEach, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import TamagotchiPanel from "../src/components/TamagotchiPanel.vue";
import { t } from "../src/i18n";
import {
  HATCH_MS,
  NEGLECT_DEATH_MS,
  advance,
  createEgg,
  pet,
  petEvents,
  pooDelay,
} from "../src/tamagotchi";

const M = 60_000;
const H = 3_600_000;
const T0 = 1_700_000_000_000;
const rng0 = () => 0;

function baby(now = T0, over = {}) {
  // "blob" pinned: createEgg otherwise draws the species (like the poo
  // schedule) from Math.random, racing the assertions
  const s = createEgg(now - HATCH_MS, 1, 0, "blob");
  const base = advance({ ...s, nextPooAt: now + pooDelay(rng0) }, now, rng0).state;
  return { ...base, ...over };
}

let mountedWrappers = [];
function mountPanel() {
  const w = mount(TamagotchiPanel);
  mountedWrappers.push(w);
  return w;
}

afterEach(() => {
  // every panel parks a window-capture keydown listener; without this
  // the stale ones would eat each other's Escape
  mountedWrappers.forEach((w) => w.unmount());
  mountedWrappers = [];
});

beforeEach(() => {
  localStorage.clear();
  pet.value = baby();
  petEvents.value = [];
});

describe("TamagotchiPanel", () => {
  it("shows the stats and the care buttons for a live pet", () => {
    const w = mountPanel();
    expect(w.findAll(".tam-stat")).toHaveLength(4);
    const labels = w.findAll(".tam-stat-label").map((n) => n.text());
    expect(labels).toEqual([t("Food"), t("Fun"), t("Rest"), t("Cleanliness")]);
    const buttons = w.findAll(".tam-act").map((n) => n.text());
    expect(buttons).toEqual([t("Feed"), t("Play"), t("Pet"), t("Clean"), t("Sleep")]);
    expect(w.find(".tam-rip").exists()).toBe(false);
  });

  it("feeding from the panel feeds the shared pet", async () => {
    pet.value = baby(T0, { food: 40 });
    const w = mountPanel();
    await w.findAll(".tam-act")[0].trigger("click");
    expect(pet.value.food).toBe(66);
  });

  it("a blocked press explains itself in the button title", async () => {
    pet.value = baby(T0, { food: 95 });
    const w = mountPanel();
    const feed = w.findAll(".tam-act")[0];
    expect(feed.attributes("disabled")).toBeDefined();
    expect(feed.attributes("title")).toBe(t("Too full to eat"));
    await feed.trigger("click");
    expect(pet.value.food).toBe(95);
  });

  it("the sleep button flips to wake and back", async () => {
    const w = mountPanel();
    const sleep = w.findAll(".tam-act")[4];
    expect(sleep.text()).toBe(t("Sleep"));
    await sleep.trigger("click");
    expect(pet.value.sleeping).toBe(true);
    expect(w.findAll(".tam-act")[4].text()).toBe(t("Wake up"));
  });

  it("an egg offers nothing but patience", () => {
    pet.value = createEgg(T0);
    const w = mountPanel();
    expect(w.find(".tam-stats").exists()).toBe(false);
    expect(w.find(".tam-actions").exists()).toBe(false);
    expect(w.find(".tam-caption").text()).toBe(t("An egg. Give it a moment."));
  });

  it("toasts lifecycle events as they arrive", async () => {
    const w = mountPanel();
    petEvents.value = [...petEvents.value, "hatch"];
    await nextTick();
    expect(w.find(".tam-toast").text()).toBe(t("It hatched!"));
  });

  it("a dead pet offers a new egg that restarts the line", async () => {
    const dying = baby(T0, { food: 0, fun: 0, rest: 0, clean: 0, generations: 3, bestAgeMs: 20 * H });
    pet.value = advance(dying, T0 + NEGLECT_DEATH_MS, rng0).state;
    const w = mountPanel();
    expect(w.find(".tam-caption").text()).toBe(t("R.I.P."));
    expect(w.find(".tam-actions").exists()).toBe(false);

    await w.find(".tam-egg-btn").trigger("click");
    expect(w.find(".tam-caption").text()).toBe(t("An egg. Give it a moment."));
    expect(pet.value.generations).toBe(4);
    expect(pet.value.bestAgeMs).toBe(20 * H);
  });

  it("closes on Escape and on a backdrop click", async () => {
    const w = mountPanel();
    await w.find(".veil").trigger("mousedown");
    expect(w.emitted("close")).toHaveLength(1);
    w.unmount();

    const w2 = mountPanel();
    // the key listener sits on window (capture): same dispatch shape as
    // ConfirmDialog's test
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    expect(w2.emitted("close")).toHaveLength(1);
    w2.unmount();
  });
});
