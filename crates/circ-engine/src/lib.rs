//! Circuitaria simulation runtime (zero-delay, delta-cycle semantics, SPEC §5.3).
//!
//! tick():
//!   1. advance clock generators (inputs set since the last tick are already pending)
//!   2. delta loop: (a) propagate combinational logic level by level; SCCs are
//!      iterated Jacobi-style (order independent) up to a budget, then frozen
//!      to X; (b) detect clock edges; (c) stop if nothing fired; (d) every
//!      fired element samples from the stable state; (e) commit all at once
//!   3. publish: tick counter, probes

pub mod eval;
pub mod oracle;

use circ_compile::{Netlist, NONE};
use eval::{clk_level, eval_gate, mask, sample, Pending};
use std::collections::HashMap;

pub const EV_OSCILLATION: u32 = 1;
pub const EV_CONTENTION: u32 = 2;
const MAX_DELTA: u32 = 512;
const PROBE_CAP: usize = 2048;

struct Sched {
    dirty: Vec<bool>,
    queues: Vec<Vec<u32>>,
    ndirty: usize,
    lo: usize,
    clk_dirty: Vec<bool>,
    clk_q: Vec<u32>,
}

impl Sched {
    #[inline(always)]
    fn mark(&mut self, g: u32, level: u32) {
        let gu = g as usize;
        if !self.dirty[gu] {
            self.dirty[gu] = true;
            self.queues[level as usize].push(g);
            self.ndirty += 1;
            self.lo = self.lo.min(level as usize);
        }
    }
}

pub struct Probe {
    pub net: u32,
    /// Ring of (val, flag) per tick.
    pub buf: Vec<(u64, u64)>,
    pub head: usize,
}

#[derive(Clone)]
pub struct Snapshot {
    val: Vec<u64>,
    flag: Vec<u64>,
    state: Vec<u64>,
    mems: Vec<Vec<u64>>,
    prev_clk: Vec<u8>,
    tick: u64,
}

pub struct Sim {
    pub nl: Netlist,
    pub val: Vec<u64>,
    pub flag: Vec<u64>,
    frozen: Vec<bool>,
    frozen_list: Vec<u32>,
    sched: Sched,
    pub state: Vec<u64>,
    pub mems: Vec<Vec<u64>>,
    prev_clk: Vec<u8>,
    cur: Vec<u32>,
    jbuf: Vec<(u32, u64, u64)>,
    pending: Vec<Pending>,
    obuf: Vec<(u64, u64)>,
    pub tick: u64,
    /// Events raised since the last call to [`Sim::take_events`].
    events: u32,
    /// Gate evaluations performed (benchmark metric).
    pub evals: u64,
    pub probes: Vec<Probe>,
}

#[inline(always)]
fn write_net(
    nl: &Netlist,
    val: &mut [u64],
    flag: &mut [u64],
    frozen: &[bool],
    sc: &mut Sched,
    n: u32,
    v: u64,
    f: u64,
) {
    let nu = n as usize;
    if (val[nu] == v && flag[nu] == f) || frozen[nu] {
        return;
    }
    val[nu] = v;
    flag[nu] = f;
    for &g in &nl.fan[nl.fan_start[nu] as usize..nl.fan_start[nu + 1] as usize] {
        sc.mark(g, nl.gate_level[g as usize]);
    }
    for &g in &nl.clk_fan[nl.clk_start[nu] as usize..nl.clk_start[nu + 1] as usize] {
        if !sc.clk_dirty[g as usize] {
            sc.clk_dirty[g as usize] = true;
            sc.clk_q.push(g);
        }
    }
}

impl Sim {
    pub fn new(nl: Netlist) -> Sim {
        Self::build(nl, None)
    }

    /// Hot patch: loads a new netlist keeping the state of everything that
    /// still exists (matched by hierarchical path + stable component id).
    pub fn with_state_from(nl: Netlist, old: &Sim) -> Sim {
        Self::build(nl, Some(old))
    }

    fn build(nl: Netlist, old: Option<&Sim>) -> Sim {
        let nn = nl.net_width.len();
        let ng = nl.gates.len();
        let mut s = Sim {
            val: nl.net_width.iter().map(|w| mask(*w)).collect(),
            flag: nl.net_width.iter().map(|w| mask(*w)).collect(),
            frozen: vec![false; nn],
            frozen_list: vec![],
            sched: Sched {
                dirty: vec![false; ng],
                queues: vec![vec![]; nl.levels as usize],
                ndirty: 0,
                lo: 0,
                clk_dirty: vec![false; ng],
                clk_q: vec![],
            },
            state: nl.state_init.clone(),
            mems: nl.mem_init.clone(),
            prev_clk: vec![2; ng],
            cur: vec![],
            jbuf: vec![],
            pending: vec![],
            obuf: vec![(0, 0); 64],
            tick: 0,
            events: 0,
            evals: 0,
            probes: vec![],
            nl,
        };
        if let Some(old) = old {
            s.tick = old.tick;
            let prev: HashMap<(&str, u64), &circ_compile::DebugComp> = old
                .nl
                .debug
                .iter()
                .map(|d| ((old.nl.paths[d.path as usize].as_str(), d.id), d))
                .collect();
            for d in &s.nl.debug {
                let Some(o) = prev.get(&(s.nl.paths[d.path as usize].as_str(), d.id)) else {
                    continue;
                };
                if o.pin_len == d.pin_len {
                    for (a, b) in s.nl.pins_of(d).iter().zip(old.nl.pins_of(o)) {
                        if s.nl.net_width[*a as usize] == old.nl.net_width[*b as usize] {
                            s.val[*a as usize] = old.val[*b as usize];
                            s.flag[*a as usize] = old.flag[*b as usize];
                        }
                    }
                }
                if d.gate == NONE || o.gate == NONE {
                    continue;
                }
                let (g, og) = (&s.nl.gates[d.gate as usize], &old.nl.gates[o.gate as usize]);
                if g.op != og.op || g.width != og.width {
                    continue;
                }
                if g.state != NONE {
                    let (a, b) = (g.state as usize, og.state as usize);
                    s.state[a] = old.state[b];
                    s.state[a + 1] = old.state[b + 1];
                }
                if g.mem != NONE
                    && s.mems[g.mem as usize].len() == old.mems[og.mem as usize].len()
                    && g.op != circ_compile::Op::Rom
                {
                    s.mems[g.mem as usize].copy_from_slice(&old.mems[og.mem as usize]);
                }
            }
        }
        for &n in &s.nl.floating {
            s.val[n as usize] = 0;
            s.flag[n as usize] = mask(s.nl.net_width[n as usize]);
        }
        for (gi, g) in s.nl.gates.iter().enumerate() {
            if let Some(k) = g.op.clk_index() {
                let n = s.nl.ins[g.in_start as usize + k] as usize;
                s.prev_clk[gi] = clk_level(s.val[n], s.flag[n]);
            }
            s.sched.mark(gi as u32, s.nl.gate_level[gi]);
        }
        s.settle();
        s
    }

    /// (a) of the delta cycle: combinational propagation to a fixed point.
    fn propagate(&mut self) {
        let nlev = self.sched.queues.len();
        let mut l = self.sched.lo;
        while self.sched.ndirty > 0 && l < nlev {
            if self.sched.queues[l].is_empty() {
                l += 1;
                continue;
            }
            let limit = self.nl.level_limit[l];
            let mut iter = 0u32;
            loop {
                std::mem::swap(&mut self.cur, &mut self.sched.queues[l]);
                self.sched.ndirty -= self.cur.len();
                self.evals += self.cur.len() as u64;
                for &gi in &self.cur {
                    self.sched.dirty[gi as usize] = false;
                    let g = &self.nl.gates[gi as usize];
                    let (n, clash) = eval_gate(
                        &self.nl,
                        g,
                        &self.val,
                        &self.flag,
                        &mut self.state,
                        &self.mems,
                        &mut self.obuf,
                    );
                    if clash {
                        self.events |= EV_CONTENTION;
                    }
                    for k in 0..n {
                        let net = self.nl.outs[g.out_start as usize + k];
                        let (v, f) = self.obuf[k];
                        if g.scc {
                            self.jbuf.push((net, v, f));
                        } else {
                            write_net(
                                &self.nl,
                                &mut self.val,
                                &mut self.flag,
                                &self.frozen,
                                &mut self.sched,
                                net,
                                v,
                                f,
                            );
                        }
                    }
                }
                self.cur.clear();
                for k in 0..self.jbuf.len() {
                    let (net, v, f) = self.jbuf[k];
                    write_net(
                        &self.nl,
                        &mut self.val,
                        &mut self.flag,
                        &self.frozen,
                        &mut self.sched,
                        net,
                        v,
                        f,
                    );
                }
                self.jbuf.clear();
                if self.sched.queues[l].is_empty() {
                    break;
                }
                iter += 1;
                if iter > limit {
                    // Not converging: freeze the still-unstable outputs to X.
                    self.events |= EV_OSCILLATION;
                    std::mem::swap(&mut self.cur, &mut self.sched.queues[l]);
                    self.sched.ndirty -= self.cur.len();
                    for &gi in &self.cur {
                        self.sched.dirty[gi as usize] = false;
                        let g = &self.nl.gates[gi as usize];
                        for k in 0..g.out_len as usize {
                            let net = self.nl.outs[g.out_start as usize + k];
                            let m = mask(self.nl.net_width[net as usize]);
                            write_net(
                                &self.nl,
                                &mut self.val,
                                &mut self.flag,
                                &self.frozen,
                                &mut self.sched,
                                net,
                                m,
                                m,
                            );
                            if !self.frozen[net as usize] {
                                self.frozen[net as usize] = true;
                                self.frozen_list.push(net);
                            }
                        }
                    }
                    self.cur.clear();
                    iter = limit / 2;
                    if self.sched.queues[l].is_empty() {
                        break;
                    }
                }
            }
            l += 1;
        }
        self.sched.lo = nlev;
    }

    /// Runs delta cycles until nothing changes, without advancing time.
    pub fn settle(&mut self) {
        for n in self.frozen_list.drain(..) {
            self.frozen[n as usize] = false;
        }
        for _ in 0..MAX_DELTA {
            self.propagate();
            if self.sched.clk_q.is_empty() {
                return;
            }
            std::mem::swap(&mut self.cur, &mut self.sched.clk_q);
            for &gi in &self.cur {
                let gu = gi as usize;
                self.sched.clk_dirty[gu] = false;
                let g = &self.nl.gates[gu];
                let n = self.nl.ins[g.in_start as usize + g.op.clk_index().unwrap()] as usize;
                let level = clk_level(self.val[n], self.flag[n]);
                let rising = self.prev_clk[gu] == 0 && level == 1;
                self.prev_clk[gu] = level;
                if rising {
                    if let Some(p) =
                        sample(&self.nl, gi, &self.val, &self.flag, &self.state, &self.mems)
                    {
                        self.pending.push(p);
                    }
                }
            }
            self.cur.clear();
            for k in 0..self.pending.len() {
                let gi = match self.pending[k] {
                    Pending::State(gi, v, f) => {
                        let s = self.nl.gates[gi as usize].state as usize;
                        self.state[s] = v;
                        self.state[s + 1] = f;
                        gi
                    }
                    Pending::Mem(gi, addr, data) => {
                        self.mems[self.nl.gates[gi as usize].mem as usize][addr as usize] = data;
                        gi
                    }
                    Pending::Push(gi, data) => {
                        let g = &self.nl.gates[gi as usize];
                        let s = g.state as usize;
                        self.mems[g.mem as usize][(self.state[s] % 1024) as usize] = data;
                        self.state[s] += 1;
                        gi
                    }
                };
                self.sched.mark(gi, self.nl.gate_level[gi as usize]);
            }
            self.pending.clear();
        }
        self.events |= EV_OSCILLATION;
    }

    pub fn tick(&mut self) {
        for k in 0..self.nl.clocks.len() {
            let gi = self.nl.clocks[k];
            let g = &self.nl.gates[gi as usize];
            let s = g.state as usize;
            self.state[s + 1] += 1;
            if self.state[s + 1] >= g.param {
                self.state[s + 1] = 0;
                self.state[s] ^= 1;
                self.sched.mark(gi, self.nl.gate_level[gi as usize]);
            }
        }
        self.settle();
        self.tick += 1;
        for p in &mut self.probes {
            let n = p.net as usize;
            if p.buf.len() < PROBE_CAP {
                p.buf.push((self.val[n], self.flag[n]));
            } else {
                p.buf[p.head] = (self.val[n], self.flag[n]);
                p.head = (p.head + 1) % PROBE_CAP;
            }
        }
    }

    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    /// Sets an input gate (switch, button, number, port). Takes effect on the
    /// next `settle()` or `tick()`.
    pub fn set_input(&mut self, gate: u32, value: u64) {
        let Some(g) = self.nl.gates.get(gate as usize) else {
            return;
        };
        if g.op != circ_compile::Op::Input {
            return;
        }
        let s = g.state as usize;
        if self.state[s] != value {
            self.state[s] = value;
            self.sched.mark(gate, self.nl.gate_level[gate as usize]);
        }
    }

    pub fn take_events(&mut self) -> u32 {
        std::mem::take(&mut self.events)
    }

    /// Value of a net if every bit is 0/1.
    pub fn get(&self, net: u32) -> Option<u64> {
        let n = net as usize;
        let m = mask(self.nl.net_width[n]);
        (self.flag[n] & m == 0).then_some(self.val[n] & m)
    }

    /// 4-state rendering, MSB first: "01ZX".
    pub fn show(&self, net: u32) -> String {
        let n = net as usize;
        (0..self.nl.net_width[n])
            .rev()
            .map(
                |b| match ((self.val[n] >> b) & 1, (self.flag[n] >> b) & 1) {
                    (0, 0) => '0',
                    (1, 0) => '1',
                    (0, _) => 'Z',
                    _ => 'X',
                },
            )
            .collect()
    }

    pub fn probe(&mut self, net: u32) -> usize {
        self.probes.push(Probe {
            net,
            buf: vec![],
            head: 0,
        });
        self.probes.len() - 1
    }

    /// Probe samples, oldest first.
    pub fn probe_read(&self, id: usize) -> Vec<(u64, u64)> {
        let p = &self.probes[id];
        let mut v = p.buf[p.head..].to_vec();
        v.extend_from_slice(&p.buf[..p.head]);
        v
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            val: self.val.clone(),
            flag: self.flag.clone(),
            state: self.state.clone(),
            mems: self.mems.clone(),
            prev_clk: self.prev_clk.clone(),
            tick: self.tick,
        }
    }

    pub fn restore(&mut self, s: &Snapshot) {
        self.val.copy_from_slice(&s.val);
        self.flag.copy_from_slice(&s.flag);
        self.state.copy_from_slice(&s.state);
        self.mems.clone_from(&s.mems);
        self.prev_clk.copy_from_slice(&s.prev_clk);
        self.tick = s.tick;
    }

    /// Approximate heap bytes held by the runtime (netlist + state).
    pub fn mem_bytes(&self) -> usize {
        let nl = &self.nl;
        let v = |len: usize, size: usize| len * size;
        v(nl.net_width.len(), 1 + 8 + 8 + 1)
            + v(
                nl.gates.len(),
                std::mem::size_of::<circ_compile::Gate>() + 4 + 1 + 1 + 1,
            )
            + v(
                nl.ins.len()
                    + nl.outs.len()
                    + nl.fan.len()
                    + nl.clk_fan.len()
                    + nl.fan_start.len()
                    + nl.clk_start.len(),
                4,
            )
            + v(self.state.len(), 8)
            + self.mems.iter().map(|m| m.len() * 8).sum::<usize>()
            + self
                .sched
                .queues
                .iter()
                .map(|q| q.capacity() * 4 + 24)
                .sum::<usize>()
    }

    /// FNV-1a over the full observable state (determinism checks).
    pub fn hash(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        let mut eat = |x: u64| {
            for b in x.to_le_bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100000001b3);
            }
        };
        self.val
            .iter()
            .chain(&self.flag)
            .chain(&self.state)
            .for_each(|x| eat(*x));
        self.mems.iter().flatten().for_each(|x| eat(*x));
        eat(self.tick);
        h
    }
}
