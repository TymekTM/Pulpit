import { describe, it, expect, beforeEach } from "vitest";
import {
  GROW_MS,
  HATCH_MS,
  MAX_POOS,
  NEGLECT_DEATH_MS,
  OFFLINE_MAX_MS,
  RATES,
  ageOf,
  advance,
  applyAction,
  blockedReason,
  createEgg,
  initPet,
  loadPet,
  moodOf,
  phaseOf,
  pet,
  petDo,
  pooDelay,
  savePet,
  spriteProps,
} from "../src/tamagotchi";

const M = 60_000;
const H = 3_600_000;
const T0 = 1_700_000_000_000;
const rng0 = () => 0; // pooDelay always lands on the minimum

function baby(now = T0, over = {}) {
  // rng0 pins the egg's FIRST poo schedule too (createEgg draws
  // Math.random for both poo timing and species), otherwise the tests
  // race on it
  const s = createEgg(now - HATCH_MS, 1, 0, "blob");
  const base = advance({ ...s, nextPooAt: now + pooDelay(rng0) }, now, rng0).state;
  return { ...base, ...over };
}

beforeEach(() => {
  localStorage.clear();
  pet.value = null;
});

describe("tamagotchi lifecycle", () => {
  it("starts as an egg that hatches after three minutes", () => {
    const egg = createEgg(T0);
    expect(phaseOf(egg, T0)).toBe("egg");
    expect(phaseOf(egg, T0 + HATCH_MS - 45_000 + 1)).toBe("hatching");

    const r = advance(egg, T0 + HATCH_MS, rng0);
    expect(r.events).toEqual(["hatch"]);
    expect(r.state.hatchedAt).toBe(T0 + HATCH_MS);
    expect(phaseOf(r.state, T0 + HATCH_MS)).toBe("baby");
    // the egg needs no care: nothing drained while waiting
    expect(r.state.food).toBe(100);
  });

  it("never mutates the state it advanced", () => {
    const egg = createEgg(T0);
    advance(egg, T0 + 10 * H, rng0);
    expect(egg.lastTick).toBe(T0);
    expect(egg.food).toBe(100);
  });

  it("drains stats at the published hourly rates", () => {
    // no poo inside the hour, so the clean rate is the base one
    const s = baby(T0, { nextPooAt: T0 + 10 * H });
    const r = advance(s, T0 + H, rng0);
    expect(r.state.food).toBeCloseTo(100 - RATES.food, 5);
    expect(r.state.fun).toBeCloseTo(100 - RATES.fun, 5);
    expect(r.state.rest).toBeCloseTo(100 - RATES.rest, 5);
    expect(r.state.clean).toBeCloseTo(100 - RATES.clean, 5);
  });

  it("grows up a day after hatching and says so once", () => {
    let s = baby(T0);
    let now = T0;
    const events = [];
    // a cared-for day: stats topped up each hour so neglect never bites
    for (let i = 0; i < 24; i++) {
      const from = now;
      now += H;
      const r = advance({ ...s, food: 100, fun: 100, rest: 100, clean: 100, lastTick: from }, now, rng0);
      events.push(...r.events);
      s = r.state;
    }
    expect(phaseOf(s, now)).toBe("adult");
    expect(events.filter((e) => e === "grow").length).toBe(1);
    expect(ageOf(s, now)).toBeGreaterThanOrEqual(GROW_MS);
  });
});

describe("tamagotchi sleep", () => {
  it("recovers rest, drains slower, and wakes itself when full", () => {
    // one minute of sleep short of a full battery, so the very first
    // step tops rest to 100 and fires the auto-wake
    const s = baby(T0, { rest: 100 - RATES.restGainAsleep / 60, sleeping: true });
    const r = advance(s, T0 + M, rng0);
    expect(r.events).toContain("wake");
    expect(r.state.sleeping).toBe(false);
    expect(r.state.rest).toBe(100);
    // food drained at the asleep rate for that one minute only
    expect(r.state.food).toBeCloseTo(100 - RATES.foodAsleep / 60, 5);
    expect(r.state.food).toBeGreaterThan(100 - RATES.food / 60);
  });

  it("pauses the clean drain while asleep", () => {
    // rest 0 keeps it asleep for the whole hour (a full recovery needs
    // almost three), so nothing can wake it mid-test
    const s = baby(T0, { sleeping: true, rest: 0, poos: 2 });
    const r = advance(s, T0 + H, rng0);
    expect(r.state.clean).toBe(100);
    expect(r.state.sleeping).toBe(true);
  });
});

describe("tamagotchi poos", () => {
  it("appears on schedule, one at a time, only awake and fed", () => {
    const s = baby(T0, { nextPooAt: T0 + 2 * M });
    const r = advance(s, T0 + 3 * M, rng0);
    expect(r.state.poos).toBe(1);
    expect(r.events).toContain("poo");
    // the poo lands on the first step past the schedule, and the next
    // one is scheduled a full minimum delay after that step
    expect(r.state.nextPooAt).toBe(T0 + 2 * M + pooDelay(rng0));
  });

  it("caps at three and skips a sleeping or starved pet", () => {
    const s = baby(T0, { poos: MAX_POOS, nextPooAt: T0 + M });
    expect(advance(s, T0 + 2 * M, rng0).events).not.toContain("poo");

    const sleeping = baby(T0, { sleeping: true, rest: 0, nextPooAt: T0 + M });
    expect(advance(sleeping, T0 + 2 * M, rng0).state.poos).toBe(0);

    const starved = baby(T0, { food: 10, nextPooAt: T0 + M });
    expect(advance(starved, T0 + 2 * M, rng0).events).not.toContain("poo");
  });

  it("each uncleaned poo speeds up the clean drain", () => {
    // the next natural poo is pushed far away so exactly two poos sit
    // there for the whole hour
    const messy = baby(T0, { poos: 2, nextPooAt: T0 + 10 * H });
    const r = advance(messy, T0 + H, rng0);
    expect(r.state.poos).toBe(2);
    expect(r.state.clean).toBeCloseTo(100 - (RATES.clean + 2 * RATES.cleanPerPoo), 5);
  });
});

describe("tamagotchi neglect and death", () => {
  it("survives eleven hours at rock bottom, dies at twelve", () => {
    const dire = { food: 0, fun: 0, rest: 0, clean: 0 };
    const alive = advance(baby(T0, dire), T0 + 11 * H, rng0);
    expect(alive.state.diedAt).toBeNull();

    const dead = advance(baby(T0, dire), T0 + 12 * H, rng0);
    expect(dead.events).toContain("die");
    expect(dead.state.diedAt).toBe(T0 + 12 * H);
    expect(dead.state.bestAgeMs).toBe(12 * H);
    expect(phaseOf(dead.state, T0 + 12 * H + 1)).toBe("dead");
  });

  it("the dead stay dead: no further change, no mood", () => {
    const s = baby(T0, { food: 0, fun: 0, rest: 0, clean: 0, sleeping: true });
    const dead = advance(s, T0 + NEGLECT_DEATH_MS + 5 * H, rng0).state;
    const frozen = advance(dead, T0 + NEGLECT_DEATH_MS + 20 * H, rng0).state;
    expect(frozen.diedAt).toBe(dead.diedAt);
    expect(moodOf(frozen)).toBe("dead");
    expect(spriteProps(frozen)).toEqual({
      stage: "dead",
      mood: "dead",
      activity: "idle",
      poos: expect.any(Number),
      species: "blob",
    });
  });
});

describe("tamagotchi actions", () => {
  it("feed tops up food and wakes the pet", () => {
    const s = baby(T0, { food: 50, sleeping: true });
    const r = applyAction(s, "feed", T0 + M);
    expect(r.ok).toBe(true);
    expect(r.state.food).toBe(76);
    expect(r.state.sleeping).toBe(false);
  });

  it("feed is refused when the pet is full", () => {
    const s = baby(T0, { food: 95 });
    expect(blockedReason(s, "feed", T0)).toBe("full");
    expect(applyAction(s, "feed", T0).state.food).toBe(95);
  });

  it("play is fun but costs food and rest", () => {
    const s = baby(T0, { fun: 50 });
    const r = applyAction(s, "play", T0);
    expect(r.state.fun).toBe(72);
    expect(r.state.food).toBe(94);
    expect(r.state.rest).toBe(92);
  });

  it("play is refused when asleep, hungry or tired", () => {
    expect(blockedReason(baby(T0, { sleeping: true }), "play", T0)).toBe("asleep");
    expect(blockedReason(baby(T0, { food: 5 }), "play", T0)).toBe("hungry");
    expect(blockedReason(baby(T0, { rest: 5 }), "play", T0)).toBe("tired");
  });

  it("pet has a cooldown", () => {
    const s = baby(T0, { fun: 50 });
    const once = applyAction(s, "pet", T0);
    expect(once.state.fun).toBe(58);
    expect(blockedReason(once.state, "pet", T0 + 1)).toBe("cooldown");
    expect(blockedReason(once.state, "pet", T0 + 30_001)).toBeNull();
  });

  it("clean sweeps the poos and shines", () => {
    const s = baby(T0, { poos: 2, clean: 40, fun: 50 });
    const r = applyAction(s, "clean", T0);
    expect(r.state.poos).toBe(0);
    expect(r.state.clean).toBe(100);
    expect(r.state.fun).toBe(54);
    expect(blockedReason(r.state, "clean", T0)).toBe("clean");
  });

  it("sleep toggles both ways", () => {
    const awake = baby(T0);
    const asleep = applyAction(awake, "sleep", T0).state;
    expect(asleep.sleeping).toBe(true);
    expect(applyAction(asleep, "sleep", T0).state.sleeping).toBe(false);
  });

  it("the egg accepts no care at all", () => {
    const egg = createEgg(T0);
    for (const name of ["feed", "play", "pet", "clean", "sleep"]) {
      expect(blockedReason(egg, name, T0 + M)).toBe("egg");
      expect(applyAction(egg, name, T0 + M).ok).toBe(false);
    }
  });

  it("a new egg carries the generation and the record forward", () => {
    // the pet dies young (12 h), so the 20 h record from its ancestor
    // must survive as the family best
    const dead = baby(T0, { food: 0, fun: 0, rest: 0, clean: 0, generations: 4, bestAgeMs: 20 * H });
    const gone = advance(dead, T0 + NEGLECT_DEATH_MS, rng0).state;
    const r = applyAction(gone, "newEgg", T0 + 20 * H);
    expect(r.ok).toBe(true);
    expect(phaseOf(r.state, T0 + 20 * H + M)).toBe("egg");
    expect(r.state.generations).toBe(5);
    expect(r.state.bestAgeMs).toBe(20 * H);
  });
});

describe("tamagotchi species", () => {
  it("draws the species at egg creation from the rng", () => {
    expect(createEgg(T0, 1, 0, null, () => 0).species).toBe("blob");
    expect(createEgg(T0, 1, 0, null, () => 0.6).species).toBe("chick");
    expect(createEgg(T0, 1, 0, null, () => 0.99).species).toBe("axolotl");
  });

  it("respects an explicit species and normalizes unknown ones", () => {
    expect(createEgg(T0, 1, 0, "frog").species).toBe("frog");
    expect(createEgg(T0, 1, 0, "dragon").species).toBe("blob");
  });

  it("a new egg draws a fresh species through the action rng", () => {
    const dead = baby(T0, { food: 0, fun: 0, rest: 0, clean: 0 });
    const gone = advance(dead, T0 + NEGLECT_DEATH_MS, rng0).state;
    const r = applyAction(gone, "newEgg", T0 + 20 * H, () => 0.9);
    expect(r.state.species).toBe("axolotl"); // floor(0.9 * 5) = 4
  });

  it("spriteProps carries the species for the sprite", () => {
    const cat = baby(T0, { species: "cat" });
    expect(spriteProps(cat, T0).species).toBe("cat");
  });

  it("a save with an unknown species falls back to the blob", () => {
    savePet(baby(T0));
    const raw = JSON.parse(localStorage.getItem("pulpit_tamagotchi_v1"));
    raw.species = "dragon";
    localStorage.setItem("pulpit_tamagotchi_v1", JSON.stringify(raw));
    expect(loadPet().species).toBe("blob");
  });
});

describe("tamagotchi moods", () => {
  it("prioritises the loudest need", () => {
    const at = (over) => moodOf(baby(T0, over));
    expect(at({})).toBe("happy");
    expect(at({ food: 100, fun: 100, rest: 100, clean: 15 })).toBe("dirty");
    expect(at({ poos: MAX_POOS })).toBe("dirty");
    expect(at({ food: 10 })).toBe("hungry");
    expect(at({ rest: 10 })).toBe("sleepy");
    expect(at({ fun: 10 })).toBe("sad");
    expect(at({ sleeping: true })).toBe("sleepy");
    expect(at({ food: 40, fun: 40, rest: 40, clean: 40 })).toBe("neutral");
  });
});

describe("tamagotchi persistence", () => {
  it("round-trips through localStorage", () => {
    const s = baby(T0, { poos: 2, food: 33.5 });
    savePet(s);
    expect(loadPet()).toEqual(s);
  });

  it("rejects broken saves and clamps wild values", () => {
    localStorage.setItem("pulpit_tamagotchi_v1", "{not json");
    expect(loadPet()).toBeNull();

    savePet(baby(T0));
    const raw = JSON.parse(localStorage.getItem("pulpit_tamagotchi_v1"));
    raw.food = 900;
    raw.poos = 9;
    raw.sleeping = true;
    raw.diedAt = T0;
    raw.hatchedAt = null;
    localStorage.setItem("pulpit_tamagotchi_v1", JSON.stringify(raw));
    const s = loadPet();
    expect(s.food).toBe(100);
    expect(s.poos).toBe(MAX_POOS);
    expect(s.sleeping).toBe(false); // the dead do not snore
    expect(phaseOf(s, T0)).toBe("dead");
  });

  it("lays a first egg when the store is empty", () => {
    const r = initPet(T0);
    expect(phaseOf(r.state, T0)).toBe("egg");
    expect(r.state.generations).toBe(1);
  });

  it("a short absence is simulated normally", () => {
    savePet(baby(T0));
    const r = initPet(T0 + 2 * H);
    expect(r.events).not.toContain("missedYou");
    expect(r.state.food).toBeCloseTo(100 - 2 * RATES.food, 5);
  });

  it("a long absence drains at most OFFLINE_MAX_MS and never kills", () => {
    savePet(baby(T0));
    const r = initPet(T0 + 3 * 24 * H);
    expect(r.events).toContain("missedYou");
    expect(r.state.diedAt).toBeNull();
    expect(r.state.food).toBe(0);
    expect(r.state.lastTick).toBe(T0 + 3 * 24 * H);
    // the drain is capped at the mercy limit, not three days
    expect(r.state.neglectMs).toBe(0);
  });
});

describe("tamagotchi reactive shell", () => {
  it("petDo applies to the shared pet and reports refusals", () => {
    pet.value = baby(T0, { food: 50 });
    expect(petDo("feed", T0).ok).toBe(true); // 50 -> 76
    expect(petDo("feed", T0).ok).toBe(true); // 76 -> 100
    expect(petDo("feed", T0).ok).toBe(false); // full now
    expect(pet.value.food).toBe(100);
    expect(pet.value.generations).toBe(1);
  });

  it("ageOf counts from the hatch, not from the egg", () => {
    expect(ageOf(createEgg(T0), T0 + H)).toBe(0);
    expect(ageOf(baby(T0), T0 + 90 * M)).toBe(90 * M);
  });
});
