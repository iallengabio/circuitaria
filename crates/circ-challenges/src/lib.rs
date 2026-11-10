//! Challenge format, headless validation harness and scoring.
//! The same code gives instant feedback in the UI and runs in CI.

use circ_compile::{compile, Netlist};
use circ_engine::Sim;
use circ_model::builder::MB;
use circ_model::{catalog, Project};
pub use circ_world::Status;
use circ_world::{Draw, Sig, World, TICKS_PER_STEP};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[cfg(feature = "solutions")]
pub mod solutions;

pub const CAMPAIGN_JSON: &str = include_str!("../../../content/challenges/campaign.json");
/// Safety cap on world steps per scenario.
const MAX_STEPS: u32 = 5000;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Challenge {
    pub id: String,
    pub world: String,
    /// World kind (see `circ_world::KINDS`).
    pub kind: String,
    pub title: BTreeMap<String, String>,
    pub story: BTreeMap<String, String>,
    pub concept: BTreeMap<String, String>,
    /// Part kinds the player may use.
    pub allowed: Vec<String>,
    /// Part budgets for the 2nd and 3rd star.
    pub par: [u32; 2],
    pub unlocks: Vec<String>,
    pub hints: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Campaign {
    pub worlds: Vec<serde_json::Value>,
    pub challenges: Vec<Challenge>,
}

pub fn campaign() -> Campaign {
    serde_json::from_str(CAMPAIGN_JSON).expect("campaign.json is valid")
}

pub fn find(id: &str) -> Option<Challenge> {
    campaign().challenges.into_iter().find(|c| c.id == id)
}

/// Starting project of a challenge: the board with its fixed ports.
pub fn template(ch: &Challenge) -> Project {
    let world = circ_world::make(&ch.kind).expect("known world kind");
    let mut b = MB::new("main");
    for (k, p) in world.inputs().iter().enumerate() {
        let id = b.add("port_in", json!({"name": p.name, "width": p.width}));
        let c = b.m.components.iter_mut().find(|c| c.id == id).unwrap();
        (c.x, c.y) = (60, 80 + k as i32 * 70);
    }
    for (k, p) in world.outputs().iter().enumerate() {
        let id = b.add("port_out", json!({"name": p.name, "width": p.width}));
        let c = b.m.components.iter_mut().find(|c| c.id == id).unwrap();
        (c.x, c.y) = (620, 80 + k as i32 * 70);
    }
    let mut p = Project::new(&ch.id);
    p.modules.insert("main".into(), b.finish_raw());
    p.next_id = 1000;
    p.meta.insert("challenge".into(), json!(ch.id));
    p
}

/// Couples a world to a simulator through the board ports.
pub struct Driver {
    pub world: Box<dyn World>,
    in_gates: Vec<u32>,
    out_nets: Vec<u32>,
    sig: Vec<Sig>,
    inp: Vec<u64>,
    pub steps: u32,
}

impl Driver {
    pub fn new(kind: &str, nl: &Netlist) -> Result<Driver, String> {
        let world = circ_world::make(kind).ok_or("unknown_world")?;
        let in_gates = world
            .inputs()
            .iter()
            .map(|p| {
                nl.ports_in
                    .get(p.name)
                    .copied()
                    .ok_or(format!("missing_port|{}", p.name))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let out_nets = world
            .outputs()
            .iter()
            .map(|p| {
                nl.ports_out
                    .get(p.name)
                    .copied()
                    .ok_or(format!("missing_port|{}", p.name))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Driver {
            sig: vec![Sig::default(); out_nets.len()],
            inp: vec![0; in_gates.len()],
            world,
            in_gates,
            out_nets,
            steps: 0,
        })
    }

    pub fn reset(&mut self, scenario: usize) {
        self.world.reset(scenario);
        self.steps = 0;
    }

    /// One world step followed by TICKS_PER_STEP circuit ticks.
    pub fn step(&mut self, sim: &mut Sim) -> Status {
        for (s, n) in self.sig.iter_mut().zip(&self.out_nets) {
            let v = sim.get(*n);
            *s = Sig {
                val: v.unwrap_or(0),
                known: v.is_some(),
            };
        }
        self.world.step(&self.sig, &mut self.inp);
        for (g, v) in self.in_gates.iter().zip(&self.inp) {
            sim.set_input(*g, *v);
        }
        for _ in 0..TICKS_PER_STEP {
            sim.tick();
        }
        self.steps += 1;
        self.world.status()
    }

    pub fn draw(&self) -> Draw {
        let mut d = Draw::default();
        self.world.draw(&mut d);
        d
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ScenarioResult {
    pub key: String,
    pub hidden: bool,
    pub passed: bool,
    pub code: String,
    pub detail: String,
    pub steps: u32,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Report {
    pub ok: bool,
    /// Blocking problem before any scenario ran (compile error, missing port).
    pub error: String,
    pub illegal: Vec<String>,
    pub scenarios: Vec<ScenarioResult>,
    pub parts: u32,
    pub stars: u32,
    pub par: [u32; 2],
}

/// Runs the player's circuit headless against every scenario and scores it.
pub fn verify(p: &Project, ch: &Challenge) -> Report {
    let nl = compile(p);
    let mut rep = Report {
        parts: nl.parts,
        par: ch.par,
        ..Default::default()
    };
    if let Some(d) = nl.diags.iter().find(|d| d.level == "error") {
        rep.error = format!("compile|{}", d.code);
        return rep;
    }
    rep.illegal = nl
        .kinds
        .keys()
        .filter(|k| catalog::PART_KINDS.contains(&k.as_str()) && !ch.allowed.contains(k))
        .cloned()
        .collect();
    let mut driver = match Driver::new(&ch.kind, &nl) {
        Ok(d) => d,
        Err(e) => {
            rep.error = e;
            return rep;
        }
    };
    for (k, sc) in driver.world.scenarios().into_iter().enumerate() {
        let mut sim = Sim::new(nl.clone());
        driver.reset(k);
        let mut status = Status::Running;
        while status == Status::Running && driver.steps < MAX_STEPS {
            status = driver.step(&mut sim);
        }
        let (code, detail) = match status {
            Status::Won => (String::new(), String::new()),
            Status::Lost { code, detail } => (code.to_string(), detail),
            Status::Running => ("timeout".to_string(), String::new()),
        };
        rep.scenarios.push(ScenarioResult {
            key: sc.key,
            hidden: sc.hidden,
            passed: code.is_empty(),
            code,
            detail,
            steps: driver.steps,
        });
    }
    rep.ok = rep.illegal.is_empty() && rep.scenarios.iter().all(|s| s.passed);
    if rep.ok {
        rep.stars = 1 + (rep.parts <= ch.par[0]) as u32 + (rep.parts <= ch.par[1]) as u32;
    }
    rep
}

/// Hash over traces of every reference project and over a headless run of
/// every challenge world. Must be identical on all platforms and in WASM.
pub fn selftest_hash() -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
        }
    };
    for e in circ_examples::all() {
        let mut sim = Sim::new(compile(&e.project));
        sim.run(3000);
        eat(&sim.hash().to_le_bytes());
    }
    for ch in campaign().challenges {
        let rep = verify(&template(&ch), &ch);
        eat(serde_json::to_string(&rep).unwrap().as_bytes());
        let mut d = Driver::new(&ch.kind, &compile(&template(&ch))).unwrap();
        eat(serde_json::to_string(&d.draw().0).unwrap().as_bytes());
        d.reset(1.min(d.world.scenarios().len() - 1));
    }
    h
}
