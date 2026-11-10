//! WebAssembly boundary. A deliberately tiny C ABI (no wasm-bindgen):
//!
//! - `call(ptr, len)`: JSON command in, JSON reply out (cold path: load,
//!   catalog, challenges, probes…). The reply is read from `out_ptr()`.
//! - `run`, `settle`, `set_input`: hot path, plain integers.
//! - `val_ptr` / `flag_ptr` / `mem_ptr`: zero-copy views of the simulation
//!   state for rendering (two u64 planes per net, see circ-engine::eval).

use circ_challenges::Driver;
use circ_compile::{compile, NONE};
use circ_engine::{Sim, Snapshot};
use circ_model::{catalog, module_at, Project};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::VecDeque;

const HISTORY_CAP: usize = 240;
const HISTORY_MAX_NETS: usize = 200_000;

#[derive(Default)]
struct St {
    project: Option<Project>,
    sim: Option<Sim>,
    driver: Option<Driver>,
    history: VecDeque<Snapshot>,
    out: Vec<u8>,
    bench_inputs: Vec<u32>,
}

thread_local! {
    static ST: RefCell<St> = RefCell::new(St::default());
}

#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` must come from `alloc(len)` with the same `len` and not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    // SAFETY: per contract, (ptr, capacity = max(len, 1)) is exactly the allocation made by `alloc`.
    drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
}

/// Runs a JSON command; returns the reply length (bytes at `out_ptr()`).
///
/// # Safety
/// `ptr..ptr+len` must be a readable buffer (normally from `alloc`).
#[no_mangle]
pub unsafe extern "C" fn call(ptr: *const u8, len: usize) -> usize {
    // SAFETY: the caller guarantees the buffer is valid for `len` bytes for the duration of the call.
    let input = std::slice::from_raw_parts(ptr, len);
    let reply = dispatch(std::str::from_utf8(input).unwrap_or(""));
    ST.with(|s| {
        let mut s = s.borrow_mut();
        s.out = reply.into_bytes();
        s.out.len()
    })
}

#[no_mangle]
pub extern "C" fn out_ptr() -> *const u8 {
    ST.with(|s| s.borrow().out.as_ptr())
}

/// Advances `n` ticks; returns the event flags raised (see circ-engine EV_*).
#[no_mangle]
pub extern "C" fn run(n: u32) -> u32 {
    ST.with(|s| {
        let s = &mut *s.borrow_mut();
        let Some(sim) = s.sim.as_mut() else { return 0 };
        if sim.nl.net_width.len() <= HISTORY_MAX_NETS {
            if s.history.len() >= HISTORY_CAP {
                s.history.pop_front();
            }
            s.history.push_back(sim.snapshot());
        }
        sim.run(n as u64);
        sim.take_events()
    })
}

#[no_mangle]
pub extern "C" fn settle() -> u32 {
    with_sim(0, |sim| {
        sim.settle();
        sim.take_events()
    })
}

#[no_mangle]
pub extern "C" fn set_input(gate: u32, lo: u32, hi: u32) {
    with_sim((), |sim| sim.set_input(gate, (hi as u64) << 32 | lo as u64));
}

#[no_mangle]
pub extern "C" fn val_ptr() -> *const u64 {
    with_sim(std::ptr::null(), |sim| sim.val.as_ptr())
}
#[no_mangle]
pub extern "C" fn flag_ptr() -> *const u64 {
    with_sim(std::ptr::null(), |sim| sim.flag.as_ptr())
}
#[no_mangle]
pub extern "C" fn net_count() -> u32 {
    with_sim(0, |sim| sim.val.len() as u32)
}
#[no_mangle]
pub extern "C" fn tick_count() -> f64 {
    with_sim(0.0, |sim| sim.tick as f64)
}
#[no_mangle]
pub extern "C" fn eval_count() -> f64 {
    with_sim(0.0, |sim| sim.evals as f64)
}
/// Memory words of a RAM/ROM/pixel/terminal gate (null if none).
#[no_mangle]
pub extern "C" fn mem_ptr(gate: u32) -> *const u64 {
    with_sim(std::ptr::null(), |sim| match sim.nl.gates.get(gate as usize) {
        Some(g) if g.mem != NONE => sim.mems[g.mem as usize].as_ptr(),
        _ => std::ptr::null(),
    })
}
#[no_mangle]
pub extern "C" fn mem_len(gate: u32) -> u32 {
    with_sim(0, |sim| match sim.nl.gates.get(gate as usize) {
        Some(g) if g.mem != NONE => sim.mems[g.mem as usize].len() as u32,
        _ => 0,
    })
}
/// First state word of a gate (input value, terminal write count…).
#[no_mangle]
pub extern "C" fn gate_state(gate: u32) -> f64 {
    with_sim(0.0, |sim| match sim.nl.gates.get(gate as usize) {
        Some(g) if g.state != NONE => sim.state[g.state as usize] as f64,
        _ => 0.0,
    })
}

/// Benchmark helper: random input activity on a `bench` netlist.
#[no_mangle]
pub extern "C" fn bench_random(ticks: u32) -> f64 {
    ST.with(|s| {
        let s = &mut *s.borrow_mut();
        let (Some(sim), ins) = (s.sim.as_mut(), &s.bench_inputs) else { return 0.0 };
        let mut rng = 0x1234_5678_9abc_def1u64;
        sim.evals = 0;
        for _ in 0..ticks {
            for _ in 0..3 {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                sim.set_input(ins[(rng % ins.len() as u64) as usize], (rng >> 8) & 1);
            }
            sim.tick();
        }
        sim.evals as f64
    })
}

fn with_sim<T>(default: T, f: impl FnOnce(&mut Sim) -> T) -> T {
    ST.with(|s| match s.borrow_mut().sim.as_mut() {
        Some(sim) => f(sim),
        None => default,
    })
}

fn dispatch(input: &str) -> String {
    let v: Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => return json!({"error": format!("bad_json: {e}")}).to_string(),
    };
    let cmd = v["cmd"].as_str().unwrap_or("");
    ST.with(|s| command(&mut s.borrow_mut(), cmd, &v)).to_string()
}

fn load(s: &mut St, p: Project, keep: bool) -> Value {
    let nl = compile(&p);
    let info = json!({"ok": !nl.has_errors(), "diags": nl.diags, "nets": nl.net_width.len(), "gates": nl.gates.len(), "levels": nl.levels, "parts": nl.parts});
    let sim = match (&s.sim, keep) {
        (Some(old), true) => Sim::with_state_from(nl, old),
        _ => Sim::new(nl),
    };
    s.sim = Some(sim);
    s.project = Some(p);
    s.history.clear();
    s.driver = None;
    info
}

fn command(s: &mut St, cmd: &str, v: &Value) -> Value {
    match cmd {
        "catalog" => catalog::catalog(),
        "pins" => {
            let params = v["params"].as_object().map(|o| o.clone().into_iter().collect()).unwrap_or_default();
            json!(catalog::pins(v["kind"].as_str().unwrap_or(""), &params))
        }
        "load" => match serde_json::to_string(&v["project"]).map_err(|e| e.to_string()).and_then(|j| Project::from_json(&j)) {
            Ok(p) => load(s, p, v["keep"].as_bool().unwrap_or(false)),
            Err(e) => json!({"ok": false, "error": e}),
        },
        "view" => {
            let (Some(sim), Some(p)) = (&s.sim, &s.project) else { return json!([]) };
            let path = v["path"].as_str().unwrap_or("");
            if module_at(p, path).is_none() {
                return json!([]);
            }
            let widths: Vec<Value> = sim
                .nl
                .debug_in(path)
                .map(|d| {
                    let nets = sim.nl.pins_of(d);
                    json!({"id": d.id, "gate": if d.gate == NONE { -1 } else { d.gate as i64 }, "nets": nets,
                           "w": nets.iter().map(|n| sim.nl.net_width[*n as usize]).collect::<Vec<_>>()})
                })
                .collect();
            json!(widths)
        }
        "probe" => {
            let Some(sim) = s.sim.as_mut() else { return json!(null) };
            sim.probes.clear();
            for n in v["nets"].as_array().into_iter().flatten() {
                if let Some(n) = n.as_u64().filter(|n| (*n as usize) < sim.val.len()) {
                    sim.probe(n as u32);
                }
            }
            json!(sim.probes.len())
        }
        "probes" => {
            let Some(sim) = &s.sim else { return json!([]) };
            json!((0..sim.probes.len())
                .map(|k| {
                    let data = sim.probe_read(k);
                    json!({"net": sim.probes[k].net, "v": data.iter().map(|d| d.0).collect::<Vec<_>>(), "f": data.iter().map(|d| d.1).collect::<Vec<_>>()})
                })
                .collect::<Vec<_>>())
        }
        "rewind" => {
            let Some(sim) = s.sim.as_mut() else { return json!(null) };
            let mut last = None;
            for _ in 0..v["steps"].as_u64().unwrap_or(1) {
                match s.history.pop_back() {
                    Some(snap) => last = Some(snap),
                    None => break,
                }
            }
            if let Some(snap) = last {
                sim.restore(&snap);
            }
            json!({"tick": sim.tick, "left": s.history.len()})
        }
        "campaign" => serde_json::from_str(circ_challenges::CAMPAIGN_JSON).unwrap_or(Value::Null),
        "template" => match circ_challenges::find(v["id"].as_str().unwrap_or("")) {
            Some(ch) => {
                let world = circ_challenges::template(&ch);
                serde_json::to_value(world).unwrap_or(Value::Null)
            }
            None => Value::Null,
        },
        "ch_start" => {
            let (Some(ch), Some(p)) = (circ_challenges::find(v["id"].as_str().unwrap_or("")), s.project.clone()) else {
                return json!({"ok": false, "error": "no_challenge"});
            };
            let nl = compile(&p);
            if nl.has_errors() {
                return json!({"ok": false, "error": format!("compile|{}", nl.diags[0].code)});
            }
            match Driver::new(&ch.kind, &nl) {
                Ok(mut d) => {
                    d.reset(v["scenario"].as_u64().unwrap_or(0) as usize);
                    let scenarios = d.world.scenarios();
                    s.sim = Some(Sim::new(nl));
                    s.history.clear();
                    s.driver = Some(d);
                    json!({"ok": true, "scenarios": scenarios})
                }
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "ch_step" => {
            let (Some(d), Some(sim)) = (s.driver.as_mut(), s.sim.as_mut()) else { return json!(null) };
            let mut status = d.world.status();
            for _ in 0..v["n"].as_u64().unwrap_or(1) {
                if status != circ_world_status_running() {
                    break;
                }
                status = d.step(sim);
            }
            json!({"status": status, "progress": d.world.progress(), "steps": d.steps, "shapes": d.draw().0, "events": sim.take_events()})
        }
        "ch_stop" => {
            s.driver = None;
            json!(true)
        }
        "ch_preview" => {
            let Some(ch) = circ_challenges::find(v["id"].as_str().unwrap_or("")) else { return json!(null) };
            let nl = compile(&circ_challenges::template(&ch));
            match Driver::new(&ch.kind, &nl) {
                Ok(d) => json!({"shapes": d.draw().0, "scenarios": d.world.scenarios()}),
                Err(_) => json!(null),
            }
        }
        "ch_verify" => match (circ_challenges::find(v["id"].as_str().unwrap_or("")), &s.project) {
            (Some(ch), Some(p)) => serde_json::to_value(circ_challenges::verify(p, &ch)).unwrap_or(Value::Null),
            _ => Value::Null,
        },
        "hash" => json!(format!("{:016x}", circ_challenges::selftest_hash())),
        "bench" => {
            let n = v["n"].as_u64().unwrap_or(100_000) as usize;
            if v["kind"] == "cpu" {
                let (mut p, _, _) = circ_examples::cpu8();
                for c in &mut p.modules.get_mut("main").unwrap().components {
                    if c.kind == "ram" {
                        c.params.insert("data".into(), json!([0x2e, 0xe0, 0x60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0]));
                    }
                }
                load(s, p, false)
            } else {
                let (p, ins) = circ_examples::random_netlist(n, 64, 42);
                let info = load(s, p, false);
                let sim = s.sim.as_ref().unwrap();
                s.bench_inputs = ins.iter().filter_map(|i| sim.nl.gate_of("", *i)).collect();
                info
            }
        }
        _ => json!({"error": format!("unknown_cmd: {cmd}")}),
    }
}

fn circ_world_status_running() -> circ_challenges::Status {
    circ_challenges::Status::Running
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call_json(v: Value) -> Value {
        let input = v.to_string();
        // SAFETY: the buffer is a live String for the whole call.
        let len = unsafe { call(input.as_ptr(), input.len()) };
        // SAFETY: out_ptr() points at `len` bytes owned by the thread-local state until the next call.
        let out = unsafe { std::slice::from_raw_parts(out_ptr(), len) };
        serde_json::from_slice(out).unwrap()
    }

    #[test]
    fn abi_roundtrip_load_run_view() {
        let p = alloc(16);
        // SAFETY: freshly allocated with the same length.
        unsafe { dealloc(p, 16) };
        assert!(call_json(json!({"cmd": "catalog"})).as_array().unwrap().len() > 30);
        let ex = circ_examples::ripple_counter();
        let info = call_json(json!({"cmd": "load", "project": ex}));
        assert_eq!(info["ok"], true, "{info}");
        run(64);
        assert_eq!(tick_count(), 64.0);
        let view = call_json(json!({"cmd": "view", "path": ""}));
        assert_eq!(view.as_array().unwrap().len(), ex.modules["main"].components.len());
        assert_eq!(call_json(json!({"cmd": "rewind", "steps": 1}))["tick"], 0);
        assert!(!val_ptr().is_null() && net_count() > 0);
        assert_eq!(call_json(json!({"cmd": "nope"}))["error"], "unknown_cmd: nope");
    }

    #[test]
    fn challenge_session() {
        let t = call_json(json!({"cmd": "template", "id": "w1-l01-lampada"}));
        assert_eq!(call_json(json!({"cmd": "load", "project": t}))["ok"], true);
        assert_eq!(call_json(json!({"cmd": "ch_start", "id": "w1-l01-lampada", "scenario": 0}))["ok"], true);
        let r = call_json(json!({"cmd": "ch_step", "n": 50}));
        assert_eq!(r["status"]["state"], "lost");
        assert!(r["shapes"].as_array().unwrap().len() > 3);
        assert_eq!(call_json(json!({"cmd": "ch_verify", "id": "w1-l01-lampada"}))["ok"], false);
    }
}
