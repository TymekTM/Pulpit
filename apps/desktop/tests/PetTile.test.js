import { describe, it, expect, vi, afterEach } from "vitest";
import { mount } from "@vue/test-utils";
import PetTile from "../src/components/tiles/PetTile.vue";
import { api } from "../src/api.js";

vi.mock("../src/api.js", () => ({
  api: { execButtonGesture: vi.fn(() => Promise.resolve()) },
}));

const tile = { id: 7, type: "pet", title: "", w: 2, h: 2 };

const live = {
  species: "cat",
  stage: "adult",
  mood: "happy",
  activity: "idle",
  poos: 0,
  sleeping: false,
  food: 88,
  fun: 72,
  rest: 15,
  clean: 95,
  ageMs: 93_720_000,
  generations: 1,
  label: "Kot · Zadowolone",
};

function mountPet(state, touch = true) {
  return mount(PetTile, { props: { tile, state, touch }, attachTo: document.body });
}

afterEach(() => {
  document.body.innerHTML = "";
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe("PetTile", () => {
  it("renders the egg placeholder before the first push", () => {
    const w = mountPet(null);
    expect(w.find(".tam-sprite").exists()).toBe(true);
    expect(w.classes()).not.toContain("dead");
    // four bars with their safe defaults (the pet has not lived yet)
    expect(w.findAll(".pet-bar").length).toBe(4);
    w.unmount();
  });

  it("mirrors the pushed snapshot: stat bars with a low warning", () => {
    const w = mountPet(live);
    const fills = w.findAll(".pet-bar .fill");
    expect(fills.length).toBe(4);
    // food 88 / fun 72 / rest 15 / clean 95 - only rest dips low
    expect(fills[0].attributes("style")).toContain("88%");
    expect(fills[1].attributes("style")).toContain("72%");
    expect(fills[2].attributes("style")).toContain("15%");
    expect(fills[3].attributes("style")).toContain("95%");
    expect(fills[2].classes()).toContain("low");
    w.unmount();
  });

  it("hides the bars and shows R.I.P. on a dead pet", () => {
    const w = mountPet({ ...live, stage: "dead", mood: "dead" });
    expect(w.classes()).toContain("dead");
    expect(w.find(".pet-rip").text()).toBe("R.I.P.");
    expect(w.find(".pet-bars").exists()).toBe(false);
    w.unmount();
  });

  it("fires tap only after the double-tap window closes", async () => {
    vi.useFakeTimers();
    const w = mountPet(live);
    await w.find(".pet-tile").trigger("pointerdown", { clientY: 10 });
    await w.find(".pet-tile").trigger("pointerup", { clientY: 12 });
    expect(w.emitted("tap")).toBeUndefined();
    await vi.advanceTimersByTime(301);
    expect(w.emitted("tap")).toHaveLength(1);
    // a tap is a feed: it goes through the store, not a gesture
    expect(api.execButtonGesture).not.toHaveBeenCalled();
    w.unmount();
  });

  it("turns a second quick tap into the double-tap gesture", async () => {
    vi.useFakeTimers();
    const w = mountPet(live);
    const face = w.find(".pet-tile");
    await face.trigger("pointerdown", { clientY: 10 });
    await face.trigger("pointerup", { clientY: 12 });
    await face.trigger("pointerdown", { clientY: 10 });
    await face.trigger("pointerup", { clientY: 12 });
    expect(api.execButtonGesture).toHaveBeenCalledWith(7, "double-tap");
    // and no lone tap may fire afterwards
    await vi.advanceTimersByTime(400);
    expect(w.emitted("tap")).toBeUndefined();
    w.unmount();
  });

  it("holds for the long-press gesture", async () => {
    vi.useFakeTimers();
    const w = mountPet(live);
    await w.find(".pet-tile").trigger("pointerdown", { clientY: 10 });
    await vi.advanceTimersByTime(500);
    expect(api.execButtonGesture).toHaveBeenCalledWith(7, "long-press");
    // release after the hold must not also count as a tap
    await w.find(".pet-tile").trigger("pointerup", { clientY: 10 });
    await vi.advanceTimersByTime(400);
    expect(w.emitted("tap")).toBeUndefined();
    w.unmount();
  });

  it("ignores pointer sequences outside touch mode", async () => {
    vi.useFakeTimers();
    const w = mountPet(live, false);
    await w.find(".pet-tile").trigger("pointerdown", { clientY: 10 });
    await vi.advanceTimersByTime(600);
    await w.find(".pet-tile").trigger("pointerup", { clientY: 10 });
    await vi.advanceTimersByTime(400);
    expect(api.execButtonGesture).not.toHaveBeenCalled();
    expect(w.emitted("tap")).toBeUndefined();
    w.unmount();
  });
});
