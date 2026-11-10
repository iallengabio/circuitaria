//! Programmatic circuit construction for tests, examples and reference
//! solutions, with a simple left-to-right auto layout.

use crate::{catalog, Component, Id, Module, Params, PinRef, Wire};
use serde_json::Value;
use std::collections::HashMap;

pub type Pin<'a> = (Id, &'a str);

pub struct MB {
    pub m: Module,
    next: Id,
    /// When set, new components are stacked in a column from this point.
    flow: Option<(i32, i32)>,
}

impl MB {
    pub fn new(name: &str) -> Self {
        MB {
            m: Module {
                name: name.into(),
                ..Default::default()
            },
            next: 1,
            flow: None,
        }
    }

    /// Continue building on top of an existing module (e.g. a challenge board).
    pub fn from_module(m: Module) -> Self {
        let next = m
            .components
            .iter()
            .map(|c| c.id)
            .chain(m.wires.iter().map(|w| w.id))
            .max()
            .unwrap_or(0)
            + 1;
        MB {
            m,
            next,
            flow: None,
        }
    }

    fn id(&mut self) -> Id {
        self.next += 1;
        self.next - 1
    }

    pub fn add(&mut self, kind: &str, params: Value) -> Id {
        let id = self.id();
        let params: Params = match params {
            Value::Object(o) => o.into_iter().collect(),
            _ => Params::new(),
        };
        let (mut x, mut y) = (0, 0);
        if let Some((fx, fy)) = self.flow {
            (x, y) = (fx, fy);
            let rows = catalog::pins(kind, &params)
                .map(|p| {
                    p.iter()
                        .filter(|d| d.dir == catalog::Dir::In)
                        .count()
                        .max(p.iter().filter(|d| d.dir == catalog::Dir::Out).count())
                })
                .unwrap_or(3);
            self.flow = Some((fx, fy + (rows as i32 * 20).max(40) + 20));
        }
        self.m.components.push(Component {
            id,
            kind: kind.into(),
            x,
            y,
            rot: 0,
            params,
        });
        id
    }

    /// Starts a manual column: following components stack downwards from (x, y).
    pub fn col(&mut self, x: i32, y: i32) {
        self.flow = Some((x, y));
    }

    pub fn wire(&mut self, a: Pin, b: Pin) {
        let id = self.id();
        self.m.wires.push(Wire {
            id,
            a: PinRef {
                c: a.0,
                p: a.1.into(),
            },
            b: PinRef {
                c: b.0,
                p: b.1.into(),
            },
        });
    }

    /// N-ary 1-bit gate with its inputs wired; returns the output pin.
    pub fn gate(&mut self, kind: &str, ins: &[Pin]) -> Pin<'static> {
        self.gate_w(kind, 1, ins)
    }

    pub fn gate_w(&mut self, kind: &str, width: u8, ins: &[Pin]) -> Pin<'static> {
        let unary = kind == "not" || kind == "buf";
        let g = self.add(
            kind,
            serde_json::json!({"inputs": ins.len().max(2), "width": width}),
        );
        for (k, p) in ins.iter().enumerate() {
            let name = if unary {
                "a".to_string()
            } else {
                format!("i{k}")
            };
            self.wire(*p, (g, &name));
        }
        (g, "y")
    }

    /// Instance of a user module; `module_id` is its key in the project.
    pub fn ic(&mut self, module_id: &str) -> Id {
        self.add("ic", serde_json::json!({ "module": module_id }))
    }

    /// Id of the component with the given `name` param (ports and pins).
    pub fn named(&self, name: &str) -> Id {
        self.m
            .components
            .iter()
            .find(|c| catalog::ps(&c.params, "name", "") == name)
            .map(|c| c.id)
            .unwrap_or_else(|| panic!("no component named {name}"))
    }

    pub fn finish(mut self) -> Module {
        auto_layout(&mut self.m);
        self.m
    }

    /// Skips layout (for huge generated netlists).
    pub fn finish_raw(self) -> Module {
        self.m
    }
}

/// Name of the IC pin bound to an inner `pin_in`/`pin_out` component.
pub fn ic_pin(inner: Id) -> String {
    format!("p{inner}")
}

const SOURCES: &[&str] = &[
    "switch",
    "button",
    "number_in",
    "clock",
    "pin_in",
    "port_in",
];
const SINKS: &[&str] = &[
    "led",
    "seg7",
    "number_out",
    "pin_out",
    "port_out",
    "buzzer",
    "pixels",
    "terminal",
];

/// Columns by signal depth (feedback edges ignored), rows stacked per column.
/// Components that already have a position (x or y non-zero) are kept.
pub fn auto_layout(m: &mut Module) {
    let n = m.components.len();
    let index: HashMap<Id, usize> = m
        .components
        .iter()
        .enumerate()
        .map(|(k, c)| (c.id, k))
        .collect();
    let mut succ: Vec<Vec<usize>> = vec![vec![]; n];
    for w in &m.wires {
        if let (Some(&a), Some(&b)) = (index.get(&w.a.c), index.get(&w.b.c)) {
            if a != b {
                succ[a].push(b);
            }
        }
    }
    // Iterative DFS post-order; edges into the active stack are feedback and skipped.
    let mut state = vec![0u8; n];
    let mut order = Vec::with_capacity(n);
    let mut dag: Vec<Vec<usize>> = vec![vec![]; n];
    for s in 0..n {
        if state[s] != 0 {
            continue;
        }
        let mut stack = vec![(s, 0usize)];
        state[s] = 1;
        while let Some(&(g, k)) = stack.last() {
            if k < succ[g].len() {
                stack.last_mut().unwrap().1 += 1;
                let h = succ[g][k];
                if state[h] == 0 {
                    state[h] = 1;
                    dag[g].push(h);
                    stack.push((h, 0));
                } else if state[h] == 2 {
                    dag[g].push(h);
                }
            } else {
                state[g] = 2;
                order.push(g);
                stack.pop();
            }
        }
    }
    let mut depth = vec![0usize; n];
    for &g in order.iter().rev() {
        if SOURCES.contains(&m.components[g].kind.as_str()) {
            depth[g] = 0;
        } else if depth[g] == 0 {
            depth[g] = 1;
        }
        for &h in &dag[g] {
            depth[h] = depth[h].max(depth[g] + 1);
        }
    }
    let last = depth.iter().copied().max().unwrap_or(0).max(1);
    for g in 0..n {
        if SINKS.contains(&m.components[g].kind.as_str()) {
            depth[g] = depth[g].max(last);
        }
    }
    let mut col_y: HashMap<usize, i32> = HashMap::new();
    for g in 0..n {
        let c = &mut m.components[g];
        if c.x != 0 || c.y != 0 {
            continue;
        }
        let rows = catalog::pins(&c.kind, &c.params)
            .map(|p| {
                let ins = p.iter().filter(|d| d.dir == catalog::Dir::In).count();
                ins.max(p.len() - ins).max(1)
            })
            .unwrap_or(3) as i32;
        let h = (rows * 20).max(40);
        let y = col_y.entry(depth[g]).or_insert(40);
        c.x = 60 + depth[g] as i32 * 150;
        c.y = *y;
        *y += h + 30;
    }
}
