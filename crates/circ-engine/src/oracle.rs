//! Slow, obviously-correct reference interpreter used only to cross-check the
//! optimized scheduler: no levels, no event queues, no SCC handling — it just
//! sweeps every gate until nothing changes, then scans every clocked element.
//! Valid for designs without combinational loops.

use crate::eval::{clk_level, eval_gate, mask, sample, Pending};
use circ_compile::Netlist;

pub struct Oracle {
    pub nl: Netlist,
    pub val: Vec<u64>,
    pub flag: Vec<u64>,
    pub state: Vec<u64>,
    pub mems: Vec<Vec<u64>>,
    prev_clk: Vec<u8>,
}

impl Oracle {
    pub fn new(nl: Netlist) -> Oracle {
        let mut o = Oracle {
            val: nl.net_width.iter().map(|w| mask(*w)).collect(),
            flag: nl.net_width.iter().map(|w| mask(*w)).collect(),
            state: nl.state_init.clone(),
            mems: nl.mem_init.clone(),
            prev_clk: vec![2; nl.gates.len()],
            nl,
        };
        for &n in &o.nl.floating {
            o.val[n as usize] = 0;
            o.flag[n as usize] = mask(o.nl.net_width[n as usize]);
        }
        o.settle();
        o
    }

    pub fn settle(&mut self) {
        let mut out = vec![(0u64, 0u64); 64];
        for _ in 0..1000 {
            for _sweep in 0..100_000 {
                let mut changed = false;
                for g in self.nl.gates.iter().rev() {
                    let (n, _) = eval_gate(&self.nl, g, &self.val, &self.flag, &mut self.state, &self.mems, &mut out);
                    for k in 0..n {
                        let net = self.nl.outs[g.out_start as usize + k] as usize;
                        if (self.val[net], self.flag[net]) != out[k] {
                            (self.val[net], self.flag[net]) = out[k];
                            changed = true;
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
            let mut pending = vec![];
            for (gi, g) in self.nl.gates.iter().enumerate() {
                let Some(k) = g.op.clk_index() else { continue };
                let n = self.nl.ins[g.in_start as usize + k] as usize;
                let level = clk_level(self.val[n], self.flag[n]);
                if self.prev_clk[gi] == 0 && level == 1 {
                    pending.extend(sample(&self.nl, gi as u32, &self.val, &self.flag, &self.state, &self.mems));
                }
                self.prev_clk[gi] = level;
            }
            if pending.is_empty() {
                return;
            }
            for p in pending {
                match p {
                    Pending::State(gi, v, f) => {
                        let s = self.nl.gates[gi as usize].state as usize;
                        self.state[s] = v;
                        self.state[s + 1] = f;
                    }
                    Pending::Mem(gi, a, d) => self.mems[self.nl.gates[gi as usize].mem as usize][a as usize] = d,
                    Pending::Push(gi, d) => {
                        let g = &self.nl.gates[gi as usize];
                        let s = g.state as usize;
                        self.mems[g.mem as usize][(self.state[s] % 1024) as usize] = d;
                        self.state[s] += 1;
                    }
                }
            }
        }
    }

    pub fn tick(&mut self) {
        for &gi in &self.nl.clocks {
            let g = &self.nl.gates[gi as usize];
            let s = g.state as usize;
            self.state[s + 1] += 1;
            if self.state[s + 1] >= g.param {
                self.state[s + 1] = 0;
                self.state[s] ^= 1;
            }
        }
        self.settle();
    }

    pub fn set_input(&mut self, gate: u32, value: u64) {
        let s = self.nl.gates[gate as usize].state as usize;
        self.state[s] = value;
    }
}
