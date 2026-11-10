//! Pure gate semantics over 4-state logic, shared by the optimized simulator
//! and the test oracle.
//!
//! Encoding per bit, as two planes (val, flag): (0,0)=0 (1,0)=1 (0,1)=Z (1,1)=X.
//! Logic inputs read Z as X.

use circ_compile::{Gate, Netlist, Op};

#[inline(always)]
pub fn mask(w: u8) -> u64 {
    if w >= 64 {
        !0
    } else {
        (1u64 << w) - 1
    }
}

type L = (u64, u64);

#[inline(always)]
fn is1(x: L) -> bool {
    x.1 & 1 == 0 && x.0 & 1 == 1
}
#[inline(always)]
fn is0(x: L) -> bool {
    x.1 & 1 == 0 && x.0 & 1 == 0
}
/// Z -> X on a data value.
#[inline(always)]
fn clean(x: L, m: u64) -> L {
    ((x.0 | x.1) & m, x.1 & m)
}
#[inline(always)]
fn not1(x: L) -> L {
    ((!x.0 | x.1) & 1, x.1 & 1)
}
const X1: L = (1, 1);

/// Evaluates gate `g`, writing its outputs to `out`. Level-sensitive state
/// (latches, async resets) is updated in place. Returns (outputs, contention).
#[inline]
pub fn eval_gate(nl: &Netlist, g: &Gate, val: &[u64], flag: &[u64], state: &mut [u64], mems: &[Vec<u64>], out: &mut [L]) -> (usize, bool) {
    let m = mask(g.width);
    let ins = &nl.ins[g.in_start as usize..g.in_start as usize + g.in_len as usize];
    let iv = |k: usize| -> L {
        let n = ins[k] as usize;
        (val[n], flag[n])
    };
    let s = g.state as usize;
    match g.op {
        Op::Buf => out[0] = clean(iv(0), m),
        Op::Not => {
            let (v, f) = iv(0);
            out[0] = ((!v | f) & m, f & m);
        }
        Op::And | Op::Nand => {
            let (mut zeros, mut ones) = (0u64, m);
            for k in 0..ins.len() {
                let (v, f) = iv(k);
                zeros |= !v & !f;
                ones &= v & !f;
            }
            zeros &= m;
            let unk = m & !(zeros | ones);
            out[0] = if g.op == Op::And { (ones | unk, unk) } else { (zeros | unk, unk) };
        }
        Op::Or | Op::Nor => {
            let (mut zeros, mut ones) = (m, 0u64);
            for k in 0..ins.len() {
                let (v, f) = iv(k);
                ones |= v & !f;
                zeros &= !v & !f;
            }
            ones &= m;
            let unk = m & !(zeros | ones);
            out[0] = if g.op == Op::Or { (ones | unk, unk) } else { (zeros | unk, unk) };
        }
        Op::Xor | Op::Xnor => {
            let (mut acc, mut x) = (0u64, 0u64);
            for k in 0..ins.len() {
                let (v, f) = iv(k);
                acc ^= v;
                x |= f;
            }
            if g.op == Op::Xnor {
                acc = !acc;
            }
            out[0] = ((acc | x) & m, x & m);
        }
        Op::Tri => {
            let en = iv(1);
            out[0] = if is1(en) {
                clean(iv(0), m)
            } else if is0(en) {
                (0, m)
            } else {
                (m, m)
            };
        }
        Op::Const => out[0] = (g.param & m, 0),
        Op::Resolve => {
            let (mut any1, mut any0, mut anyx) = (0u64, 0u64, 0u64);
            for k in 0..ins.len() {
                let (v, f) = iv(k);
                any1 |= v & !f;
                any0 |= !v & !f;
                anyx |= v & f;
            }
            let clash = any1 & any0 & m;
            let x = (anyx | clash) & m;
            let z = m & !(any1 | any0 | x);
            out[0] = match g.param {
                1 => ((any1 & !x) | x | z, x),
                2 => ((any1 & !x) | x, x),
                _ => ((any1 & !x) | x, x | z),
            };
            out[0].0 &= m;
            return (1, clash != 0);
        }
        Op::Mux => {
            let sel = iv(ins.len() - 1);
            let sm = mask(g.param as u8);
            out[0] = if sel.1 & sm != 0 { (m, m) } else { clean(iv((sel.0 & sm) as usize), m) };
        }
        Op::Demux => {
            let n = g.out_len as usize;
            let sel = iv(1);
            let sm = mask(g.param as u8);
            let a = clean(iv(0), m);
            for (k, o) in out[..n].iter_mut().enumerate() {
                *o = if sel.1 & sm != 0 {
                    (m, m)
                } else if k as u64 == sel.0 & sm {
                    a
                } else {
                    (0, 0)
                };
            }
            return (n, false);
        }
        Op::Decoder => {
            let n = g.out_len as usize;
            let a = iv(0);
            for (k, o) in out[..n].iter_mut().enumerate() {
                *o = if a.1 & m != 0 { X1 } else { ((k as u64 == a.0 & m) as u64, 0) };
            }
            return (n, false);
        }
        Op::Split => {
            let n = g.out_len as usize;
            let (v, f) = clean(iv(0), m);
            let mut off = 0u32;
            for k in 0..n {
                let w = nl.net_width[nl.outs[g.out_start as usize + k] as usize];
                let wm = mask(w);
                out[k] = ((v >> off) & wm, (f >> off) & wm);
                off += w as u32;
            }
            return (n, false);
        }
        Op::Merge => {
            let (mut v, mut f, mut off) = (0u64, 0u64, 0u32);
            for k in 0..ins.len() {
                let w = nl.net_width[ins[k] as usize];
                let (a, b) = clean(iv(k), mask(w));
                v |= a << off;
                f |= b << off;
                off += w as u32;
            }
            out[0] = (v & m, f & m);
        }
        Op::Slice => {
            let wm = mask(nl.net_width[nl.outs[g.out_start as usize] as usize]);
            let (v, f) = clean(iv(0), m);
            out[0] = ((v >> g.param) & wm, (f >> g.param) & wm);
        }
        Op::Adder => {
            let (a, b, c) = (iv(0), iv(1), iv(2));
            if (a.1 | b.1) & m != 0 || c.1 & 1 != 0 {
                out[0] = (m, m);
                out[1] = X1;
            } else {
                let sum = (a.0 & m) as u128 + (b.0 & m) as u128 + (c.0 & 1) as u128;
                out[0] = (sum as u64 & m, 0);
                out[1] = ((sum >> g.width) as u64 & 1, 0);
            }
            return (2, false);
        }
        Op::Cmp => {
            let (a, b) = (iv(0), iv(1));
            if (a.1 | b.1) & m != 0 {
                out[..3].fill(X1);
            } else {
                let (a, b) = (a.0 & m, b.0 & m);
                out[0] = ((a == b) as u64, 0);
                out[1] = ((a > b) as u64, 0);
                out[2] = ((a < b) as u64, 0);
            }
            return (3, false);
        }
        Op::LatchSr => {
            let (sv, rv) = (iv(0), iv(1));
            if is1(sv) && is0(rv) {
                (state[s], state[s + 1]) = (1, 0);
            } else if is0(sv) && is1(rv) {
                (state[s], state[s + 1]) = (0, 0);
            } else if !(is0(sv) && is0(rv)) {
                (state[s], state[s + 1]) = X1;
            }
            out[0] = (state[s], state[s + 1]);
            out[1] = not1(out[0]);
            return (2, false);
        }
        Op::LatchD => {
            let en = iv(1);
            if is1(en) {
                (state[s], state[s + 1]) = clean(iv(0), 1);
            } else if !is0(en) {
                (state[s], state[s + 1]) = X1;
            }
            out[0] = (state[s], state[s + 1]);
            out[1] = not1(out[0]);
            return (2, false);
        }
        Op::Dff | Op::Tff | Op::Jkff => {
            if is1(iv(ins.len() - 1)) {
                (state[s], state[s + 1]) = (0, 0);
            }
            out[0] = (state[s], state[s + 1]);
            out[1] = not1(out[0]);
            return (2, false);
        }
        Op::Register => {
            if is1(iv(3)) {
                (state[s], state[s + 1]) = (0, 0);
            }
            out[0] = (state[s] & m, state[s + 1] & m);
        }
        Op::Counter => {
            if is1(iv(2)) {
                (state[s], state[s + 1]) = (0, 0);
            }
            out[0] = (state[s] & m, state[s + 1] & m);
            let top = if g.param == 0 { m } else { g.param - 1 };
            out[1] = if state[s + 1] & m != 0 { X1 } else { ((state[s] & m == top) as u64, 0) };
            return (2, false);
        }
        Op::ShiftReg => {
            if is1(iv(3)) {
                (state[s], state[s + 1]) = (0, 0);
            }
            out[0] = (state[s] & m, state[s + 1] & m);
            let sh = g.width as u32 - 1;
            out[1] = ((state[s] >> sh) & 1, (state[s + 1] >> sh) & 1);
            return (2, false);
        }
        Op::Rom | Op::Ram => {
            let a = iv(0);
            let mem = &mems[g.mem as usize];
            out[0] = if a.1 & (mem.len() as u64 - 1) != 0 { (m, m) } else { (mem[(a.0 as usize) & (mem.len() - 1)] & m, 0) };
        }
        Op::Pixels | Op::Terminal => return (0, false),
        Op::Input => out[0] = (state[s] & m, 0),
        Op::Clock => out[0] = (state[s] & 1, 0),
    }
    (1, false)
}

/// What an edge-triggered gate will commit after sampling on a clock edge.
#[derive(Clone, Copy, Debug)]
pub enum Pending {
    State(u32, u64, u64),
    Mem(u32, u64, u64),
    Push(u32, u64),
}

/// Samples the synchronous inputs of gate `gi` from the stable state.
#[inline]
pub fn sample(nl: &Netlist, gi: u32, val: &[u64], flag: &[u64], state: &[u64], mems: &[Vec<u64>]) -> Option<Pending> {
    let g = &nl.gates[gi as usize];
    let m = mask(g.width);
    let ins = &nl.ins[g.in_start as usize..g.in_start as usize + g.in_len as usize];
    let iv = |k: usize| -> L {
        let n = ins[k] as usize;
        (val[n], flag[n])
    };
    let s = g.state as usize;
    let st = |v: u64, f: u64| Some(Pending::State(gi, v, f));
    match g.op {
        Op::Dff => {
            let d = clean(iv(0), 1);
            st(d.0, d.1)
        }
        Op::Tff => {
            let t = iv(0);
            if is1(t) {
                let q = not1((state[s], state[s + 1]));
                st(q.0, q.1)
            } else if is0(t) {
                None
            } else {
                st(1, 1)
            }
        }
        Op::Jkff => {
            let (j, k) = (iv(0), iv(1));
            if j.1 & 1 != 0 || k.1 & 1 != 0 {
                return st(1, 1);
            }
            match (j.0 & 1, k.0 & 1) {
                (0, 0) => None,
                (0, _) => st(0, 0),
                (_, 0) => st(1, 0),
                _ => {
                    let q = not1((state[s], state[s + 1]));
                    st(q.0, q.1)
                }
            }
        }
        Op::Register => {
            let en = iv(2);
            if is1(en) {
                let d = clean(iv(0), m);
                st(d.0, d.1)
            } else if is0(en) {
                None
            } else {
                st(m, m)
            }
        }
        Op::Counter => {
            let (en, load, up) = (iv(1), iv(3), iv(5));
            if is0(en) {
                return None;
            }
            if !is1(en) || load.1 & 1 != 0 {
                return st(m, m);
            }
            if is1(load) {
                let d = clean(iv(4), m);
                return st(d.0, d.1);
            }
            if state[s + 1] & m != 0 {
                return None;
            }
            if up.1 & 1 != 0 {
                return st(m, m);
            }
            let q = state[s] & m;
            let top = if g.param == 0 { m } else { g.param - 1 };
            let next = if up.0 & 1 == 1 {
                if q >= top {
                    0
                } else {
                    q + 1
                }
            } else if q == 0 {
                top
            } else {
                q - 1
            };
            st(next & m, 0)
        }
        Op::ShiftReg => {
            let (en, load) = (iv(2), iv(4));
            if is0(en) {
                return None;
            }
            if !is1(en) || load.1 & 1 != 0 {
                return st(m, m);
            }
            if is1(load) {
                let d = clean(iv(5), m);
                return st(d.0, d.1);
            }
            let sin = clean(iv(0), 1);
            st(((state[s] << 1) | sin.0) & m, ((state[s + 1] << 1) | sin.1) & m)
        }
        Op::Ram => {
            let (a, we) = (iv(0), iv(2));
            let len = mems[g.mem as usize].len() as u64;
            if is1(we) && a.1 & (len - 1) == 0 {
                Some(Pending::Mem(gi, a.0 & (len - 1), iv(1).0 & m))
            } else {
                None
            }
        }
        Op::Pixels => {
            let (x, y, c, we) = (iv(0), iv(1), iv(2), iv(3));
            if is1(we) && (x.1 | y.1) & 31 == 0 {
                Some(Pending::Mem(gi, (y.0 & 31) * 32 + (x.0 & 31), c.0 & 15))
            } else {
                None
            }
        }
        Op::Terminal => {
            let d = iv(0);
            if is1(iv(2)) && d.1 & 0xff == 0 {
                Some(Pending::Push(gi, d.0 & 0xff))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Clock level as seen by the edge detector: 0, 1 or 2 (unknown).
#[inline(always)]
pub fn clk_level(v: u64, f: u64) -> u8 {
    if f & 1 != 0 {
        2
    } else {
        (v & 1) as u8
    }
}
