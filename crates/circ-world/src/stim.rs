//! Scripted worlds: a deterministic sequence of events happens in the scene
//! and, at the end of each beat, the world checks what the circuit did.

use crate::draw::*;
use crate::{Port, Rng, Scenario, Sig, Status, World};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Lamp,
    Vault,
    Door,
    Museum,
    Council,
    Market,
    Lock,
    Turret,
    Factory,
    Traffic,
    Vending,
    Telegraph,
    Simon,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "lamp" => Kind::Lamp,
            "vault" => Kind::Vault,
            "door" => Kind::Door,
            "museum" => Kind::Museum,
            "council" => Kind::Council,
            "market" => Kind::Market,
            "lock" => Kind::Lock,
            "turret" => Kind::Turret,
            "factory" => Kind::Factory,
            "traffic" => Kind::Traffic,
            "vending" => Kind::Vending,
            "telegraph" => Kind::Telegraph,
            "simon" => Kind::Simon,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Kind::Lamp => "lamp",
            Kind::Vault => "vault",
            Kind::Door => "door",
            Kind::Museum => "museum",
            Kind::Council => "council",
            Kind::Market => "market",
            Kind::Lock => "lock",
            Kind::Turret => "turret",
            Kind::Factory => "factory",
            Kind::Traffic => "traffic",
            Kind::Vending => "vending",
            Kind::Telegraph => "telegraph",
            Kind::Simon => "simon",
        }
    }
    /// (visible, hidden) scenario counts.
    fn counts(self) -> (usize, usize) {
        match self {
            Kind::Museum => (2, 2),
            Kind::Lamp => (1, 1),
            _ => (1, 2),
        }
    }
}

const fn p(name: &'static str, width: u8) -> Port {
    Port { name, width }
}

fn ports(k: Kind) -> (Vec<Port>, Vec<Port>) {
    match k {
        Kind::Lamp => (vec![p("interruptor", 1)], vec![p("lampada", 1)]),
        Kind::Vault => (vec![p("b1", 1), p("b2", 1), p("b3", 1), p("b4", 1)], vec![p("abrir", 1)]),
        Kind::Door => (vec![p("guardaA", 1), p("guardaB", 1), p("mestre", 1)], vec![p("porta", 1)]),
        Kind::Museum => (vec![p("feixe1", 1), p("feixe2", 1), p("feixe3", 1), p("chave", 1)], vec![p("sirene", 1)]),
        Kind::Council => (vec![p("voto1", 1), p("voto2", 1), p("voto3", 1)], vec![p("aprovado", 1)]),
        Kind::Market => (vec![p("precoA", 4), p("precoB", 4)], vec![p("total", 5)]),
        Kind::Lock => (vec![p("botao", 1)], vec![p("trava", 1)]),
        Kind::Turret => (vec![p("sensor", 1)], vec![p("canhaoE", 1), p("canhaoD", 1)]),
        Kind::Factory => (vec![p("sensor", 1), p("zerar", 1)], vec![p("contagem", 4), p("parar", 1)]),
        Kind::Traffic => (vec![p("clk", 1)], vec![p("verde", 1), p("amarelo", 1), p("vermelho", 1), p("pedestre", 1)]),
        Kind::Vending => (vec![p("moeda1", 1), p("moeda2", 1)], vec![p("liberar", 1), p("troco", 1)]),
        Kind::Telegraph => (vec![p("dado", 8), p("enviar", 1), p("clk", 1)], vec![p("laser", 1)]),
        Kind::Simon => (vec![p("cor", 2), p("gravar", 1), p("tocar", 1), p("reiniciar", 1)], vec![p("saida", 2)]),
    }
}

/// One beat: apply `set`, wait `hold` world steps, then require `exp`.
#[derive(Clone, Debug, Default)]
struct Step {
    set: Vec<(usize, u64)>,
    exp: Vec<(usize, u64)>,
}

fn st(set: &[(usize, u64)], exp: &[(usize, u64)]) -> Step {
    Step { set: set.to_vec(), exp: exp.to_vec() }
}

/// Combinational beats: every vector sets all inputs and expects f(inputs).
fn comb(vectors: Vec<Vec<u64>>, f: impl Fn(&[u64]) -> Vec<u64>) -> Vec<Step> {
    vectors
        .into_iter()
        .map(|v| Step { exp: f(&v).into_iter().enumerate().collect(), set: v.into_iter().enumerate().collect() })
        .collect()
}

/// All 2^n one-bit vectors in a seeded random order.
fn shuffled(r: &mut Rng, n: usize) -> Vec<Vec<u64>> {
    let mut v: Vec<Vec<u64>> = (0..1u64 << n).map(|x| (0..n).map(|b| (x >> b) & 1).collect()).collect();
    for k in (1..v.len()).rev() {
        v.swap(k, r.below(k as u64 + 1) as usize);
    }
    v
}

fn bits(rows: &[&[u64]]) -> Vec<Vec<u64>> {
    rows.iter().map(|r| r.to_vec()).collect()
}

fn script(kind: Kind, sc: usize) -> Vec<Step> {
    let mut r = Rng::new(sc as u64 * 7919 + kind as u64 * 104_729 + 17);
    let hidden = sc >= kind.counts().0;
    let b = |x: bool| x as u64;
    match kind {
        Kind::Lamp => comb(if hidden { (0..10).map(|_| vec![r.below(2)]).collect() } else { bits(&[&[0], &[1], &[0], &[1]]) }, |i| vec![1 - i[0]]),
        Kind::Vault => comb(
            if hidden { shuffled(&mut r, 4) } else { bits(&[&[0, 0, 0, 0], &[1, 1, 1, 1], &[1, 0, 1, 0], &[1, 0, 1, 1], &[0, 0, 1, 1], &[1, 0, 1, 1]]) },
            |i| vec![b(i == [1, 0, 1, 1])],
        ),
        Kind::Door => comb(
            if hidden { shuffled(&mut r, 3) } else { bits(&[&[0, 0, 0], &[1, 0, 0], &[0, 1, 0], &[1, 1, 0], &[0, 0, 1], &[0, 0, 0]]) },
            |i| vec![(i[0] & i[1]) | i[2]],
        ),
        Kind::Museum => comb(
            match sc {
                0 => bits(&[&[1, 1, 1, 0], &[1, 0, 1, 0], &[1, 1, 1, 0], &[0, 1, 1, 0], &[1, 1, 1, 0]]),
                1 => bits(&[&[1, 1, 1, 1], &[1, 1, 0, 1], &[0, 1, 0, 1], &[1, 1, 1, 1], &[1, 1, 1, 0]]),
                _ => shuffled(&mut r, 4),
            },
            |i| vec![(1 - i[3]) & (1 - (i[0] & i[1] & i[2]))],
        ),
        Kind::Council => comb(
            if hidden { shuffled(&mut r, 3) } else { bits(&[&[0, 0, 0], &[1, 0, 0], &[1, 1, 0], &[0, 1, 1], &[1, 1, 1], &[0, 0, 1]]) },
            |i| vec![b(i[0] + i[1] + i[2] >= 2)],
        ),
        Kind::Market => comb(
            if hidden { (0..14).map(|_| vec![r.below(16), r.below(16)]).collect() } else { bits(&[&[3, 4], &[9, 9], &[0, 7], &[15, 15], &[8, 8]]) },
            |i| vec![i[0] + i[1]],
        ),
        Kind::Lock => {
            let mut s = vec![st(&[(0, 0)], &[(0, 0)])];
            let mut q = 0;
            for _ in 0..if hidden { 5 + r.below(4) } else { 3 } {
                q ^= 1;
                s.push(st(&[(0, 1)], &[(0, q)]));
                s.push(st(&[(0, 0)], &[(0, q)]));
            }
            s
        }
        Kind::Turret => {
            let mut s = vec![st(&[(0, 0)], &[(0, 0), (1, 0)])];
            for k in 0..if hidden { 7 + r.below(4) } else { 4 } {
                s.push(st(&[(0, 1)], &[(0, b(k % 2 == 0)), (1, b(k % 2 == 1))]));
                s.push(st(&[(0, 0)], &[(0, 0), (1, 0)]));
            }
            s
        }
        Kind::Factory => {
            let mut s = vec![st(&[(0, 0), (1, 0)], &[(0, 0), (1, 0)])];
            let batches: &[u64] = if hidden { &[10, 10, 4] } else { &[10, 3] };
            for &n in batches {
                for count in 1..=n {
                    let exp = [(0, count), (1, b(count == 10))];
                    s.push(st(&[(0, 1)], &exp));
                    s.push(st(&[(0, 0)], &exp));
                }
                if n == 10 {
                    s.push(st(&[(1, 1)], &[(0, 0), (1, 0)]));
                    s.push(st(&[(1, 0)], &[(0, 0), (1, 0)]));
                }
            }
            s
        }
        Kind::Traffic => {
            let lights = |ph: u64| -> Vec<(usize, u64)> {
                let (g, y, red) = (b(ph < 4), b(ph == 4), b(ph > 4));
                vec![(0, g), (1, y), (2, red), (3, red)]
            };
            let mut s = vec![st(&[(0, 0)], &lights(0))];
            let mut ph = 0;
            for _ in 0..if hidden { 20 + r.below(8) } else { 10 } {
                ph = (ph + 1) % 8;
                s.push(st(&[(0, 1)], &lights(ph)));
                s.push(st(&[(0, 0)], &lights(ph)));
            }
            s
        }
        Kind::Vending => {
            let coins: Vec<u64> = if hidden { (0..16).map(|_| 1 + r.below(2)).collect() } else { vec![1, 1, 1, 2, 2, 1, 2, 2, 1, 1] };
            let mut s = vec![st(&[(0, 0), (1, 0)], &[(0, 0), (1, 0)])];
            let mut total = 0;
            for c in coins {
                total += c;
                let exp = if total >= 3 { [(0, 1), (1, b(total == 4))] } else { [(0, 0), (1, 0)] };
                if total >= 3 {
                    total = 0;
                }
                s.push(st(&[(c as usize - 1, 1)], &exp));
                s.push(st(&[(c as usize - 1, 0)], &exp));
            }
            s
        }
        Kind::Telegraph => {
            let msg: Vec<u64> = if hidden { (0..5).map(|_| r.below(256)).collect() } else { b"OI".iter().map(|c| *c as u64).collect() };
            let mut s = vec![st(&[(0, 0), (1, 0), (2, 0)], &[])];
            for byte in msg {
                s.push(st(&[(0, byte), (1, 1)], &[]));
                s.push(st(&[(2, 1)], &[]));
                s.push(st(&[(2, 0), (1, 0)], &[(0, byte >> 7 & 1)]));
                for k in (0..7).rev() {
                    s.push(st(&[(2, 1)], &[]));
                    s.push(st(&[(2, 0)], &[(0, byte >> k & 1)]));
                }
            }
            s
        }
        Kind::Simon => {
            let rounds: Vec<Vec<u64>> = if hidden { vec![(0..8).map(|_| r.below(4)).collect(), (0..5).map(|_| r.below(4)).collect()] } else { vec![vec![2, 0, 3, 1]] };
            let mut s = vec![st(&[(0, 0), (1, 0), (2, 0), (3, 0)], &[])];
            for seq in rounds {
                s.push(st(&[(3, 1)], &[]));
                s.push(st(&[(3, 0)], &[]));
                for &x in &seq {
                    s.push(st(&[(0, x), (1, 1)], &[]));
                    s.push(st(&[(1, 0)], &[]));
                }
                s.push(st(&[(3, 1)], &[]));
                s.push(st(&[(3, 0)], &[(0, seq[0])]));
                for &x in &seq[1..] {
                    s.push(st(&[(2, 1)], &[(0, x)]));
                    s.push(st(&[(2, 0)], &[(0, x)]));
                }
            }
            s
        }
    }
}

const HOLD: u32 = 2;

pub struct Stim {
    kind: Kind,
    ins: Vec<Port>,
    outs: Vec<Port>,
    script: Vec<Step>,
    i: usize,
    t: u32,
    cur: Vec<u64>,
    last: Vec<Sig>,
    status: Status,
}

impl Stim {
    pub fn new(kind: Kind) -> Stim {
        let (ins, outs) = ports(kind);
        let mut s = Stim { kind, cur: vec![0; ins.len()], last: vec![Sig::default(); outs.len()], ins, outs, script: vec![], i: 0, t: 0, status: Status::Running };
        s.reset(0);
        s
    }
    fn on(&self, k: usize) -> bool {
        self.last[k].on()
    }
    /// Events that already happened in this scenario matching `f`.
    fn done(&self, f: impl Fn(&Step) -> bool) -> usize {
        self.script[..self.i.min(self.script.len())].iter().filter(|s| f(s)).count()
    }
}

impl World for Stim {
    fn inputs(&self) -> Vec<Port> {
        self.ins.clone()
    }
    fn outputs(&self) -> Vec<Port> {
        self.outs.clone()
    }
    fn scenarios(&self) -> Vec<Scenario> {
        let (v, h) = self.kind.counts();
        (0..v + h).map(|k| Scenario { key: format!("{}.{}", self.kind.name(), k + 1), hidden: k >= v }).collect()
    }
    fn reset(&mut self, scenario: usize) {
        self.script = script(self.kind, scenario);
        self.i = 0;
        self.t = 0;
        self.cur.fill(0);
        self.last.fill(Sig::default());
        self.status = Status::Running;
    }
    fn step(&mut self, out: &[Sig], inp: &mut [u64]) {
        self.last.copy_from_slice(out);
        if self.status == Status::Running {
            if self.t >= HOLD {
                for &(k, want) in &self.script[self.i].exp {
                    let got = out[k];
                    if !got.known || got.val != want {
                        let shown = if got.known { got.val.to_string() } else { "?".into() };
                        self.status = Status::Lost { code: "wrong_output", detail: format!("{}|{}|{}", self.outs[k].name, want, shown) };
                        break;
                    }
                }
                self.i += 1;
                self.t = 0;
                if self.i >= self.script.len() && self.status == Status::Running {
                    self.status = Status::Won;
                }
            }
            if self.status == Status::Running {
                if self.t == 0 {
                    for &(k, v) in &self.script[self.i].set {
                        self.cur[k] = v;
                    }
                }
                self.t += 1;
            }
        }
        inp.copy_from_slice(&self.cur);
    }
    fn status(&self) -> Status {
        self.status.clone()
    }
    fn progress(&self) -> u32 {
        (self.i * 100 / self.script.len().max(1)) as u32
    }
    fn draw(&self, d: &mut Draw) {
        d.bg();
        let c = &self.cur;
        match self.kind {
            Kind::Lamp => {
                d.rect(40, 70, 26, 40, 5, PANEL);
                d.rect(47, if c[0] == 1 { 76 } else { 92 }, 12, 12, 3, if c[0] == 1 { GREEN } else { DIM });
                d.line(200, 0, 200, 50, 2, DIM);
                d.lamp(200, 68, 20, ON, self.on(0));
                d.text(200, 128, 26, FG, if self.on(0) { "🙂" } else { "😴" });
            }
            Kind::Vault => {
                d.text(40, 96, 34, FG, "🏴‍☠️");
                for k in 0..4 {
                    d.lamp(96 + k * 28, 70, 10, BLUE, c[k as usize] == 1);
                    d.text(96 + k * 28, 96, 10, FG, (k + 1).to_string());
                }
                d.rect(222, 40, 70, 100, 8, PANEL);
                d.text(257, 98, 36, FG, if self.on(0) { "💰" } else { "🔒" });
            }
            Kind::Door => {
                for (k, x) in [(0, 50), (1, 110)] {
                    d.text(x, 92, 34, FG, "💂");
                    d.lamp(x, 124, 7, GREEN, c[k] == 1);
                }
                d.text(170, 96, 28, FG, "🗝️");
                d.lamp(170, 124, 7, ON, c[2] == 1);
                d.rect(230, 36, 60, 114, 4, PANEL);
                if self.on(0) {
                    d.rect(234, 40, 14, 106, 2, GREEN);
                    d.text(270, 100, 26, FG, "🚶");
                } else {
                    d.rect(234, 40, 52, 106, 2, DIM);
                }
            }
            Kind::Museum => {
                d.text(160, 34, 26, FG, "🖼️");
                for k in 0..3 {
                    let y = 62 + k as i32 * 26;
                    d.rect(28, y - 5, 10, 10, 2, RED);
                    d.rect(282, y - 5, 10, 10, 2, if c[k] == 1 { GREEN } else { DIM });
                    if c[k] == 1 {
                        d.beam(38, y, 282, y, RED);
                    } else {
                        d.beam(38, y, 150, y, RED);
                        d.text(162, y + 8, 22, FG, "🕵️");
                    }
                }
                d.text(60, 172, 14, FG, if c[3] == 1 { "🔑💂" } else { "💂" });
                d.text(260, 172, 20, FG, if self.on(0) { "🚨" } else { "🔕" });
            }
            Kind::Council => {
                for k in 0..3 {
                    d.text(70 + k * 60, 70, 30, FG, "🧑‍⚖️");
                    d.text(70 + k * 60, 110, 22, FG, if c[k as usize] == 1 { "👍" } else { "👎" });
                }
                d.text(262, 96, 40, FG, if self.on(0) { "✅" } else { "❌" });
            }
            Kind::Market => {
                d.text(50, 60, 26, FG, "🍎");
                d.text(50, 92, 16, ON, format!("${}", c[0]));
                d.text(120, 60, 26, FG, "🧀");
                d.text(120, 92, 16, ON, format!("${}", c[1]));
                d.text(85, 132, 22, FG, "🛒");
                d.rect(200, 50, 90, 60, 6, PANEL);
                let shown = if self.last[0].known { self.last[0].val.to_string() } else { "??".into() };
                d.text(245, 90, 30, GREEN, shown);
            }
            Kind::Lock => {
                d.lamp(80, 90, 18, RED, c[0] == 1);
                d.text(80, 132, 12, FG, "👆");
                d.text(220, 100, 54, FG, if self.on(0) { "🔒" } else { "🔓" });
            }
            Kind::Turret => {
                if c[0] == 1 {
                    d.text(160, 44, 30, FG, "🛸");
                }
                for (k, x) in [(0, 90), (1, 230)] {
                    d.rect(x - 10, 124, 20, 26, 4, PANEL);
                    if self.on(k) {
                        d.beam(x, 124, 160, 44, if k == 0 { BLUE } else { GREEN });
                    }
                }
                d.text(160, 172, 12, FG, format!("💥 {}", self.done(|s| s.set == [(0, 1)])));
            }
            Kind::Factory => {
                d.rect(20, 110, 280, 10, 3, if self.on(1) { RED } else { DIM });
                let passed = self.script[..self.i.min(self.script.len())].iter().rev().take_while(|s| s.set != [(1, 0)]).filter(|s| s.set == [(0, 1)]).count() as i32;
                for k in 0..passed.min(10) {
                    d.text(288 - k * 24, 104, 16, FG, "📦");
                }
                d.rect(120, 30, 80, 44, 6, PANEL);
                let shown = if self.last[0].known { self.last[0].val.to_string() } else { "??".into() };
                d.text(160, 62, 28, GREEN, shown);
                d.text(40, 172, 14, FG, if self.on(1) { "🛑" } else { "⚙️" });
            }
            Kind::Traffic => {
                d.rect(40, 24, 34, 96, 8, PANEL);
                d.lamp(57, 42, 11, RED, self.on(2));
                d.lamp(57, 72, 11, ON, self.on(1));
                d.lamp(57, 102, 11, GREEN, self.on(0));
                d.rect(100, 120, 220, 30, 0, "#2c3257");
                let x = 120 + (self.i as i32 * 9) % 190;
                d.text(if self.on(0) { x } else { 120 }, 140, 20, FG, "🚗");
                d.text(250, 80, 26, FG, if self.on(3) { "🚶" } else { "🧍" });
                d.lamp(250, 104, 5, GREEN, self.on(3));
            }
            Kind::Vending => {
                d.rect(180, 24, 90, 120, 8, PANEL);
                d.text(225, 70, 30, FG, "🥤");
                if self.on(0) {
                    d.text(225, 132, 22, FG, "🥤");
                }
                if self.on(1) {
                    d.text(150, 132, 16, FG, "🪙");
                }
                if c[0] == 1 || c[1] == 1 {
                    d.text(110, 80, 22, FG, "🪙");
                    d.text(110, 106, 14, ON, if c[0] == 1 { "1" } else { "2" });
                }
                d.text(50, 96, 30, FG, "🧒");
            }
            Kind::Telegraph => {
                d.rect(30, 70, 30, 50, 4, PANEL);
                d.rect(260, 70, 30, 50, 4, PANEL);
                if self.on(0) {
                    d.beam(60, 90, 260, 90, RED);
                }
                d.text(45, 56, 18, ON, char::from_u32(c[0] as u32).filter(|ch| ch.is_ascii_graphic()).map(String::from).unwrap_or(format!("{:02X}", c[0])));
                let got: String = self.script[..self.i.min(self.script.len())].iter().rev().take_while(|s| s.set.first() != Some(&(0, c[0]))).filter(|s| !s.exp.is_empty()).map(|s| if s.exp[0].1 == 1 { '1' } else { '0' }).collect::<Vec<_>>().into_iter().rev().collect();
                d.text(160, 140, 14, GREEN, got);
                d.text(275, 56, 18, FG, "📡");
            }
            Kind::Simon => {
                let pads = [RED, GREEN, BLUE, ON];
                for k in 0..4usize {
                    let (x, y) = (60 + (k as i32 % 2) * 50, 46 + (k as i32 / 2) * 50);
                    d.rect(x, y, 44, 44, 8, if c[1] == 1 && c[0] == k as u64 { pads[k] } else { DIM });
                    let (x2, y2) = (x + 150, y);
                    d.rect(x2, y2, 44, 44, 8, if self.last[0].known && self.last[0].val == k as u64 { pads[k] } else { DIM });
                }
                d.text(107, 172, 12, FG, "🎵");
                d.text(257, 172, 12, FG, "🤖");
            }
        }
    }
}
