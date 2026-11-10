//! Design (hierarchical) -> CompiledNetlist (flat, levelized, ready to run).
//!
//! Pipeline: flatten hierarchy with a union-find over pins -> nets and widths
//! -> gates -> bus resolution and pin defaults -> SCC + levels -> CSR arrays.

use circ_model::{catalog, Component, Dir, Id, PinDesc, Project};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

pub const NONE: u32 = u32::MAX;
const MAX_DEPTH: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Buf,
    Not,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Tri,
    Const,
    /// Multi-driver / pulled net resolution. param: 0 none, 1 pull-up, 2 pull-down.
    Resolve,
    Mux,
    Demux,
    Decoder,
    Split,
    Merge,
    Slice,
    Adder,
    Cmp,
    LatchSr,
    LatchD,
    Dff,
    Tff,
    Jkff,
    Register,
    Counter,
    ShiftReg,
    Rom,
    Ram,
    Pixels,
    Terminal,
    Input,
    Clock,
}

impl Op {
    /// Index of the clock input for edge-triggered ops.
    pub fn clk_index(self) -> Option<usize> {
        Some(match self {
            Op::Dff | Op::Tff | Op::Register | Op::ShiftReg | Op::Terminal => 1,
            Op::Jkff => 2,
            Op::Counter => 0,
            Op::Ram => 3,
            Op::Pixels => 4,
            _ => return None,
        })
    }
    /// Is input `i` a combinational dependency of the outputs?
    /// Edge-triggered ops only depend on async controls (and RAM on addr).
    pub fn comb_input(self, i: usize) -> bool {
        match self {
            Op::Dff | Op::Tff => i == 2,
            Op::Jkff | Op::Register | Op::ShiftReg => i == 3,
            Op::Counter => i == 2,
            Op::Ram => i == 0,
            Op::Pixels | Op::Terminal => false,
            _ => true,
        }
    }
    pub fn has_state(self) -> bool {
        matches!(
            self,
            Op::LatchSr | Op::LatchD | Op::Dff | Op::Tff | Op::Jkff | Op::Register | Op::Counter | Op::ShiftReg | Op::Input | Op::Clock | Op::Terminal
        )
    }
}

#[derive(Clone, Debug)]
pub struct Gate {
    pub op: Op,
    pub width: u8,
    pub scc: bool,
    pub in_len: u16,
    pub out_len: u16,
    pub in_start: u32,
    pub out_start: u32,
    /// Index into the state array (2 words: val, flag) or NONE.
    pub state: u32,
    /// Index into mems or NONE.
    pub mem: u32,
    pub param: u64,
}

#[derive(Clone, Debug)]
pub struct DebugComp {
    pub path: u32,
    pub id: Id,
    pub gate: u32,
    pub pin_start: u32,
    pub pin_len: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diag {
    pub level: &'static str,
    pub code: &'static str,
    pub path: String,
    pub comp: Id,
    pub detail: String,
}

#[derive(Default, Clone, Debug)]
pub struct Netlist {
    pub net_width: Vec<u8>,
    pub gates: Vec<Gate>,
    pub gate_level: Vec<u32>,
    pub ins: Vec<u32>,
    pub outs: Vec<u32>,
    /// CSR: net -> gates combinationally depending on it.
    pub fan_start: Vec<u32>,
    pub fan: Vec<u32>,
    /// CSR: net -> edge-triggered gates clocked by it.
    pub clk_start: Vec<u32>,
    pub clk_fan: Vec<u32>,
    pub levels: u32,
    /// Fixed-point iteration budget per level (covers its SCC gates).
    pub level_limit: Vec<u32>,
    pub state_init: Vec<u64>,
    pub mem_init: Vec<Vec<u64>>,
    pub clocks: Vec<u32>,
    /// Nets without any driver: they float (Z).
    pub floating: Vec<u32>,
    pub paths: Vec<String>,
    /// Sorted by (path string, id).
    pub debug: Vec<DebugComp>,
    pub debug_nets: Vec<u32>,
    pub ports_in: BTreeMap<String, u32>,
    pub ports_out: BTreeMap<String, u32>,
    pub diags: Vec<Diag>,
    pub parts: u32,
    pub kinds: BTreeMap<String, u32>,
}

impl Netlist {
    pub fn has_errors(&self) -> bool {
        self.diags.iter().any(|d| d.level == "error")
    }

    pub fn debug_comp(&self, path: &str, id: Id) -> Option<&DebugComp> {
        self.debug.binary_search_by(|d| (self.paths[d.path as usize].as_str(), d.id).cmp(&(path, id))).ok().map(|k| &self.debug[k])
    }

    /// Components directly inside the instance at `path`.
    pub fn debug_in<'a>(&'a self, path: &'a str) -> impl Iterator<Item = &'a DebugComp> + 'a {
        let start = self.debug.partition_point(|d| self.paths[d.path as usize].as_str() < path);
        self.debug[start..].iter().take_while(move |d| self.paths[d.path as usize] == path)
    }

    pub fn pins_of(&self, d: &DebugComp) -> &[u32] {
        &self.debug_nets[d.pin_start as usize..(d.pin_start + d.pin_len) as usize]
    }

    /// Net of a named pin (slow path: tests, harness, tools).
    pub fn pin_net(&self, p: &Project, path: &str, id: Id, pin: &str) -> Option<u32> {
        let d = self.debug_comp(path, id)?;
        let (_, m) = circ_model::module_at(p, path)?;
        let c = m.components.iter().find(|c| c.id == id)?;
        let pins = pins_for(p, c)?;
        let k = pins.iter().position(|pd| pd.name == pin)?;
        self.pins_of(d).get(k).copied()
    }

    pub fn gate_of(&self, path: &str, id: Id) -> Option<u32> {
        self.debug_comp(path, id).map(|d| d.gate).filter(|g| *g != NONE)
    }
}

/// Pins of any component, resolving `ic` through the project.
pub fn pins_for(p: &Project, c: &Component) -> Option<Vec<PinDesc>> {
    if c.kind == "ic" {
        p.modules.get(catalog::ps(&c.params, "module", "")).map(catalog::module_pins)
    } else {
        catalog::pins(&c.kind, &c.params)
    }
}

struct Fc<'a> {
    path: u32,
    comp: &'a Component,
    pins: Vec<(PinDesc, u32)>,
    root: bool,
}

struct Cx<'a> {
    p: &'a Project,
    uf: Vec<u32>,
    flat: Vec<Fc<'a>>,
    diags: Vec<Diag>,
    paths: Vec<String>,
}

impl<'a> Cx<'a> {
    fn node(&mut self) -> u32 {
        self.uf.push(self.uf.len() as u32);
        self.uf.len() as u32 - 1
    }
    fn find(&mut self, mut x: u32) -> u32 {
        while self.uf[x as usize] != x {
            let p = self.uf[x as usize];
            self.uf[x as usize] = self.uf[p as usize];
            x = p;
        }
        x
    }
    fn union(&mut self, a: u32, b: u32) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.uf[a.max(b) as usize] = a.min(b);
        }
    }
    fn diag(&mut self, level: &'static str, code: &'static str, path: &str, comp: Id, detail: String) {
        if self.diags.len() < 200 {
            self.diags.push(Diag { level, code, path: path.into(), comp, detail });
        }
    }

    /// Instantiates module `mid` at `path`; returns interface nodes by inner pin id.
    fn inst(&mut self, mid: &'a str, path: String, stack: &mut Vec<&'a str>) -> HashMap<Id, u32> {
        let mut iface = HashMap::new();
        let p: &'a Project = self.p;
        let Some(m) = p.modules.get(mid) else {
            self.diag("error", "missing_module", &path, 0, mid.into());
            return iface;
        };
        let pidx = self.paths.len() as u32;
        self.paths.push(path.clone());
        let mut local: HashMap<Id, usize> = HashMap::with_capacity(m.components.len());
        let mut tunnels: BTreeMap<&str, u32> = BTreeMap::new();
        for c in &m.components {
            let pins = if c.kind == "ic" {
                let child = catalog::ps(&c.params, "module", "");
                match p.modules.get(child) {
                    Some(cm) => catalog::module_pins(cm),
                    None => {
                        self.diag("error", "missing_module", &path, c.id, child.into());
                        continue;
                    }
                }
            } else {
                match catalog::pins(&c.kind, &c.params) {
                    Some(v) => v,
                    None => {
                        self.diag("error", "unknown_kind", &path, c.id, c.kind.clone());
                        continue;
                    }
                }
            };
            let pn: Vec<(PinDesc, u32)> = pins.into_iter().map(|pd| (pd, self.node())).collect();
            match c.kind.as_str() {
                "ic" => {
                    let child = catalog::ps(&c.params, "module", "");
                    if stack.contains(&child) || stack.len() >= MAX_DEPTH {
                        self.diag("error", "hierarchy_cycle", &path, c.id, child.into());
                    } else {
                        stack.push(child);
                        let inner = self.inst(child, format!("{path}{}/", c.id), stack);
                        stack.pop();
                        for (pd, node) in &pn {
                            if let Some(n) = pd.name[1..].parse::<Id>().ok().and_then(|k| inner.get(&k)) {
                                self.union(*node, *n);
                            }
                        }
                    }
                }
                "tunnel" => {
                    let name = catalog::ps(&c.params, "name", "");
                    match tunnels.get(name) {
                        Some(&n) => self.union(n, pn[0].1),
                        None => {
                            tunnels.insert(name, pn[0].1);
                        }
                    }
                }
                "pin_in" | "pin_out" => {
                    iface.insert(c.id, pn[0].1);
                }
                _ => {}
            }
            local.insert(c.id, self.flat.len());
            self.flat.push(Fc { path: pidx, comp: c, pins: pn, root: path.is_empty() });
        }
        for w in &m.wires {
            let mut ends = [NONE; 2];
            for (k, r) in [&w.a, &w.b].into_iter().enumerate() {
                if let Some(&fi) = local.get(&r.c) {
                    if let Some((_, n)) = self.flat[fi].pins.iter().find(|(pd, _)| pd.name == r.p) {
                        ends[k] = *n;
                    }
                }
            }
            if ends[0] == NONE || ends[1] == NONE {
                self.diag("warn", "dangling_wire", &path, w.a.c, format!("wire {}", w.id));
            } else {
                self.union(ends[0], ends[1]);
            }
        }
        iface
    }
}

struct Tmp {
    op: Op,
    width: u8,
    param: u64,
    ins: Vec<u32>,
    outs: Vec<u32>,
    state_init: u64,
    mem: Option<Vec<u64>>,
}

fn op_of(kind: &str, root: bool) -> Option<Op> {
    Some(match kind {
        "buf" => Op::Buf,
        "not" => Op::Not,
        "and" => Op::And,
        "nand" => Op::Nand,
        "or" => Op::Or,
        "nor" => Op::Nor,
        "xor" => Op::Xor,
        "xnor" => Op::Xnor,
        "tri" => Op::Tri,
        "const" => Op::Const,
        "mux" => Op::Mux,
        "demux" => Op::Demux,
        "decoder" => Op::Decoder,
        "splitter" => Op::Split,
        "merger" => Op::Merge,
        "slice" => Op::Slice,
        "adder" => Op::Adder,
        "cmp" => Op::Cmp,
        "latch_sr" => Op::LatchSr,
        "latch_d" => Op::LatchD,
        "dff" => Op::Dff,
        "tff" => Op::Tff,
        "jkff" => Op::Jkff,
        "register" => Op::Register,
        "counter" => Op::Counter,
        "shiftreg" => Op::ShiftReg,
        "rom" => Op::Rom,
        "ram" => Op::Ram,
        "pixels" => Op::Pixels,
        "terminal" => Op::Terminal,
        "switch" | "button" | "number_in" | "port_in" => Op::Input,
        "pin_in" if root => Op::Input,
        "clock" => Op::Clock,
        _ => return None,
    })
}

pub fn compile(p: &Project) -> Netlist {
    let mut cx = Cx { p, uf: vec![], flat: vec![], diags: vec![], paths: vec![] };
    let mut stack = vec![p.main.as_str()];
    cx.inst(&p.main, String::new(), &mut stack);

    // Nodes -> dense net ids; widths.
    let nodes = cx.uf.len();
    let mut net_of = vec![NONE; nodes];
    let mut net_width: Vec<u8> = vec![];
    for n in 0..nodes as u32 {
        let r = cx.find(n) as usize;
        if net_of[r] == NONE {
            net_of[r] = net_width.len() as u32;
            net_width.push(0);
        }
        net_of[n as usize] = net_of[r];
    }
    let mut bad = vec![false; net_width.len()];
    let mut diags = std::mem::take(&mut cx.diags);
    let paths = std::mem::take(&mut cx.paths);
    let flat = std::mem::take(&mut cx.flat);
    for fc in &flat {
        for (pd, node) in &fc.pins {
            let n = net_of[*node as usize] as usize;
            if pd.width == 0 {
                continue;
            }
            if net_width[n] == 0 {
                net_width[n] = pd.width;
            } else if net_width[n] != pd.width && !bad[n] {
                bad[n] = true;
                if diags.len() < 200 {
                    diags.push(Diag {
                        level: "error",
                        code: "width_mismatch",
                        path: paths[fc.path as usize].clone(),
                        comp: fc.comp.id,
                        detail: format!("{} vs {}", net_width[n], pd.width),
                    });
                }
            }
        }
    }
    for w in net_width.iter_mut() {
        if *w == 0 {
            *w = 1;
        }
    }

    // Gates, pulls, ports, stats.
    let mut tmp: Vec<Tmp> = Vec::with_capacity(flat.len());
    let mut in_defaults: Vec<Vec<(Option<u8>, u8)>> = Vec::with_capacity(flat.len());
    let mut fc_gate = vec![NONE; flat.len()];
    let mut pull = vec![0u8; net_width.len()];
    let mut nl = Netlist::default();
    for (fi, fc) in flat.iter().enumerate() {
        let c = fc.comp;
        let kind = c.kind.as_str();
        *nl.kinds.entry(kind.to_string()).or_insert(0) += 1;
        if catalog::PART_KINDS.contains(&kind) {
            nl.parts += 1;
        }
        let net = |k: usize| net_of[fc.pins[k].1 as usize];
        match kind {
            "pull" => {
                pull[net(0) as usize] = if catalog::pb(&c.params, "up", true) { 1 } else { 2 };
                continue;
            }
            "port_out" if fc.root => {
                nl.ports_out.insert(catalog::ps(&c.params, "name", "").to_string(), net(0));
                continue;
            }
            _ => {}
        }
        let Some(op) = op_of(kind, fc.root) else { continue };
        let w = catalog::width(&c.params);
        let mut t = Tmp { op, width: w, param: 0, ins: vec![], outs: vec![], state_init: 0, mem: None };
        let mut defs = vec![];
        for (pd, node) in &fc.pins {
            match pd.dir {
                Dir::In => {
                    t.ins.push(net_of[*node as usize]);
                    defs.push((pd.default, pd.width));
                }
                Dir::Out => t.outs.push(net_of[*node as usize]),
                Dir::Passive => {}
            }
        }
        match kind {
            "const" => t.param = catalog::pu(&c.params, "value", 1),
            "mux" | "demux" => t.param = catalog::pu(&c.params, "sel", 1).clamp(1, 3),
            "slice" => t.param = catalog::pu(&c.params, "lo", 0).min(63),
            "decoder" => t.width = catalog::pu(&c.params, "bits", 2).clamp(1, 4) as u8,
            "splitter" | "merger" => t.width = catalog::parts(&c.params).iter().sum(),
            "counter" => t.param = catalog::pu(&c.params, "mod", 0),
            "clock" => t.param = catalog::pu(&c.params, "half", 4).max(1),
            "switch" => {
                t.width = 1;
                t.state_init = catalog::pb(&c.params, "on", false) as u64;
            }
            "button" => t.width = 1,
            "number_in" => t.state_init = catalog::pu(&c.params, "value", 0),
            "port_in" => {
                nl.ports_in.insert(catalog::ps(&c.params, "name", "").to_string(), tmp.len() as u32);
            }
            "rom" | "ram" => {
                let n = 1usize << catalog::pu(&c.params, "addrBits", 4).clamp(1, 16);
                let mut d = catalog::mem_data(&c.params);
                d.resize(n, 0);
                t.mem = Some(d);
            }
            "pixels" | "terminal" => t.mem = Some(vec![0; 1024]),
            _ => {}
        }
        fc_gate[fi] = tmp.len() as u32;
        tmp.push(t);
        in_defaults.push(defs);
    }

    // Drivers per net; resolve multi-driven or pulled nets through private nets.
    let nets0 = net_width.len();
    let mut drivers: Vec<Vec<(u32, u16)>> = vec![vec![]; nets0];
    for (g, t) in tmp.iter().enumerate() {
        for (k, n) in t.outs.iter().enumerate() {
            drivers[*n as usize].push((g as u32, k as u16));
        }
    }
    for n in 0..nets0 {
        if drivers[n].len() <= 1 && pull[n] == 0 {
            continue;
        }
        if drivers[n].len() > 1 && drivers[n].iter().any(|(g, _)| tmp[*g as usize].op != Op::Tri) && diags.len() < 200 {
            diags.push(Diag { level: "warn", code: "multi_driver", path: String::new(), comp: 0, detail: format!("net {n}") });
        }
        let mut ins = vec![];
        for &(g, k) in &drivers[n] {
            let private = net_width.len() as u32;
            net_width.push(net_width[n]);
            tmp[g as usize].outs[k as usize] = private;
            ins.push(private);
        }
        tmp.push(Tmp { op: Op::Resolve, width: net_width[n], param: pull[n] as u64, ins, outs: vec![n as u32], state_init: 0, mem: None });
        in_defaults.push(vec![]);
    }
    let driven = |n: usize, drivers: &Vec<Vec<(u32, u16)>>| n >= nets0 || !drivers[n].is_empty() || pull[n] != 0;

    // Unconnected control pins fall back to their default constant.
    let mut consts: BTreeMap<(u8, u8), u32> = BTreeMap::new();
    let user_gates = in_defaults.len();
    for g in 0..user_gates {
        for k in 0..tmp[g].ins.len() {
            let n = tmp[g].ins[k] as usize;
            let Some(&(Some(d), w)) = in_defaults[g].get(k) else { continue };
            if driven(n, &drivers) {
                continue;
            }
            let cn = *consts.entry((d, w)).or_insert_with(|| {
                net_width.push(w);
                net_width.len() as u32 - 1
            });
            tmp[g].ins[k] = cn;
        }
    }
    for ((d, w), n) in &consts {
        tmp.push(Tmp { op: Op::Const, width: *w, param: *d as u64, ins: vec![], outs: vec![*n], state_init: 0, mem: None });
    }
    let nnets = net_width.len();
    let mut has_driver = vec![false; nnets];
    for t in &tmp {
        for n in &t.outs {
            has_driver[*n as usize] = true;
        }
    }
    nl.floating = (0..nnets as u32).filter(|n| !has_driver[*n as usize]).collect();

    // Combinational fanout (pre-sort indices) for SCC + levels.
    let ng = tmp.len();
    let (fan_start, fan) = csr(nnets, tmp.iter().enumerate().flat_map(|(g, t)| {
        let op = t.op;
        t.ins.iter().enumerate().filter(move |(k, _)| op.comb_input(*k)).map(move |(_, n)| (*n, g as u32))
    }));
    let succ = |n: u32| &fan[fan_start[n as usize] as usize..fan_start[n as usize + 1] as usize];

    // Iterative Tarjan. SCC ids come out in reverse topological order.
    let mut idx = vec![NONE; ng];
    let mut low = vec![0u32; ng];
    let mut on = vec![false; ng];
    let mut scc = vec![NONE; ng];
    let mut scc_size: Vec<u32> = vec![];
    let mut st: Vec<u32> = vec![];
    let mut frames: Vec<(u32, u32, u32)> = vec![];
    let mut counter = 0u32;
    for s in 0..ng as u32 {
        if idx[s as usize] != NONE {
            continue;
        }
        idx[s as usize] = counter;
        low[s as usize] = counter;
        counter += 1;
        st.push(s);
        on[s as usize] = true;
        frames.push((s, 0, 0));
        while let Some(&(g, mut oi, mut fi)) = frames.last() {
            let gu = g as usize;
            let mut pushed = false;
            while (oi as usize) < tmp[gu].outs.len() {
                let fl = succ(tmp[gu].outs[oi as usize]);
                if (fi as usize) < fl.len() {
                    let h = fl[fi as usize];
                    fi += 1;
                    let hu = h as usize;
                    if idx[hu] == NONE {
                        idx[hu] = counter;
                        low[hu] = counter;
                        counter += 1;
                        st.push(h);
                        on[hu] = true;
                        *frames.last_mut().unwrap() = (g, oi, fi);
                        frames.push((h, 0, 0));
                        pushed = true;
                        break;
                    } else if on[hu] {
                        low[gu] = low[gu].min(idx[hu]);
                    }
                } else {
                    oi += 1;
                    fi = 0;
                }
            }
            if pushed {
                continue;
            }
            frames.pop();
            if let Some(&(par, _, _)) = frames.last() {
                low[par as usize] = low[par as usize].min(low[gu]);
            }
            if low[gu] == idx[gu] {
                let sid = scc_size.len() as u32;
                let mut size = 0;
                loop {
                    let h = st.pop().unwrap();
                    on[h as usize] = false;
                    scc[h as usize] = sid;
                    size += 1;
                    if h == g {
                        break;
                    }
                }
                scc_size.push(size);
            }
        }
    }
    let nscc = scc_size.len();
    let mut by_scc: Vec<u32> = (0..ng as u32).collect();
    by_scc.sort_by_key(|g| std::cmp::Reverse(scc[*g as usize]));
    let mut scc_level = vec![0u32; nscc];
    let mut self_loop = vec![false; ng];
    for &g in &by_scc {
        let gu = g as usize;
        let lv = scc_level[scc[gu] as usize];
        for o in &tmp[gu].outs {
            for &h in succ(*o) {
                let hs = scc[h as usize] as usize;
                if hs != scc[gu] as usize {
                    scc_level[hs] = scc_level[hs].max(lv + 1);
                } else if h == g {
                    self_loop[gu] = true;
                }
            }
        }
    }

    // Final order: by level (stable).
    let level_of = |g: usize| scc_level[scc[g] as usize];
    let mut order: Vec<u32> = (0..ng as u32).collect();
    order.sort_by_key(|g| level_of(*g as usize));
    let mut new_idx = vec![0u32; ng];
    for (k, g) in order.iter().enumerate() {
        new_idx[*g as usize] = k as u32;
    }
    nl.levels = order.last().map(|g| level_of(*g as usize) + 1).unwrap_or(1);
    nl.level_limit = vec![64; nl.levels as usize];
    for &g in &order {
        let gu = g as usize;
        let t = &mut tmp[gu];
        let in_scc = scc_size[scc[gu] as usize] > 1 || self_loop[gu];
        let lv = level_of(gu);
        if in_scc {
            nl.level_limit[lv as usize] += 2;
        }
        let state = if t.op.has_state() {
            nl.state_init.extend([t.state_init, 0]);
            nl.state_init.len() as u32 - 2
        } else {
            NONE
        };
        let mem = match t.mem.take() {
            Some(m) => {
                nl.mem_init.push(m);
                nl.mem_init.len() as u32 - 1
            }
            None => NONE,
        };
        if t.op == Op::Clock {
            nl.clocks.push(nl.gates.len() as u32);
        }
        nl.gates.push(Gate {
            op: t.op,
            width: t.width,
            scc: in_scc,
            in_len: t.ins.len() as u16,
            out_len: t.outs.len() as u16,
            in_start: nl.ins.len() as u32,
            out_start: nl.outs.len() as u32,
            state,
            mem,
            param: t.param,
        });
        nl.gate_level.push(lv);
        nl.ins.extend(&t.ins);
        nl.outs.extend(&t.outs);
    }
    let (fs, f) = csr(nnets, nl.gates.iter().enumerate().flat_map(|(g, gate)| {
        let ins = &nl.ins[gate.in_start as usize..gate.in_start as usize + gate.in_len as usize];
        let op = gate.op;
        ins.iter().enumerate().filter(move |(k, _)| op.comb_input(*k)).map(move |(_, n)| (*n, g as u32))
    }));
    let (cs, cf) = csr(nnets, nl.gates.iter().enumerate().filter_map(|(g, gate)| {
        gate.op.clk_index().map(|k| (nl.ins[gate.in_start as usize + k], g as u32))
    }));
    (nl.fan_start, nl.fan, nl.clk_start, nl.clk_fan) = (fs, f, cs, cf);

    // Debug info, sorted by (path, id).
    let mut dbg: Vec<usize> = (0..flat.len()).collect();
    dbg.sort_by(|a, b| (paths[flat[*a].path as usize].as_str(), flat[*a].comp.id).cmp(&(paths[flat[*b].path as usize].as_str(), flat[*b].comp.id)));
    for fi in dbg {
        let fc = &flat[fi];
        let pin_start = nl.debug_nets.len() as u32;
        nl.debug_nets.extend(fc.pins.iter().map(|(_, n)| net_of[*n as usize]));
        let gate = if fc_gate[fi] == NONE { NONE } else { new_idx[fc_gate[fi] as usize] };
        nl.debug.push(DebugComp { path: fc.path, id: fc.comp.id, gate, pin_start, pin_len: fc.pins.len() as u32 });
    }
    for g in nl.ports_in.values_mut() {
        *g = new_idx[*g as usize];
    }
    nl.net_width = net_width;
    nl.paths = paths;
    nl.diags = diags;
    nl
}

/// Builds a CSR adjacency (start offsets, items) from (bucket, item) pairs.
fn csr(buckets: usize, pairs: impl Iterator<Item = (u32, u32)> + Clone) -> (Vec<u32>, Vec<u32>) {
    let mut start = vec![0u32; buckets + 1];
    for (b, _) in pairs.clone() {
        start[b as usize + 1] += 1;
    }
    for k in 0..buckets {
        start[k + 1] += start[k];
    }
    let mut pos = start.clone();
    let mut items = vec![0u32; start[buckets] as usize];
    for (b, it) in pairs {
        items[pos[b as usize] as usize] = it;
        pos[b as usize] += 1;
    }
    (start, items)
}
