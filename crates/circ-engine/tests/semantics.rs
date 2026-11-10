//! Named semantic cases for SPEC §5.3 / §5.6 / §5.7 / §5.10, the oracle
//! cross-check (property test) and behavioural-vs-gates equivalence.

use circ_compile::compile;
use circ_engine::{oracle::Oracle, Sim, EV_CONTENTION, EV_OSCILLATION};
use circ_model::builder::{ic_pin, MB};
use circ_model::{Id, Module, Project};
use proptest::prelude::*;
use serde_json::json;

struct T {
    p: Project,
    sim: Sim,
}

fn proj(main: Module) -> Project {
    let mut p = Project::new("t");
    p.modules.insert("main".into(), main);
    p
}

impl T {
    fn new(p: Project) -> T {
        let nl = compile(&p);
        assert!(!nl.has_errors(), "{:?}", nl.diags);
        T {
            sim: Sim::new(nl),
            p,
        }
    }
    fn of(main: Module) -> T {
        T::new(proj(main))
    }
    fn net(&self, id: Id, pin: &str) -> u32 {
        self.sim.nl.pin_net(&self.p, "", id, pin).unwrap()
    }
    fn set(&mut self, id: Id, v: u64) {
        let g = self.sim.nl.gate_of("", id).unwrap();
        self.sim.set_input(g, v);
        self.sim.settle();
    }
    fn get(&self, id: Id, pin: &str) -> Option<u64> {
        self.sim.get(self.net(id, pin))
    }
    fn show(&self, id: Id, pin: &str) -> String {
        self.sim.show(self.net(id, pin))
    }
    fn pulse(&mut self, id: Id) {
        self.set(id, 1);
        self.set(id, 0);
    }
}

#[test]
fn shift_register_has_no_race() {
    let mut b = MB::new("main");
    let (d, clk) = (b.add("switch", json!({})), b.add("switch", json!({})));
    let ffs: Vec<Id> = (0..4).map(|_| b.add("dff", json!({}))).collect();
    b.wire((d, "y"), (ffs[0], "d"));
    for k in 0..4 {
        b.wire((clk, "y"), (ffs[k], "clk"));
        if k > 0 {
            b.wire((ffs[k - 1], "q"), (ffs[k], "d"));
        }
    }
    let mut t = T::of(b.finish());
    t.set(d, 1);
    t.pulse(clk);
    t.set(d, 0);
    let q = |t: &T| {
        ffs.iter()
            .map(|f| t.get(*f, "q").unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        q(&t),
        [1, 0, 0, 0],
        "one edge moves the bit exactly one stage"
    );
    t.pulse(clk);
    assert_eq!(q(&t), [0, 1, 0, 0]);
    t.pulse(clk);
    t.pulse(clk);
    assert_eq!(q(&t), [0, 0, 0, 1]);
}

#[test]
fn ripple_counter_counts_through_derived_clocks() {
    let mut b = MB::new("main");
    let clk = b.add("switch", json!({}));
    let ffs: Vec<Id> = (0..4).map(|_| b.add("tff", json!({}))).collect();
    b.wire((clk, "y"), (ffs[0], "clk"));
    for k in 1..4 {
        b.wire((ffs[k - 1], "nq"), (ffs[k], "clk"));
    }
    let mut t = T::of(b.finish());
    for n in 1..=20u64 {
        t.pulse(clk);
        let v: u64 = ffs
            .iter()
            .enumerate()
            .map(|(k, f)| t.get(*f, "q").unwrap() << k)
            .sum();
        assert_eq!(v, n % 16, "a single tick ripples through all four stages");
    }
}

fn nand_latch(b: &mut MB) -> (Id, Id, Id, Id) {
    let (s, r) = (
        b.add("switch", json!({"on": true})),
        b.add("switch", json!({"on": true})),
    );
    let (g1, g2) = (b.add("nand", json!({})), b.add("nand", json!({})));
    b.wire((s, "y"), (g1, "i0"));
    b.wire((g2, "y"), (g1, "i1"));
    b.wire((g1, "y"), (g2, "i0"));
    b.wire((r, "y"), (g2, "i1"));
    (s, r, g1, g2)
}

#[test]
fn sr_latch_from_nands() {
    let mut b = MB::new("main");
    let (s, r, q, nq) = nand_latch(&mut b);
    let mut t = T::of(b.finish());
    assert_eq!(
        t.show(q, "y"),
        "X",
        "power-up state is unknown, not an arbitrary winner"
    );
    assert_eq!(
        t.sim.take_events() & EV_OSCILLATION,
        0,
        "an idle latch is not an oscillation"
    );
    t.set(s, 0);
    assert_eq!((t.get(q, "y"), t.get(nq, "y")), (Some(1), Some(0)));
    t.set(s, 1);
    assert_eq!((t.get(q, "y"), t.get(nq, "y")), (Some(1), Some(0)), "holds");
    t.set(r, 0);
    t.set(r, 1);
    assert_eq!((t.get(q, "y"), t.get(nq, "y")), (Some(0), Some(1)));
}

#[test]
fn sr_latch_simultaneous_release_is_unknown() {
    let mut b = MB::new("main");
    let (s, r, q, nq) = nand_latch(&mut b);
    let mut t = T::of(b.finish());
    // Drive both active (0), then release both in the same instant.
    t.set(s, 0);
    t.set(r, 0);
    assert_eq!((t.get(q, "y"), t.get(nq, "y")), (Some(1), Some(1)));
    t.sim.take_events();
    let (gs, gr) = (
        t.sim.nl.gate_of("", s).unwrap(),
        t.sim.nl.gate_of("", r).unwrap(),
    );
    t.sim.set_input(gs, 1);
    t.sim.set_input(gr, 1);
    t.sim.settle();
    assert_ne!(t.sim.take_events() & EV_OSCILLATION, 0);
    assert_eq!((t.show(q, "y"), t.show(nq, "y")), ("X".into(), "X".into()));
    // And it recovers once driven again.
    t.set(s, 0);
    assert_eq!(t.get(q, "y"), Some(1));
}

#[test]
fn ring_oscillator_is_reported_and_does_not_hang() {
    let mut b = MB::new("main");
    let en = b.add("switch", json!({}));
    let g = b.add("nand", json!({}));
    let (n1, n2) = (b.add("not", json!({})), b.add("not", json!({})));
    b.wire((en, "y"), (g, "i0"));
    b.wire((g, "y"), (n1, "a"));
    b.wire((n1, "y"), (n2, "a"));
    b.wire((n2, "y"), (g, "i1"));
    let mut t = T::of(b.finish());
    assert_eq!(t.get(g, "y"), Some(1), "disabled ring is stable");
    t.sim.take_events();
    t.set(en, 1);
    assert_ne!(t.sim.take_events() & EV_OSCILLATION, 0);
    assert_eq!(t.show(n2, "y"), "X");
    t.set(en, 0);
    assert_eq!(t.get(g, "y"), Some(1), "stops oscillating when disabled");
    assert_eq!(t.get(n2, "y"), Some(1));
}

#[test]
fn tristate_bus_resolution_and_contention() {
    let mut b = MB::new("main");
    let (a, ea) = (
        b.add("number_in", json!({"width": 4, "value": 5})),
        b.add("switch", json!({})),
    );
    let (c, ec) = (
        b.add("number_in", json!({"width": 4, "value": 9})),
        b.add("switch", json!({})),
    );
    let (ta, tc) = (
        b.add("tri", json!({"width": 4})),
        b.add("tri", json!({"width": 4})),
    );
    let out = b.add("number_out", json!({"width": 4}));
    b.wire((a, "y"), (ta, "a"));
    b.wire((ea, "y"), (ta, "en"));
    b.wire((c, "y"), (tc, "a"));
    b.wire((ec, "y"), (tc, "en"));
    b.wire((ta, "y"), (out, "a"));
    b.wire((tc, "y"), (out, "a"));
    let mut t = T::of(b.finish());
    assert_eq!(t.show(out, "a"), "ZZZZ");
    t.set(ea, 1);
    assert_eq!(t.get(out, "a"), Some(5));
    t.sim.take_events();
    t.set(ec, 1);
    assert_ne!(t.sim.take_events() & EV_CONTENTION, 0);
    assert_eq!(
        t.show(out, "a"),
        "XX01",
        "5=0101 vs 9=1001: only differing bits clash"
    );
    t.set(ea, 0);
    assert_eq!(t.get(out, "a"), Some(9));
}

#[test]
fn pull_up_defines_a_floating_bus() {
    let mut b = MB::new("main");
    let (a, en, tri, pull, led) = (
        b.add("const", json!({"value": 0})),
        b.add("switch", json!({})),
        b.add("tri", json!({})),
        b.add("pull", json!({"up": true})),
        b.add("led", json!({})),
    );
    b.wire((a, "y"), (tri, "a"));
    b.wire((en, "y"), (tri, "en"));
    b.wire((tri, "y"), (led, "a"));
    b.wire((pull, "p"), (led, "a"));
    let mut t = T::of(b.finish());
    assert_eq!(t.get(led, "a"), Some(1));
    t.set(en, 1);
    assert_eq!(t.get(led, "a"), Some(0));
}

/// half adder -> full adder -> 8-bit adder, all as nested ICs.
fn nested_adder() -> (Project, Id, Id, Id) {
    let mut p = Project::new("adder");
    let mut h = MB::new("half");
    let (a, b_) = (
        h.add("pin_in", json!({"name": "a"})),
        h.add("pin_in", json!({"name": "b"})),
    );
    let (s, c) = (
        h.add("pin_out", json!({"name": "s"})),
        h.add("pin_out", json!({"name": "c"})),
    );
    let x = h.gate("xor", &[(a, "y"), (b_, "y")]);
    let n = h.gate("and", &[(a, "y"), (b_, "y")]);
    h.wire(x, (s, "a"));
    h.wire(n, (c, "a"));
    p.modules.insert("half".into(), h.finish());
    let hp = [a, b_, s, c].map(ic_pin);

    let mut f = MB::new("full");
    let (fa, fb, fc) = (
        f.add("pin_in", json!({"name": "a"})),
        f.add("pin_in", json!({"name": "b"})),
        f.add("pin_in", json!({"name": "cin"})),
    );
    let (fs, fco) = (
        f.add("pin_out", json!({"name": "s"})),
        f.add("pin_out", json!({"name": "cout"})),
    );
    let (h1, h2) = (f.ic("half"), f.ic("half"));
    f.wire((fa, "y"), (h1, &hp[0]));
    f.wire((fb, "y"), (h1, &hp[1]));
    f.wire((h1, &hp[2]), (h2, &hp[0]));
    f.wire((fc, "y"), (h2, &hp[1]));
    f.wire((h2, &hp[2]), (fs, "a"));
    let or = f.gate("or", &[(h1, &hp[3]), (h2, &hp[3])]);
    f.wire(or, (fco, "a"));
    p.modules.insert("full".into(), f.finish());
    let fp = [fa, fb, fc, fs, fco].map(ic_pin);

    let mut m = MB::new("main");
    let (a, b_) = (
        m.add("number_in", json!({"width": 8})),
        m.add("number_in", json!({"width": 8})),
    );
    let (sa, sb) = (
        m.add("splitter", json!({"width": 8})),
        m.add("splitter", json!({"width": 8})),
    );
    let mg = m.add("merger", json!({"width": 9, "parts": "1,1,1,1,1,1,1,1,1"}));
    let out = m.add("number_out", json!({"width": 9}));
    m.wire((a, "y"), (sa, "a"));
    m.wire((b_, "y"), (sb, "a"));
    let mut carry: Option<Id> = None;
    for k in 0..8 {
        let fa = m.ic("full");
        m.wire((sa, &format!("b{k}")), (fa, &fp[0]));
        m.wire((sb, &format!("b{k}")), (fa, &fp[1]));
        match carry {
            Some(prev) => m.wire((prev, &fp[4]), (fa, &fp[2])),
            None => {
                let zero = m.add("const", json!({"value": 0}));
                m.wire((zero, "y"), (fa, &fp[2]));
            }
        }
        m.wire((fa, &fp[3]), (mg, &format!("b{k}")));
        carry = Some(fa);
    }
    m.wire((carry.unwrap(), &fp[4]), (mg, "b8"));
    m.wire((mg, "y"), (out, "a"));
    p.modules.insert("main".into(), m.finish());
    (p, a, b_, out)
}

#[test]
fn nested_ics_build_an_8_bit_adder() {
    let (p, a, b, out) = nested_adder();
    let mut t = T::new(p);
    assert_eq!(t.sim.nl.parts, 8 * 5, "5 gates per full adder, flattened");
    for (x, y) in [
        (0u64, 0u64),
        (1, 1),
        (255, 1),
        (200, 100),
        (255, 255),
        (170, 85),
        (37, 219),
    ] {
        t.set(a, x);
        t.set(b, y);
        assert_eq!(t.get(out, "a"), Some(x + y));
    }
    // State inside an instance is addressable through its hierarchical path.
    let inner = t
        .sim
        .nl
        .debug
        .iter()
        .filter(|d| t.sim.nl.paths[d.path as usize].matches('/').count() == 2)
        .count();
    assert_eq!(inner, 8 * 2 * 6, "16 half adders with 6 components each");
}

#[test]
fn file_roundtrip_simulates_identically() {
    let (p, a, b, out) = nested_adder();
    let q = Project::from_json(&p.to_json()).unwrap();
    assert_eq!(p, q);
    let (mut t1, mut t2) = (T::new(p), T::new(q));
    for t in [&mut t1, &mut t2] {
        t.set(a, 99);
        t.set(b, 101);
        assert_eq!(t.get(out, "a"), Some(200));
    }
    assert_eq!(t1.sim.hash(), t2.sim.hash());
}

#[test]
fn hierarchy_cycles_and_width_mismatches_are_diagnosed() {
    let mut p = Project::new("bad");
    let mut a = MB::new("a");
    a.ic("a");
    p.modules.insert("a".into(), a.finish());
    let mut m = MB::new("main");
    m.ic("a");
    let (n, l) = (
        m.add("number_in", json!({"width": 4})),
        m.add("led", json!({})),
    );
    m.wire((n, "y"), (l, "a"));
    p.modules.insert("main".into(), m.finish());
    let nl = compile(&p);
    let codes: Vec<_> = nl.diags.iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&"hierarchy_cycle") && codes.contains(&"width_mismatch"),
        "{codes:?}"
    );
    let _ = Sim::new(nl); // still loads without panicking
}

#[test]
fn clock_gating_and_generator() {
    let mut b = MB::new("main");
    let (clk, en) = (
        b.add("clock", json!({"half": 2})),
        b.add("switch", json!({})),
    );
    let g = b.gate("and", &[(clk, "y"), (en, "y")]);
    let cnt = b.add("counter", json!({"width": 8}));
    b.wire(g, (cnt, "clk"));
    let mut t = T::of(b.finish());
    t.sim.run(40);
    assert_eq!(t.get(cnt, "q"), Some(0), "gated off");
    t.set(en, 1);
    t.sim.run(40);
    assert_eq!(t.get(cnt, "q"), Some(10), "one rising edge every 4 ticks");
}

#[test]
fn counter_modulus_load_and_async_clear() {
    let mut b = MB::new("main");
    let (clk, clr, load) = (
        b.add("switch", json!({})),
        b.add("switch", json!({})),
        b.add("switch", json!({})),
    );
    let d = b.add("number_in", json!({"width": 4, "value": 7}));
    let c = b.add("counter", json!({"width": 4, "mod": 10}));
    b.wire((clk, "y"), (c, "clk"));
    b.wire((clr, "y"), (c, "clr"));
    b.wire((load, "y"), (c, "load"));
    b.wire((d, "y"), (c, "d"));
    let mut t = T::of(b.finish());
    for n in 1..=23u64 {
        t.pulse(clk);
        assert_eq!(t.get(c, "q"), Some(n % 10));
        assert_eq!(t.get(c, "co"), Some((n % 10 == 9) as u64));
    }
    t.set(load, 1);
    t.pulse(clk);
    assert_eq!(t.get(c, "q"), Some(7));
    t.set(load, 0);
    t.set(clr, 1);
    assert_eq!(t.get(c, "q"), Some(0), "clear is asynchronous");
}

#[test]
fn ram_rom_and_memory_mapped_outputs() {
    let mut b = MB::new("main");
    let (addr, din, we, clk) = (
        b.add("number_in", json!({"width": 4})),
        b.add("number_in", json!({"width": 8})),
        b.add("switch", json!({})),
        b.add("switch", json!({})),
    );
    let ram = b.add("ram", json!({"addrBits": 4, "width": 8}));
    let rom = b.add(
        "rom",
        json!({"addrBits": 4, "width": 8, "data": "10 20 3f"}),
    );
    b.wire((addr, "y"), (ram, "addr"));
    b.wire((addr, "y"), (rom, "addr"));
    b.wire((din, "y"), (ram, "din"));
    b.wire((we, "y"), (ram, "we"));
    b.wire((clk, "y"), (ram, "clk"));
    let mut t = T::of(b.finish());
    assert_eq!(t.get(rom, "d"), Some(0x10));
    t.set(addr, 2);
    assert_eq!(t.get(rom, "d"), Some(0x3f));
    t.set(din, 0xab);
    t.pulse(clk);
    assert_eq!(t.get(ram, "dout"), Some(0), "write needs WE");
    t.set(we, 1);
    t.pulse(clk);
    assert_eq!(t.get(ram, "dout"), Some(0xab));
    t.set(addr, 3);
    assert_eq!(t.get(ram, "dout"), Some(0));
    t.set(addr, 2);
    assert_eq!(t.get(ram, "dout"), Some(0xab));
}

#[test]
fn hot_patch_preserves_state() {
    let mut b = MB::new("main");
    let clk = b.add("switch", json!({}));
    let cnt = b.add("counter", json!({"width": 8}));
    b.wire((clk, "y"), (cnt, "clk"));
    let (s_id, _, q, _) = nand_latch(&mut b);
    let mut t = T::of(b.finish());
    for _ in 0..5 {
        t.pulse(clk);
    }
    t.set(s_id, 0);
    t.set(s_id, 1);
    assert_eq!(t.get(q, "y"), Some(1));
    // Edit while running: add an LED on the counter output.
    let mut b = MB::from_module(t.p.modules["main"].clone());
    let out = b.add("number_out", json!({"width": 8}));
    b.wire((cnt, "q"), (out, "a"));
    t.p.modules.insert("main".into(), b.finish());
    t.sim = Sim::with_state_from(compile(&t.p), &t.sim);
    assert_eq!(t.get(out, "a"), Some(5), "native state survives");
    assert_eq!(
        t.get(q, "y"),
        Some(1),
        "state held in a gate-level loop survives too"
    );
    t.pulse(clk);
    assert_eq!(t.get(out, "a"), Some(6));
}

#[test]
fn snapshot_restore_replays_deterministically() {
    let mut b = MB::new("main");
    let clk = b.add("clock", json!({"half": 1}));
    let cnt = b.add("counter", json!({"width": 16}));
    let sr = b.add("shiftreg", json!({"width": 16}));
    b.wire((clk, "y"), (cnt, "clk"));
    b.wire((clk, "y"), (sr, "clk"));
    b.wire((cnt, "co"), (sr, "sin"));
    let mut t = T::of(b.finish());
    t.sim.run(100);
    let snap = t.sim.snapshot();
    t.sim.run(1000);
    let h = t.sim.hash();
    t.sim.restore(&snap);
    t.sim.run(1000);
    assert_eq!(t.sim.hash(), h);
}

/// Behavioural D flip-flop vs. its reference implementation in gates
/// (master-slave from NAND latches), driven by the same random stimulus.
#[test]
fn dff_matches_gate_level_reference() {
    let mut b = MB::new("main");
    let (d, clk) = (b.add("switch", json!({})), b.add("switch", json!({})));
    let native = b.add("dff", json!({}));
    b.wire((d, "y"), (native, "d"));
    b.wire((clk, "y"), (native, "clk"));
    let nclk = b.gate("not", &[(clk, "y")]);
    let dlatch = |b: &mut MB, d: (Id, &str), en: (Id, &str)| {
        let nd = b.gate("not", &[d]);
        let s = b.gate("nand", &[d, en]);
        let r = b.gate("nand", &[nd, en]);
        let (q, nq) = (b.add("nand", json!({})), b.add("nand", json!({})));
        b.wire(s, (q, "i0"));
        b.wire((nq, "y"), (q, "i1"));
        b.wire((q, "y"), (nq, "i0"));
        b.wire(r, (nq, "i1"));
        q
    };
    let master = dlatch(&mut b, (d, "y"), nclk);
    let slave = dlatch(&mut b, (master, "y"), (clk, "y"));
    let mut t = T::of(b.finish());
    // Initialise the gate-level version (its power-up state is X by design).
    t.pulse(clk);
    let mut rng = 0x2545F4914F6CDD1Du64;
    for _ in 0..400 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        if rng & 1 == 0 {
            t.set(d, (rng >> 1) & 1);
        } else {
            t.set(clk, (rng >> 1) & 1);
        }
        assert_eq!(t.get(native, "q"), t.get(slave, "y"));
    }
    assert_eq!(t.sim.take_events() & EV_OSCILLATION, 0);
}

// ---------- Oracle cross-check on random designs ----------

#[derive(Debug, Clone)]
struct Spec {
    inputs: usize,
    ffs: Vec<(usize, usize)>,       // (clock source pick, d pick)
    gates: Vec<(u8, usize, usize)>, // (kind, a pick, b pick)
    stim: Vec<(usize, u64, bool)>,  // (input, value, tick?)
}

fn spec() -> impl Strategy<Value = Spec> {
    (
        1usize..5,
        prop::collection::vec((0usize..64, 0usize..64), 0..6),
        prop::collection::vec((0u8..8, 0usize..64, 0usize..64), 1..40),
    )
        .prop_flat_map(|(inputs, ffs, gates)| {
            let stim = prop::collection::vec((0..inputs, 0u64..16, any::<bool>()), 1..60);
            (Just(inputs), Just(ffs), Just(gates), stim)
        })
        .prop_map(|(inputs, ffs, gates, stim)| Spec {
            inputs,
            ffs,
            gates,
            stim,
        })
}

fn build(s: &Spec) -> (Project, Vec<Id>) {
    const W: u8 = 4;
    let mut b = MB::new("main");
    let inputs: Vec<Id> = (0..s.inputs)
        .map(|_| b.add("number_in", json!({"width": W})))
        .collect();
    let mut sig: Vec<(Id, &str)> = inputs.iter().map(|i| (*i, "y")).collect();
    // 1-bit clock candidates: bit 0 of inputs, then earlier flip-flop outputs (ripple).
    let mut clocks: Vec<(Id, &str)> = vec![];
    for i in &inputs {
        let sl = b.add("slice", json!({"width": W, "lo": 0, "out": 1}));
        b.wire((*i, "y"), (sl, "a"));
        clocks.push((sl, "y"));
    }
    let regs: Vec<Id> = s
        .ffs
        .iter()
        .map(|(c, _)| {
            let r = b.add("register", json!({"width": W}));
            let clk = clocks[c % clocks.len()];
            b.wire(clk, (r, "clk"));
            let sl = b.add("slice", json!({"width": W, "lo": 1, "out": 1}));
            b.wire((r, "q"), (sl, "a"));
            clocks.push((sl, "y"));
            sig.push((r, "q"));
            r
        })
        .collect();
    for (k, a, c) in &s.gates {
        let (a, c) = (sig[a % sig.len()], sig[c % sig.len()]);
        let out = match k {
            0 => b.gate_w("and", W, &[a, c]),
            1 => b.gate_w("or", W, &[a, c]),
            2 => b.gate_w("xor", W, &[a, c]),
            3 => b.gate_w("nand", W, &[a, c]),
            4 => b.gate_w("not", W, &[a]),
            5 => {
                let g = b.add("adder", json!({"width": W}));
                b.wire(a, (g, "a"));
                b.wire(c, (g, "b"));
                (g, "s")
            }
            6 => {
                let g = b.add("mux", json!({"width": W, "sel": 1}));
                let sl = b.add("slice", json!({"width": W, "lo": 2, "out": 1}));
                b.wire(a, (sl, "a"));
                b.wire(a, (g, "i0"));
                b.wire(c, (g, "i1"));
                b.wire((sl, "y"), (g, "sel"));
                (g, "y")
            }
            _ => b.gate_w("nor", W, &[a, c]),
        };
        sig.push(out);
    }
    for (r, (_, d)) in regs.iter().zip(&s.ffs) {
        b.wire(sig[d % sig.len()], (*r, "d"));
    }
    (proj(b.finish_raw()), inputs)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]
    #[test]
    fn optimized_engine_matches_oracle(s in spec()) {
        let (p, inputs) = build(&s);
        let nl = compile(&p);
        prop_assert!(!nl.has_errors());
        let mut sim = Sim::new(nl.clone());
        let mut ora = Oracle::new(nl);
        prop_assert_eq!((&sim.val, &sim.flag), (&ora.val, &ora.flag));
        for (i, v, tick) in &s.stim {
            let g = sim.nl.gate_of("", inputs[*i]).unwrap();
            sim.set_input(g, *v);
            ora.set_input(g, *v);
            if *tick { sim.tick(); ora.tick(); } else { sim.settle(); ora.settle(); }
            prop_assert_eq!((&sim.val, &sim.flag, &sim.state), (&ora.val, &ora.flag, &ora.state));
        }
    }
}
