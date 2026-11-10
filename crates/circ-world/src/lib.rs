//! Deterministic 2D worlds that the player's circuit is plugged into.
//!
//! A world exposes board ports (sensors in, actuators out), a list of
//! scenarios, a fixed-step `step()` using integers only, a win/lose status and
//! a display list. No floats, no clocks, no I/O: identical on native and WASM.

use serde::Serialize;

mod closed;
mod draw;
mod stim;
pub use draw::{Draw, Shape};

/// Circuit ticks simulated per world step.
pub const TICKS_PER_STEP: u32 = 4;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Port {
    pub name: &'static str,
    pub width: u8,
}

/// A value read from the circuit; `known` is false if any bit is Z or X.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sig {
    pub val: u64,
    pub known: bool,
}

impl Sig {
    pub fn on(self) -> bool {
        self.known && self.val & 1 == 1
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Status {
    Running,
    Won,
    /// `code` is an i18n key suffix; `detail` carries values for the message.
    Lost {
        code: &'static str,
        detail: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct Scenario {
    /// i18n key suffix for visible scenarios.
    pub key: String,
    pub hidden: bool,
}

pub trait World {
    /// World -> circuit (sensors, buttons, clocks).
    fn inputs(&self) -> Vec<Port>;
    /// Circuit -> world (actuators).
    fn outputs(&self) -> Vec<Port>;
    fn scenarios(&self) -> Vec<Scenario>;
    fn reset(&mut self, scenario: usize);
    /// One world step: reads what the circuit drives, updates the world and
    /// writes the new sensor values.
    fn step(&mut self, out: &[Sig], inp: &mut [u64]);
    fn status(&self) -> Status;
    /// Progress in 0..=100 for the UI.
    fn progress(&self) -> u32;
    fn draw(&self, d: &mut Draw);
}

pub const KINDS: &[&str] = &[
    "lamp",
    "vault",
    "door",
    "museum",
    "council",
    "market",
    "lock",
    "turret",
    "factory",
    "traffic",
    "vending",
    "elevator",
    "line",
    "telegraph",
    "simon",
    "maze",
];

pub fn make(kind: &str) -> Option<Box<dyn World>> {
    Some(match kind {
        "elevator" => Box::new(closed::Elevator::default()),
        "line" => Box::new(closed::Line::default()),
        "maze" => Box::new(closed::Maze::default()),
        k => Box::new(stim::Stim::new(stim::Kind::parse(k)?)),
    })
}

/// xorshift64*: the only randomness source for worlds.
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545F4914F6CDD1D) >> 16
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}
