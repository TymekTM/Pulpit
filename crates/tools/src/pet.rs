//! The desk pet: a small tamagotchi that lives on the server. The
//! simulation runs here, so the pet keeps living (and answers tablet
//! care gestures) with the editor window closed or with only the
//! headless server running. Every surface (desktop PetTile.vue, the
//! Android client, stock Deckboard tablets) only renders the compact
//! snapshot pushed on the `pet` channel and sends care gestures back.
//!
//! Every rule is a pure function of (state, now) with an injected rng,
//! so tests pin the clock and never wait. The state advances in fixed
//! one-minute steps; time the process did not run (Pulpit closed, the
//! PC asleep) is simulated in "offline" mode, capped at
//! [`OFFLINE_MAX_MS`] and without the neglect counter - closing the app
//! drains the pet but never kills it, only ignoring a running one does.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

const M: i64 = 60_000;
const H: i64 = 3_600_000;

/// One simulation step. Stats drain linearly, so a coarse step only
/// blurs event timing by up to a minute - invisible at pet pace.
const STEP_MS: i64 = M;

/// Points lost (or gained) per hour.
pub const RATE_FOOD: f64 = 22.0; // full bowl to empty in ~4.5 h
pub const RATE_FUN: f64 = 16.0;
pub const RATE_REST: f64 = 10.0; // awake for ~10 h before it must sleep
pub const RATE_CLEAN: f64 = 5.0;
pub const RATE_CLEAN_PER_POO: f64 = 7.0; // each uncleaned poo adds this much
pub const RATE_REST_GAIN_ASLEEP: f64 = 35.0; // empty to full in ~3 h
pub const RATE_FOOD_ASLEEP: f64 = RATE_FOOD * 0.4; // sleep slows the drain
pub const RATE_FUN_ASLEEP: f64 = RATE_FUN * 0.4;

pub const HATCH_MS: i64 = 3 * M; // egg -> baby
pub const HATCHING_MS: i64 = 45_000; // the cracking finale of the egg
pub const GROW_MS: i64 = 24 * H; // baby -> adult
/// Cumulative time with any stat at rock bottom kills the pet.
pub const NEGLECT_DEATH_MS: i64 = 12 * H;
/// The most of an absence that is simulated (offline, neglect paused).
pub const OFFLINE_MAX_MS: i64 = 18 * H;
/// A tick gap longer than this means the process did not run (system
/// sleep, a stopped debugger): simulated as an absence, not as neglect.
const ABSENCE_GAP_MS: i64 = 5 * M;

const POO_MIN_MS: f64 = 45.0 * M as f64;
const POO_MAX_MS: f64 = 90.0 * M as f64;
pub const MAX_POOS: u32 = 3;

/// The routine stat drain reaches clients on this budget; events and
/// visible changes (stage, mood, poos, sleep) push at once.
const PUSH_EVERY_MS: i64 = 15_000;

/// Species the sprites draw. Cosmetic only - the rules do not care.
/// "blob" first: it is the fallback.
pub const SPECIES: [&str; 5] = ["blob", "cat", "frog", "chick", "axolotl"];

pub fn normalize_species(v: &str) -> &'static str {
    SPECIES.iter().copied().find(|s| *s == v).unwrap_or("blob")
}

/// The pet tile kind (the tile carries no per-id state: the pet is
/// global, so every pet tile on any board renders the same animal).
pub fn is_pet_action(kind: &str) -> bool {
    kind == "pet"
}

/// The full simulation state, persisted to `~/pulpitApp/pet.json`. Same
/// field names as the pre-1.11.1 webview save, so a desktop pet kept in
/// localStorage adopts as-is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pet {
    pub v: u32,
    pub created: i64,
    pub last_tick: i64,
    pub hatched_at: Option<i64>,
    pub died_at: Option<i64>,
    pub species: String,
    pub food: f64,
    pub fun: f64,
    pub rest: f64,
    pub clean: f64,
    pub poos: u32,
    pub sleeping: bool,
    pub next_poo_at: i64,
    pub neglect_ms: i64,
    pub generations: u32,
    pub best_age_ms: i64,
    pub cares: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetEvent {
    Hatch,
    Grow,
    Poo,
    Sleep,
    Wake,
    Die,
}

/// A care action. Tablets and touch mode reach them as gestures: tap =
/// feed, double-tap = play, long-press = clean (a new egg on a dead pet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetAction {
    Feed,
    Play,
    Clean,
    NewEgg,
}

fn poo_delay(rng: &mut dyn FnMut() -> f64) -> i64 {
    (POO_MIN_MS + rng() * (POO_MAX_MS - POO_MIN_MS)) as i64
}

impl Pet {
    /// A fresh egg. `generations` and `best_age_ms` survive the previous
    /// pet; the species is drawn from `rng` unless given.
    pub fn egg(
        now: i64,
        generations: u32,
        best_age_ms: i64,
        species: Option<&str>,
        rng: &mut dyn FnMut() -> f64,
    ) -> Self {
        let species = match species {
            Some(s) => normalize_species(s),
            None => SPECIES[((rng() * SPECIES.len() as f64) as usize).min(SPECIES.len() - 1)],
        };
        Pet {
            v: 1,
            created: now,
            last_tick: now,
            hatched_at: None,
            died_at: None,
            species: species.to_string(),
            food: 100.0,
            fun: 100.0,
            rest: 100.0,
            clean: 100.0,
            poos: 0,
            sleeping: false,
            next_poo_at: now + poo_delay(rng),
            neglect_ms: 0,
            generations,
            best_age_ms,
            cares: 0,
        }
    }

    /// The visible life phase, straight from the timestamps.
    pub fn phase(&self, now: i64) -> &'static str {
        if self.died_at.is_some() {
            return "dead";
        }
        match self.hatched_at {
            None if now >= self.created + HATCH_MS - HATCHING_MS => "hatching",
            None => "egg",
            Some(h) if now >= h + GROW_MS => "adult",
            Some(_) => "baby",
        }
    }

    /// Which face the sprite pulls. Priority: death, sleep, then the
    /// loudest need below 20 points; otherwise mood follows the average.
    pub fn mood(&self) -> &'static str {
        if self.died_at.is_some() {
            return "dead";
        }
        if self.sleeping {
            return "sleepy";
        }
        if self.clean < 20.0 || self.poos >= MAX_POOS {
            return "dirty";
        }
        if self.food < 20.0 {
            return "hungry";
        }
        if self.rest < 20.0 {
            return "sleepy";
        }
        if self.fun < 20.0 {
            return "sad";
        }
        let avg = (self.food + self.fun + self.rest + self.clean) / 4.0;
        if avg < 45.0 {
            "neutral"
        } else {
            "happy"
        }
    }

    /// Lived time: from the hatch to death, or to `now` while alive.
    pub fn age(&self, now: i64) -> i64 {
        match self.hatched_at {
            None => 0,
            Some(h) => (self.died_at.unwrap_or(now) - h).max(0),
        }
    }

    /// Run the simulation from `last_tick` to `now`, returning the
    /// events crossed, oldest first. `offline` stops the neglect counter:
    /// draining a closed app's pet is boarding at a farm, never dying.
    pub fn advance(
        &mut self,
        now: i64,
        rng: &mut dyn FnMut() -> f64,
        offline: bool,
    ) -> Vec<PetEvent> {
        let mut events = Vec::new();
        let mut t = self.last_tick;
        let mut was_baby = self.hatched_at.is_some() && self.phase(t) == "baby";

        while t < now && self.died_at.is_none() {
            let dt = STEP_MS.min(now - t);
            t += dt;
            let hours = dt as f64 / H as f64;

            let Some(hatched_at) = self.hatched_at else {
                // the egg needs no care, only time
                if t >= self.created + HATCH_MS {
                    self.hatched_at = Some(self.created + HATCH_MS);
                    was_baby = true;
                    events.push(PetEvent::Hatch);
                }
                continue;
            };

            if self.sleeping {
                self.rest = (self.rest + RATE_REST_GAIN_ASLEEP * hours).clamp(0.0, 100.0);
                self.food = (self.food - RATE_FOOD_ASLEEP * hours).max(0.0);
                self.fun = (self.fun - RATE_FUN_ASLEEP * hours).max(0.0);
                if self.rest >= 100.0 {
                    self.sleeping = false;
                    events.push(PetEvent::Wake);
                }
            } else {
                self.food = (self.food - RATE_FOOD * hours).max(0.0);
                self.fun = (self.fun - RATE_FUN * hours).max(0.0);
                self.rest = (self.rest - RATE_REST * hours).max(0.0);
                let clean_rate = RATE_CLEAN + self.poos as f64 * RATE_CLEAN_PER_POO;
                self.clean = (self.clean - clean_rate * hours).max(0.0);
                // sleep is automatic: a pet runs itself into the ground,
                // then naps until fully rested
                if self.rest <= 5.0 {
                    self.sleeping = true;
                    events.push(PetEvent::Sleep);
                }
            }

            if t >= self.next_poo_at {
                if !self.sleeping && self.food > 15.0 && self.poos < MAX_POOS {
                    self.poos += 1;
                    events.push(PetEvent::Poo);
                }
                self.next_poo_at = t + poo_delay(rng);
            }

            if was_baby && self.phase(t) == "adult" {
                was_baby = false;
                events.push(PetEvent::Grow);
            }

            let bottomed =
                self.food <= 0.0 || self.fun <= 0.0 || self.rest <= 0.0 || self.clean <= 0.0;
            if !offline && bottomed {
                self.neglect_ms += dt;
                if self.neglect_ms >= NEGLECT_DEATH_MS {
                    self.neglect_ms = NEGLECT_DEATH_MS;
                    self.died_at = Some(t);
                    self.sleeping = false;
                    self.best_age_ms = self.best_age_ms.max(t - hatched_at);
                    events.push(PetEvent::Die);
                }
            }
        }
        self.last_tick = now;
        events
    }

    /// Catch up on an absence (the process did not run): the first
    /// [`OFFLINE_MAX_MS`] of it is simulated with neglect paused, the rest
    /// is skipped.
    pub fn catch_up_absence(&mut self, now: i64, rng: &mut dyn FnMut() -> f64) -> Vec<PetEvent> {
        let until = now.min(self.last_tick + OFFLINE_MAX_MS);
        let events = self.advance(until, rng, true);
        self.last_tick = now.max(self.last_tick);
        events
    }

    /// Why an action is unavailable, or `None` when it is not.
    pub fn blocked(&self, action: PetAction, now: i64) -> Option<&'static str> {
        if action == PetAction::NewEgg {
            return if self.died_at.is_some() {
                None
            } else {
                Some("alive")
            };
        }
        match self.phase(now) {
            "dead" => return Some("dead"),
            "egg" | "hatching" => return Some("egg"),
            _ => {}
        }
        match action {
            PetAction::Feed if self.food >= 92.0 => Some("full"),
            PetAction::Play if self.sleeping => Some("asleep"),
            PetAction::Play if self.food <= 8.0 => Some("hungry"),
            PetAction::Play if self.rest <= 8.0 => Some("tired"),
            PetAction::Clean if self.clean >= 100.0 && self.poos == 0 => Some("clean"),
            _ => None,
        }
    }

    /// Apply one care action, or say why it was refused.
    pub fn apply(
        &mut self,
        action: PetAction,
        now: i64,
        rng: &mut dyn FnMut() -> f64,
    ) -> Result<(), &'static str> {
        if let Some(reason) = self.blocked(action, now) {
            return Err(reason);
        }
        match action {
            PetAction::NewEgg => {
                // the next generation hatches a freshly drawn species
                *self = Pet::egg(now, self.generations + 1, self.best_age_ms, None, rng);
            }
            PetAction::Feed => {
                self.food = (self.food + 26.0).min(100.0);
                self.sleeping = false; // dinner wakes
                self.cares += 1;
            }
            PetAction::Play => {
                self.fun = (self.fun + 22.0).min(100.0);
                self.food = (self.food - 6.0).max(0.0);
                self.rest = (self.rest - 8.0).max(0.0);
                self.cares += 1;
            }
            PetAction::Clean => {
                self.poos = 0;
                self.clean = 100.0;
                self.fun = (self.fun + 4.0).min(100.0);
                self.cares += 1;
            }
        }
        Ok(())
    }

    /// The compact object pushed on the `pet` channel: the derived
    /// fields every surface renders without reimplementing the rules,
    /// plus a display `label` the legacy lane shows as plain text.
    pub fn snapshot(&self, now: i64) -> Value {
        let stage = self.phase(now);
        let mood = self.mood();
        let species = normalize_species(&self.species);
        let mood_label = if stage == "dead" {
            "R.I.P."
        } else {
            mood_label(mood)
        };
        json!({
            "species": species,
            "stage": stage,
            "mood": mood,
            "activity": if self.sleeping { "sleep" } else { "idle" },
            "poos": self.poos,
            "sleeping": self.sleeping,
            "food": self.food.round() as i64,
            "fun": self.fun.round() as i64,
            "rest": self.rest.round() as i64,
            "clean": self.clean.round() as i64,
            "ageMs": self.age(now),
            "generations": self.generations,
            "label": format!("{} · {}", species_label(species), mood_label),
        })
    }

    /// A saved full state (pet.json, or the old webview localStorage
    /// save), sanitized: wild values clamped, broken saves rejected.
    pub fn from_saved(v: &Value) -> Option<Self> {
        let num = |key: &str| v.get(key).and_then(Value::as_f64).filter(|n| n.is_finite());
        let created = num("created")? as i64;
        let last_tick = num("lastTick")? as i64;
        let died_at = num("diedAt").map(|n| n as i64);
        let stat = |key: &str| num(key).unwrap_or(0.0).clamp(0.0, 100.0);
        Some(Pet {
            v: 1,
            created,
            last_tick,
            hatched_at: num("hatchedAt").map(|n| n as i64),
            died_at,
            species: normalize_species(v.get("species").and_then(Value::as_str).unwrap_or(""))
                .to_string(),
            food: stat("food"),
            fun: stat("fun"),
            rest: stat("rest"),
            clean: stat("clean"),
            poos: (num("poos").unwrap_or(0.0).floor().max(0.0) as u32).min(MAX_POOS),
            sleeping: v.get("sleeping").and_then(Value::as_bool).unwrap_or(false)
                && died_at.is_none(),
            next_poo_at: num("nextPooAt")
                .map(|n| n as i64)
                .unwrap_or(last_tick + POO_MIN_MS as i64),
            neglect_ms: num("neglectMs").unwrap_or(0.0).max(0.0) as i64,
            generations: num("generations").unwrap_or(1.0).floor().max(1.0) as u32,
            best_age_ms: num("bestAgeMs").unwrap_or(0.0).max(0.0) as i64,
            cares: num("cares").unwrap_or(0.0).floor().max(0.0) as u32,
        })
    }

    /// Rebuild a pet from a 1.11.0 `pet.json`, which held only the
    /// render snapshot (species, stage, stats, age): close enough that
    /// the animal, its age and its generation survive the upgrade.
    fn from_snapshot(v: &Value, now: i64, rng: &mut dyn FnMut() -> f64) -> Option<Self> {
        let species = v.get("species").and_then(Value::as_str)?;
        let stage = v.get("stage").and_then(Value::as_str)?;
        let num = |key: &str| v.get(key).and_then(Value::as_f64).filter(|n| n.is_finite());
        let generations = num("generations").unwrap_or(1.0).floor().max(1.0) as u32;
        let mut pet = Pet::egg(now, generations, 0, Some(species), rng);
        if stage == "egg" || stage == "hatching" {
            return Some(pet);
        }
        let age = num("ageMs").unwrap_or(0.0).max(0.0) as i64;
        let hatched_at = now - age;
        pet.created = hatched_at - HATCH_MS;
        pet.hatched_at = Some(hatched_at);
        if stage == "dead" {
            pet.died_at = Some(now);
            pet.best_age_ms = age;
            return Some(pet);
        }
        let stat = |key: &str| num(key).unwrap_or(100.0).clamp(0.0, 100.0);
        pet.food = stat("food");
        pet.fun = stat("fun");
        pet.rest = stat("rest");
        pet.clean = stat("clean");
        pet.poos = (num("poos").unwrap_or(0.0).max(0.0) as u32).min(MAX_POOS);
        pet.sleeping = v.get("sleeping").and_then(Value::as_bool).unwrap_or(false);
        Some(pet)
    }
}

fn mood_label(mood: &str) -> &'static str {
    use pulpit_i18n::tr;
    match mood {
        "happy" => tr("Beaming", "Promienne"),
        "sad" => tr("Bored", "Znudzone"),
        "hungry" => tr("Hungry", "Głodne"),
        "sleepy" => tr("Sleepy", "Śpiące"),
        "dirty" => tr("Needs a bath", "Wymaga kąpieli"),
        "dead" => "R.I.P.",
        _ => tr("Content", "Zadowolone"),
    }
}

fn species_label(species: &str) -> &'static str {
    use pulpit_i18n::tr;
    match species {
        "cat" => tr("Cat", "Kot"),
        "frog" => tr("Frog", "Żaba"),
        "chick" => tr("Chick", "Kurczak"),
        "axolotl" => tr("Axolotl", "Aksolotl"),
        _ => tr("Blob", "Kleks"),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn random() -> f64 {
    rand::random::<f64>()
}

struct Inner {
    pet: Pet,
    /// Laid fresh by this process (no pet.json at startup) and not
    /// cared for yet: a pre-1.11.1 desktop pet from the webview's
    /// localStorage may still move in.
    adoptable: bool,
    last_push: i64,
    /// (stage, mood, poos, sleeping) of the last push: a visible change
    /// pushes at once instead of waiting for the routine budget.
    last_look: (String, String, u32, bool),
}

/// The shared pet: simulation, persistence and relay.
pub struct PetManager {
    store_path: PathBuf,
    inner: Mutex<Inner>,
    change_tx: UnboundedSender<Value>,
}

impl PetManager {
    /// Load `pet.json` (or lay a first egg) and catch up on the time
    /// the process was not running. Pushes nothing yet: [`spawn_pet`]
    /// relays the first snapshot once the ticker is up.
    pub fn new(store_path: PathBuf, change_tx: UnboundedSender<Value>, now: i64) -> Self {
        Self::with_rng(store_path, change_tx, now, &mut random)
    }

    fn with_rng(
        store_path: PathBuf,
        change_tx: UnboundedSender<Value>,
        now: i64,
        rng: &mut dyn FnMut() -> f64,
    ) -> Self {
        let saved: Option<Value> = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok());
        let restored = saved
            .as_ref()
            .and_then(|v| Pet::from_saved(v).or_else(|| Pet::from_snapshot(v, now, rng)));
        let adoptable = restored.is_none();
        let mut pet = restored.unwrap_or_else(|| Pet::egg(now, 1, 0, None, rng));
        if now - pet.last_tick > ABSENCE_GAP_MS {
            pet.catch_up_absence(now, rng);
        } else {
            pet.advance(now, rng, false);
        }
        let mgr = Self {
            store_path,
            inner: Mutex::new(Inner {
                pet,
                adoptable,
                last_push: i64::MIN,
                last_look: Default::default(),
            }),
            change_tx,
        };
        // the first egg, an upgraded or a caught-up state lands on disk
        // right away
        mgr.persist(&mgr.inner.lock().unwrap().pet);
        mgr
    }

    /// The current snapshot, for a surface that just connected.
    pub fn get(&self) -> Option<Value> {
        Some(self.inner.lock().unwrap().pet.snapshot(now_ms()))
    }

    /// The full state (tests, diagnostics).
    pub fn pet(&self) -> Pet {
        self.inner.lock().unwrap().pet.clone()
    }

    /// A care gesture from any surface: tap = feed, double-tap = play,
    /// long-press = clean, or a new egg on a dead pet.
    pub fn gesture(&self, gesture: &str) {
        self.gesture_at(gesture, now_ms(), &mut random);
    }

    fn gesture_at(&self, gesture: &str, now: i64, rng: &mut dyn FnMut() -> f64) {
        let mut inner = self.inner.lock().unwrap();
        let action = match gesture {
            "tap" => PetAction::Feed,
            "double-tap" => PetAction::Play,
            "long-press" if inner.pet.died_at.is_some() => PetAction::NewEgg,
            "long-press" => PetAction::Clean,
            _ => return,
        };
        // bring the clock current first so the action sees live stats
        self.advance_locked(&mut inner, now, rng);
        if inner.pet.apply(action, now, rng).is_ok() {
            inner.adoptable = false;
            self.push_locked(&mut inner, now);
        }
    }

    /// Move a pre-1.11.1 desktop pet (the webview's localStorage save)
    /// in. Only while the server pet is a fresh egg nobody has cared for;
    /// returns whether it was taken.
    pub fn adopt(&self, saved: &Value) -> bool {
        self.adopt_at(saved, now_ms(), &mut random)
    }

    fn adopt_at(&self, saved: &Value, now: i64, rng: &mut dyn FnMut() -> f64) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if !inner.adoptable {
            return false;
        }
        let Some(mut pet) = Pet::from_saved(saved) else {
            return false;
        };
        pet.catch_up_absence(now, rng);
        inner.pet = pet;
        inner.adoptable = false;
        self.push_locked(&mut inner, now);
        true
    }

    /// One simulation tick: advance to `now`, push on events, visible
    /// changes, or the routine budget.
    pub fn tick(&self) {
        self.tick_at(now_ms(), &mut random);
    }

    fn tick_at(&self, now: i64, rng: &mut dyn FnMut() -> f64) {
        let mut inner = self.inner.lock().unwrap();
        let events = self.advance_locked(&mut inner, now, rng);
        let look = look_of(&inner.pet, now);
        if !events.is_empty()
            || look != inner.last_look
            || now.saturating_sub(inner.last_push) >= PUSH_EVERY_MS
        {
            self.push_locked(&mut inner, now);
        }
    }

    fn advance_locked(
        &self,
        inner: &mut Inner,
        now: i64,
        rng: &mut dyn FnMut() -> f64,
    ) -> Vec<PetEvent> {
        if now - inner.pet.last_tick > ABSENCE_GAP_MS {
            // the process did not run (system sleep): mercy, not neglect
            inner.pet.catch_up_absence(now, rng)
        } else {
            inner.pet.advance(now, rng, false)
        }
    }

    fn push_locked(&self, inner: &mut Inner, now: i64) {
        inner.last_push = now;
        inner.last_look = look_of(&inner.pet, now);
        self.persist(&inner.pet);
        let _ = self
            .change_tx
            .send(json!({ "pet": inner.pet.snapshot(now) }));
    }

    fn persist(&self, pet: &Pet) {
        if let Some(parent) = self.store_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_vec_pretty(pet) {
            Ok(bytes) => {
                if let Err(e) = pulpit_db::write_atomic(&self.store_path, &bytes) {
                    tracing::warn!("pet.json write failed: {e}");
                }
            }
            Err(e) => tracing::warn!("pet state serialization failed: {e}"),
        }
    }
}

fn look_of(pet: &Pet, now: i64) -> (String, String, u32, bool) {
    (
        pet.phase(now).to_string(),
        pet.mood().to_string(),
        pet.poos,
        pet.sleeping,
    )
}

pub type SharedPetManager = Arc<PetManager>;

/// Start the pet: load it, push the first snapshot and tick it once a
/// second for as long as the runtime lives. Returns the manager plus the
/// snapshot stream for `pulpit_host::forward_producer`. Must run inside
/// a tokio runtime.
pub fn spawn_pet(store_path: PathBuf) -> (Arc<PetManager>, UnboundedReceiver<Value>) {
    let (tx, rx) = unbounded_channel();
    let manager = Arc::new(PetManager::new(store_path, tx, now_ms()));
    let ticker = manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            ticker.tick();
        }
    });
    (manager, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_700_000_000_000;

    fn rng0() -> impl FnMut() -> f64 {
        || 0.0
    }

    /// A freshly hatched blob at `T0`, next poo pinned to the minimum.
    fn baby() -> Pet {
        let mut r = rng0();
        let mut p = Pet::egg(T0 - HATCH_MS, 1, 0, Some("blob"), &mut r);
        p.advance(T0, &mut r, false);
        p.next_poo_at = T0 + POO_MIN_MS as i64;
        p
    }

    fn adv(p: &mut Pet, to: i64) -> Vec<PetEvent> {
        p.advance(to, &mut rng0(), false)
    }

    #[test]
    fn an_egg_hatches_after_three_minutes_without_care() {
        let mut egg = Pet::egg(T0, 1, 0, None, &mut rng0());
        assert_eq!(egg.phase(T0), "egg");
        assert_eq!(egg.phase(T0 + HATCH_MS - HATCHING_MS + 1), "hatching");
        assert_eq!(adv(&mut egg, T0 + HATCH_MS), vec![PetEvent::Hatch]);
        assert_eq!(egg.hatched_at, Some(T0 + HATCH_MS));
        assert_eq!(egg.phase(T0 + HATCH_MS), "baby");
        assert_eq!(egg.food, 100.0);
    }

    #[test]
    fn stats_drain_at_the_hourly_rates() {
        let mut p = baby();
        p.next_poo_at = T0 + 10 * H;
        adv(&mut p, T0 + H);
        assert!((p.food - (100.0 - RATE_FOOD)).abs() < 1e-6);
        assert!((p.fun - (100.0 - RATE_FUN)).abs() < 1e-6);
        assert!((p.rest - (100.0 - RATE_REST)).abs() < 1e-6);
        assert!((p.clean - (100.0 - RATE_CLEAN)).abs() < 1e-6);
    }

    #[test]
    fn grows_up_a_day_after_hatching_once() {
        let mut p = baby();
        let mut events = Vec::new();
        for i in 1..=24 {
            p.food = 100.0;
            p.fun = 100.0;
            p.rest = 100.0;
            p.clean = 100.0;
            events.extend(adv(&mut p, T0 + i * H));
        }
        assert_eq!(p.phase(T0 + 24 * H), "adult");
        assert_eq!(events.iter().filter(|e| **e == PetEvent::Grow).count(), 1);
    }

    #[test]
    fn sleeps_when_rest_runs_dry_and_wakes_rested() {
        let mut p = baby();
        p.rest = 5.1;
        assert!(adv(&mut p, T0 + M).contains(&PetEvent::Sleep));
        assert!(p.sleeping);
        assert!(adv(&mut p, T0 + 3 * H).contains(&PetEvent::Wake));
        assert!(!p.sleeping);
        assert!(p.rest > 95.0);
    }

    #[test]
    fn sleep_pauses_the_clean_drain_and_poos() {
        let mut p = baby();
        p.sleeping = true;
        p.rest = 0.0;
        p.poos = 2;
        p.next_poo_at = T0 + M;
        adv(&mut p, T0 + H);
        assert_eq!(p.clean, 100.0);
        assert_eq!(p.poos, 2);
        assert!(p.sleeping);
    }

    #[test]
    fn poos_land_on_schedule_and_cap_at_three() {
        let mut p = baby();
        p.next_poo_at = T0 + 2 * M;
        assert!(adv(&mut p, T0 + 3 * M).contains(&PetEvent::Poo));
        assert_eq!(p.poos, 1);
        assert_eq!(p.next_poo_at, T0 + 2 * M + POO_MIN_MS as i64);

        let mut full = baby();
        full.poos = MAX_POOS;
        full.next_poo_at = T0 + M;
        assert!(!adv(&mut full, T0 + 2 * M).contains(&PetEvent::Poo));

        let mut starved = baby();
        starved.food = 10.0;
        starved.next_poo_at = T0 + M;
        assert!(!adv(&mut starved, T0 + 2 * M).contains(&PetEvent::Poo));
    }

    #[test]
    fn each_poo_speeds_up_the_clean_drain() {
        let mut p = baby();
        p.poos = 2;
        p.next_poo_at = T0 + 10 * H;
        adv(&mut p, T0 + H);
        let expected = 100.0 - (RATE_CLEAN + 2.0 * RATE_CLEAN_PER_POO);
        assert!((p.clean - expected).abs() < 1e-6);
    }

    #[test]
    fn twelve_hours_at_rock_bottom_kill_eleven_do_not() {
        let dire = |mut p: Pet| {
            p.food = 0.0;
            p.fun = 0.0;
            p.rest = 0.0;
            p.clean = 0.0;
            p
        };
        let mut alive = dire(baby());
        adv(&mut alive, T0 + 11 * H);
        assert_eq!(alive.died_at, None);

        let mut dead = dire(baby());
        assert!(adv(&mut dead, T0 + 12 * H).contains(&PetEvent::Die));
        assert_eq!(dead.died_at, Some(T0 + 12 * H));
        assert_eq!(dead.best_age_ms, 12 * H);
        assert_eq!(dead.mood(), "dead");

        // the dead stay dead
        let frozen = dead.clone();
        adv(&mut dead, T0 + 30 * H);
        assert_eq!(dead.died_at, frozen.died_at);
        assert_eq!(dead.food, frozen.food);
    }

    #[test]
    fn an_absence_drains_but_never_kills() {
        let mut p = baby();
        p.catch_up_absence(T0 + 7 * 24 * H, &mut rng0());
        assert_eq!(p.died_at, None);
        assert_eq!(p.neglect_ms, 0);
        assert_eq!(p.food, 0.0);
        assert_eq!(p.last_tick, T0 + 7 * 24 * H);
    }

    #[test]
    fn care_actions_follow_the_rules() {
        let mut r = rng0();
        let mut p = baby();
        p.food = 50.0;
        p.sleeping = true;
        p.apply(PetAction::Feed, T0, &mut r).unwrap();
        assert_eq!(p.food, 76.0);
        assert!(!p.sleeping);
        p.food = 95.0;
        assert_eq!(p.apply(PetAction::Feed, T0, &mut r), Err("full"));

        p.food = 50.0;
        p.fun = 50.0;
        p.rest = 50.0;
        p.apply(PetAction::Play, T0, &mut r).unwrap();
        assert_eq!((p.fun, p.food, p.rest), (72.0, 44.0, 42.0));
        p.sleeping = true;
        assert_eq!(p.apply(PetAction::Play, T0, &mut r), Err("asleep"));

        p.poos = 2;
        p.clean = 30.0;
        p.apply(PetAction::Clean, T0, &mut r).unwrap();
        assert_eq!((p.poos, p.clean), (0, 100.0));
        assert_eq!(p.apply(PetAction::Clean, T0, &mut r), Err("clean"));
        assert_eq!(p.apply(PetAction::NewEgg, T0, &mut r), Err("alive"));

        let egg = Pet::egg(T0, 1, 0, None, &mut r);
        assert_eq!(egg.blocked(PetAction::Feed, T0), Some("egg"));
    }

    #[test]
    fn a_new_egg_carries_the_generation_and_record_forward() {
        let mut p = baby();
        p.died_at = Some(T0 + H);
        p.best_age_ms = 5 * H;
        p.apply(PetAction::NewEgg, T0 + 2 * H, &mut || 0.99)
            .unwrap();
        assert_eq!(p.generations, 2);
        assert_eq!(p.best_age_ms, 5 * H);
        assert_eq!(p.hatched_at, None);
        assert_eq!(p.species, "axolotl");
    }

    #[test]
    fn snapshot_has_the_shape_every_client_renders() {
        pulpit_i18n::reset_locale();
        let s = baby().snapshot(T0);
        assert_eq!(s["species"], "blob");
        assert_eq!(s["stage"], "baby");
        assert_eq!(s["mood"], "happy");
        assert_eq!(s["activity"], "idle");
        assert_eq!(s["food"], 100);
        assert_eq!(s["ageMs"], 0);
        assert_eq!(s["generations"], 1);
        assert!(s["label"].as_str().unwrap().contains(" · "));
    }

    #[test]
    fn saved_state_round_trips_and_wild_values_clamp() {
        let p = baby();
        let back = Pet::from_saved(&serde_json::to_value(&p).unwrap()).unwrap();
        assert_eq!(back, p);

        let wild = json!({
            "created": T0, "lastTick": T0, "food": 900, "fun": -5,
            "poos": 12, "species": "dragon", "generations": 0
        });
        let s = Pet::from_saved(&wild).unwrap();
        assert_eq!((s.food, s.fun, s.poos), (100.0, 0.0, MAX_POOS));
        assert_eq!(s.species, "blob");
        assert_eq!(s.generations, 1);
        assert!(Pet::from_saved(&json!({ "species": "cat" })).is_none());
    }

    fn temp_store(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pulpit-pet-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("pet.json")
    }

    #[test]
    fn the_manager_survives_a_restart() {
        let path = temp_store("restart");
        let (tx, mut rx) = unbounded_channel();
        let mgr = PetManager::with_rng(path.clone(), tx, T0, &mut rng0());
        mgr.tick_at(T0 + HATCH_MS, &mut rng0());
        let first = rx.try_recv().expect("hatch pushed")["pet"].clone();
        assert_eq!(first["stage"], "baby");
        mgr.gesture_at("double-tap", T0 + HATCH_MS + M, &mut rng0());
        let cared = mgr.pet();
        assert_eq!(cared.cares, 1);

        // a restart a few seconds later picks the same animal back up
        let (tx2, _rx2) = unbounded_channel();
        let again = PetManager::with_rng(path.clone(), tx2, T0 + HATCH_MS + M + 5_000, &mut rng0());
        let p = again.pet();
        assert_eq!(p.created, cared.created);
        assert_eq!(p.cares, 1);
        assert!(!again.inner.lock().unwrap().adoptable);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn gestures_map_to_care_and_a_dead_long_press_lays_an_egg() {
        let path = temp_store("gestures");
        let (tx, mut rx) = unbounded_channel();
        let mgr = PetManager::with_rng(path.clone(), tx, T0, &mut rng0());
        let now = T0 + HATCH_MS + H;
        mgr.tick_at(now, &mut rng0());
        mgr.gesture_at("tap", now, &mut rng0());
        assert!(mgr.pet().food > 99.0);
        mgr.gesture_at("long-press", now, &mut rng0());
        assert_eq!(mgr.pet().clean, 100.0);

        mgr.inner.lock().unwrap().pet.died_at = Some(now);
        mgr.gesture_at("long-press", now + M, &mut rng0());
        let p = mgr.pet();
        assert_eq!((p.generations, p.hatched_at, p.died_at), (2, None, None));
        while let Ok(v) = rx.try_recv() {
            assert!(v["pet"]["stage"].is_string());
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_routine_drain_pushes_on_a_budget() {
        let path = temp_store("budget");
        let (tx, mut rx) = unbounded_channel();
        let mgr = PetManager::with_rng(path.clone(), tx, T0, &mut rng0());
        let start = T0 + HATCH_MS + 10 * M;
        mgr.tick_at(start, &mut rng0());
        while rx.try_recv().is_ok() {}
        for s in 1..PUSH_EVERY_MS / 1000 {
            mgr.tick_at(start + s * 1000, &mut rng0());
        }
        assert!(rx.try_recv().is_err(), "no push inside the budget");
        mgr.tick_at(start + PUSH_EVERY_MS, &mut rng0());
        assert!(rx.try_recv().is_ok());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_fresh_egg_adopts_the_old_desktop_pet_once() {
        let path = temp_store("adopt");
        let (tx, _rx) = unbounded_channel();
        let mgr = PetManager::with_rng(path.clone(), tx, T0, &mut rng0());
        let mut old = baby();
        old.species = "frog".into();
        old.generations = 4;
        assert!(mgr.adopt_at(&serde_json::to_value(&old).unwrap(), T0 + M, &mut rng0()));
        assert_eq!(mgr.pet().species, "frog");
        assert_eq!(mgr.pet().generations, 4);
        // a second webview (or a reload) cannot replace it again
        assert!(!mgr.adopt_at(&serde_json::to_value(&old).unwrap(), T0 + M, &mut rng0()));
        assert!(path.exists());

        // an uncared-for first egg is on disk too, and a restart keeps it
        // (and closes the adoption window)
        let path2 = temp_store("adopt-fresh");
        let (tx, _rx) = unbounded_channel();
        let fresh = PetManager::with_rng(path2.clone(), tx, T0, &mut || 0.5);
        let laid = fresh.pet();
        let (tx, _rx) = unbounded_channel();
        let again = PetManager::with_rng(path2.clone(), tx, T0 + 10_000, &mut rng0());
        assert_eq!(again.pet().species, laid.species);
        assert!(!again.adopt_at(&serde_json::to_value(&old).unwrap(), T0 + M, &mut rng0()));
        let _ = std::fs::remove_dir_all(path2.parent().unwrap());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_1_11_0_snapshot_file_upgrades_to_a_living_pet() {
        let path = temp_store("upgrade");
        std::fs::write(
            &path,
            r#"{"species":"cat","stage":"adult","food":40,"fun":60,"rest":70,"clean":80,
                "poos":1,"sleeping":false,"ageMs":90000000,"generations":3,"label":"Kot"}"#,
        )
        .unwrap();
        let (tx, _rx) = unbounded_channel();
        let mgr = PetManager::with_rng(path.clone(), tx, T0, &mut rng0());
        let p = mgr.pet();
        assert_eq!(p.species, "cat");
        assert_eq!(p.generations, 3);
        assert_eq!(p.age(T0), 90_000_000);
        assert_eq!(p.phase(T0), "adult");
        assert_eq!(p.food, 40.0);
        // the full state replaced the snapshot on disk
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"lastTick\""));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
