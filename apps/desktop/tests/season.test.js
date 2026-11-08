import { describe, it, expect, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { handle, calls } from "./tauri.js";
import {
  applySeason,
  initSeason,
  normalizeSeason,
  season,
  seasonMode,
  setSeasonMode,
} from "../src/season.js";
import GeneralPage from "../src/components/settings/GeneralPage.vue";
import SeasonBurst from "../src/components/season/SeasonBurst.vue";
import SeasonAmbient from "../src/components/season/SeasonAmbient.vue";

describe("season", () => {
  beforeEach(() => {
    applySeason("off");
    seasonMode.value = "auto";
  });

  it("normalizes unknown values to off", () => {
    for (const s of ["christmas", "halloween", "easter", "valentines"]) expect(normalizeSeason(s)).toBe(s);
    expect(normalizeSeason("ramadan")).toBe("off");
    expect(normalizeSeason(undefined)).toBe("off");
  });

  it("flags <html> only while a season is active", () => {
    applySeason("christmas");
    expect(document.documentElement.dataset.season).toBe("christmas");
    applySeason("easter");
    expect(document.documentElement.dataset.season).toBe("easter");
    applySeason("off");
    expect(document.documentElement.dataset.season).toBeUndefined();
  });

  it("first paint comes from get_season", async () => {
    handle("get_season", () => ({ mode: "halloween", season: "halloween" }));
    await initSeason();
    expect(seasonMode.value).toBe("halloween");
    expect(season.value).toBe("halloween");
  });

  it("a failing backend leaves the plain skin", async () => {
    handle("get_season", () => {
      throw new Error("no backend");
    });
    await initSeason();
    expect(season.value).toBe("off");
  });

  it("setSeasonMode sends the mode and applies the answer", async () => {
    handle("set_season", ({ mode }) => ({ mode, season: mode === "off" ? "off" : "christmas" }));
    await setSeasonMode("off");
    expect(calls.at(-1)).toEqual({ cmd: "set_season", args: { mode: "off" } });
    expect(seasonMode.value).toBe("off");
    expect(season.value).toBe("off");
  });

  it("Settings select saves the choice and reverts on refusal", async () => {
    handle("get_settings", () => ({ hotkey: "Ctrl+Alt+D" }));
    handle("set_season", ({ mode }) => {
      if (mode === "christmas") throw "disk full";
      return { mode, season: mode === "auto" ? "valentines" : "off" };
    });
    const wrapper = mount(GeneralPage);
    await flushPromises();
    const select = wrapper.find('select[aria-label="Seasonal theme"]');
    expect(select.exists()).toBe(true);
    expect(select.findAll("option").map((o) => o.element.value)).toEqual([
      "auto",
      "off",
      "christmas",
      "halloween",
      "easter",
      "valentines",
    ]);
    await select.setValue("christmas");
    await flushPromises();
    expect(select.element.value).toBe("auto");
    expect(wrapper.text()).toContain("disk full");
    await select.setValue("off");
    await flushPromises();
    expect(seasonMode.value).toBe("off");
    await select.setValue("auto");
    await flushPromises();
    expect(wrapper.text()).toContain("On right now: Valentine's Day.");
  });

  it("snow renders the full particle budget", () => {
    const wrapper = mount(SeasonAmbient, { props: { season: "christmas", count: 12 } });
    expect(wrapper.findAll(".particle.snow")).toHaveLength(12);
  });

  it("each season has its own particles, hearts rising", () => {
    const shape = (season) => mount(SeasonAmbient, { props: { season, count: 20 } });
    expect(shape("halloween").findAll(".particle.leaf").length).toBeGreaterThan(0);
    expect(shape("easter").findAll(".particle.petal").length).toBeGreaterThan(0);
    const hearts = shape("valentines").findAll(".particle.heart");
    expect(hearts.length).toBeGreaterThan(0);
    expect(hearts.every((h) => h.classes("rise"))).toBe(true);
  });

  it("a burst appears at the pressed point and caps its count", async () => {
    const wrapper = mount(SeasonBurst);
    for (let i = 0; i < 10; i++) wrapper.vm.burst(100, 50);
    await wrapper.vm.$nextTick();
    const bursts = wrapper.findAll(".burst");
    expect(bursts.length).toBeLessThanOrEqual(6);
    expect(bursts[0].attributes("style")).toContain("left: 100px");
  });

  it("bursts take the season's shape", async () => {
    const wrapper = mount(SeasonBurst, { props: { season: "valentines" } });
    wrapper.vm.burst(10, 10);
    await wrapper.vm.$nextTick();
    expect(wrapper.findAll(".bit.heart")).toHaveLength(9);
  });
});

