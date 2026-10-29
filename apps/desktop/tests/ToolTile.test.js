import { describe, it, expect, vi, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import ToolTile from "../src/components/tiles/ToolTile.vue";
import { windowVisible } from "../src/statusMedia.js";

const tile = (type) => ({ id: 1, type, title: "", w: 1, h: 1 });

function mountTool(type, state, cmd = {}) {
  return mount(ToolTile, { props: { tile: tile(type), state, cmd }, attachTo: document.body });
}

afterEach(() => {
  windowVisible.value = true;
  document.body.innerHTML = "";
  vi.useRealTimers();
});

describe("ToolTile ticking", () => {
  it("counts a running stopwatch up once a second, not 5x a second", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(100_000);
    const w = mountTool("tool-stopwatch", { running: true, startedAtMs: 100_000, elapsedMs: 0 });
    await flushPromises();
    expect(w.find(".tool-value").text()).toBe("00:00");
    await vi.advanceTimersByTimeAsync(3050);
    expect(w.find(".tool-value").text()).toBe("00:04");
    const renders = [];
    for (let i = 0; i < 5; i++) {
      await vi.advanceTimersByTimeAsync(200);
      renders.push(w.find(".tool-value").text());
    }
    // at most two value changes across one second of 200 ms steps
    expect(new Set(renders).size).toBeLessThanOrEqual(2);
    w.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("counts a running timer down and stops ticking at zero", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(100_000);
    const w = mountTool("tool-timer", {
      running: true,
      startedAtMs: 100_000,
      elapsedMs: 0,
      durationMs: 3000,
    });
    await flushPromises();
    expect(w.find(".tool-value").text()).toBe("00:03");
    await vi.advanceTimersByTimeAsync(1100);
    expect(w.find(".tool-value").text()).toBe("00:02");
    await vi.advanceTimersByTimeAsync(2500);
    expect(w.find(".tool-value").text()).toBe("00:00");
    expect(w.find(".tool-body").classes()).toContain("flashing");
    expect(vi.getTimerCount()).toBe(0);
    w.unmount();
  });

  it("starts no ticker for a paused stopwatch", async () => {
    vi.useFakeTimers();
    const w = mountTool("tool-stopwatch", { running: false, elapsedMs: 5000 });
    await flushPromises();
    expect(vi.getTimerCount()).toBe(0);
    w.unmount();
  });

  it("does not tick while the window is hidden and resumes on show", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(100_000);
    windowVisible.value = false;
    const w = mountTool("tool-stopwatch", { running: true, startedAtMs: 100_000, elapsedMs: 0 });
    await flushPromises();
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(5000);
    expect(w.find(".tool-value").text()).toBe("00:00");

    windowVisible.value = true;
    await flushPromises();
    expect(vi.getTimerCount()).toBe(1);
    expect(w.find(".tool-value").text()).toBe("00:05");

    // hiding again tears the chain down
    windowVisible.value = false;
    await flushPromises();
    expect(vi.getTimerCount()).toBe(0);
    w.unmount();
  });

  it("ticks the clock with a cached formatter and stops on unmount", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(Date.UTC(2026, 0, 1, 12, 0, 0));
    const w = mountTool("tool-clock", null, { seconds: "yes", timezone: "UTC", format: "24h" });
    await flushPromises();
    const first = w.find(".tool-time").text();
    await vi.advanceTimersByTimeAsync(2100);
    expect(w.find(".tool-time").text()).not.toBe(first);
    w.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
