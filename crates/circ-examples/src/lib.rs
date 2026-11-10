//! Reference projects (SPEC §16), built programmatically so they stay in sync
//! with the model. `circ-cli examples` writes them to `content/examples/`.

use circ_model::builder::{ic_pin, MB};
use circ_model::{Id, Project};
use serde_json::json;

pub struct Example {
    pub id: &'static str,
    pub title_pt: &'static str,
    pub title_en: &'static str,
    pub project: Project,
}

pub fn all() -> Vec<Example> {
    let e = |id, title_pt, title_en, project| Example {
        id,
        title_pt,
        title_en,
        project,
    };
    vec![
        e(
            "latch-sr",
            "Latch SR com NANDs",
            "SR latch from NANDs",
            sr_latch(),
        ),
        e(
            "contador-ripple",
            "Contador ripple de 4 bits",
            "4-bit ripple counter",
            ripple_counter(),
        ),
        e(
            "somador-8-bits",
            "Somador de 8 bits (CIs aninhados)",
            "8-bit adder (nested ICs)",
            adder8().0,
        ),
        e(
            "serializador",
            "Serializador / deserializador",
            "Serializer / deserializer",
            serializer(),
        ),
        e(
            "tela-de-pixels",
            "Desenhando na tela de pixels",
            "Drawing on the pixel display",
            pixel_demo(),
        ),
        e(
            "cpu-8-bits",
            "CPU de 8 bits (estilo SAP-1)",
            "8-bit CPU (SAP-1 style)",
            cpu8().0,
        ),
    ]
}

fn project(name: &str, main: MB) -> Project {
    let mut p = Project::new(name);
    p.modules.insert("main".into(), main.finish());
    p.next_id = 1_000_000;
    p
}

pub fn sr_latch() -> Project {
    let mut b = MB::new("main");
    let (s, r) = (
        b.add("switch", json!({"on": true})),
        b.add("switch", json!({"on": true})),
    );
    let (g1, g2) = (b.add("nand", json!({})), b.add("nand", json!({})));
    let (q, nq) = (
        b.add("led", json!({"color": "green"})),
        b.add("led", json!({"color": "red"})),
    );
    b.wire((s, "y"), (g1, "i0"));
    b.wire((g2, "y"), (g1, "i1"));
    b.wire((g1, "y"), (g2, "i0"));
    b.wire((r, "y"), (g2, "i1"));
    b.wire((g1, "y"), (q, "a"));
    b.wire((g2, "y"), (nq, "a"));
    let t = b.add("text", json!({"text": "S e R são ativos em 0: desligue S para gravar 1, desligue R para gravar 0."}));
    let mut m = b.finish();
    if let Some(c) = m.components.iter_mut().find(|c| c.id == t) {
        (c.x, c.y) = (60, 200);
    }
    let mut p = Project::new("Latch SR");
    p.modules.insert("main".into(), m);
    p.next_id = 1_000_000;
    p
}

pub fn ripple_counter() -> Project {
    let mut b = MB::new("main");
    let clk = b.add("clock", json!({"half": 8}));
    let mg = b.add("merger", json!({"width": 4}));
    let mut prev = (clk, "y");
    for k in 0..4 {
        let ff = b.add("tff", json!({}));
        b.wire(prev, (ff, "clk"));
        let color = ["red", "yellow", "green", "blue"][k];
        let led = b.add("led", json!({ "color": color }));
        b.wire((ff, "q"), (led, "a"));
        b.wire((ff, "q"), (mg, &format!("b{k}")));
        prev = (ff, "nq");
    }
    let out = b.add("seg7", json!({}));
    b.wire((mg, "y"), (out, "d"));
    project("Contador ripple", b)
}

/// half adder -> full adder -> 8-bit adder. Returns (project, a, b, sum).
pub fn adder8() -> (Project, Id, Id, Id) {
    let mut p = Project::new("Somador de 8 bits");
    let mut h = MB::new("Meio-somador");
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
    let mut hm = h.finish();
    hm.color = "teal".into();
    hm.icon = "½".into();
    p.modules.insert("half".into(), hm);
    let hp = [a, b_, s, c].map(ic_pin);

    let mut f = MB::new("Somador completo");
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
    let mut fm = f.finish();
    fm.color = "purple".into();
    fm.icon = "+".into();
    p.modules.insert("full".into(), fm);
    let fp = [fa, fb, fc, fs, fco].map(ic_pin);

    let mut m = MB::new("main");
    let (a, b_) = (
        m.add("number_in", json!({"width": 8, "value": 100})),
        m.add("number_in", json!({"width": 8, "value": 23})),
    );
    let (sa, sb) = (
        m.add("splitter", json!({"width": 8})),
        m.add("splitter", json!({"width": 8})),
    );
    let mg = m.add("merger", json!({"width": 9}));
    let out = m.add("number_out", json!({"width": 9}));
    m.wire((a, "y"), (sa, "a"));
    m.wire((b_, "y"), (sb, "a"));
    let zero = m.add("const", json!({"value": 0}));
    let mut carry = (zero, "y".to_string());
    for k in 0..8 {
        let fa = m.ic("full");
        m.wire((sa, &format!("b{k}")), (fa, &fp[0]));
        m.wire((sb, &format!("b{k}")), (fa, &fp[1]));
        m.wire((carry.0, &carry.1), (fa, &fp[2]));
        m.wire((fa, &fp[3]), (mg, &format!("b{k}")));
        carry = (fa, fp[4].clone());
    }
    m.wire((carry.0, &carry.1), (mg, "b8"));
    m.wire((mg, "y"), (out, "a"));
    p.modules.insert("main".into(), m.finish());
    p.next_id = 1_000_000;
    (p, a, b_, out)
}

/// Parallel -> serial -> parallel over a single wire.
pub fn serializer() -> Project {
    let mut b = MB::new("main");
    let clk = b.add("clock", json!({"half": 4}));
    let data = b.add("number_in", json!({"width": 8, "value": 0xA5}));
    let send = b.add("button", json!({}));
    let tx = b.add("shiftreg", json!({"width": 8}));
    let rx = b.add("shiftreg", json!({"width": 8}));
    let (line, out) = (
        b.add("led", json!({"color": "yellow"})),
        b.add("number_out", json!({"width": 8, "base": "hex"})),
    );
    b.wire((clk, "y"), (tx, "clk"));
    b.wire((clk, "y"), (rx, "clk"));
    b.wire((data, "y"), (tx, "d"));
    b.wire((send, "y"), (tx, "load"));
    b.wire((tx, "sout"), (rx, "sin"));
    b.wire((tx, "sout"), (line, "a"));
    b.wire((rx, "q"), (out, "a"));
    project("Serializador", b)
}

pub fn pixel_demo() -> Project {
    let mut b = MB::new("main");
    let clk = b.add("clock", json!({"half": 1}));
    let cnt = b.add("counter", json!({"width": 10}));
    let (x, y) = (
        b.add("slice", json!({"width": 10, "lo": 0, "out": 5})),
        b.add("slice", json!({"width": 10, "lo": 5, "out": 5})),
    );
    let xor = b.gate_w("xor", 5, &[(x, "y"), (y, "y")]);
    let col = b.add("slice", json!({"width": 5, "lo": 1, "out": 4}));
    let px = b.add("pixels", json!({}));
    b.wire((clk, "y"), (cnt, "clk"));
    b.wire((cnt, "q"), (x, "a"));
    b.wire((cnt, "q"), (y, "a"));
    b.wire(xor, (col, "a"));
    b.wire((x, "y"), (px, "x"));
    b.wire((y, "y"), (px, "y"));
    b.wire((col, "y"), (px, "c"));
    b.wire((clk, "y"), (px, "clk"));
    project("Tela de pixels", b)
}

// Control word bits of the reference CPU.
const HLT: u16 = 1;
const MI: u16 = 1 << 1;
const RI: u16 = 1 << 2;
const RO: u16 = 1 << 3;
const IO: u16 = 1 << 4;
const II: u16 = 1 << 5;
const AI: u16 = 1 << 6;
const AO: u16 = 1 << 7;
const EO: u16 = 1 << 8;
const SU: u16 = 1 << 9;
const BI: u16 = 1 << 10;
const OI: u16 = 1 << 11;
const CE: u16 = 1 << 12;
const CO: u16 = 1 << 13;
const J: u16 = 1 << 14;
const FI: u16 = 1 << 15;

pub const OP_NOP: u8 = 0;
pub const OP_LDA: u8 = 1;
pub const OP_ADD: u8 = 2;
pub const OP_SUB: u8 = 3;
pub const OP_STA: u8 = 4;
pub const OP_LDI: u8 = 5;
pub const OP_JMP: u8 = 6;
pub const OP_JC: u8 = 7;
pub const OP_JZ: u8 = 8;
pub const OP_OUT: u8 = 14;
pub const OP_HLT: u8 = 15;

/// Microcode ROM: address = step(3) | opcode(4) << 3 | carry << 7 | zero << 8.
fn microcode() -> Vec<u64> {
    let mut rom = vec![0u64; 512];
    for (addr, word) in rom.iter_mut().enumerate() {
        let (step, op, cf, zf) = (
            addr & 7,
            ((addr >> 3) & 15) as u8,
            addr >> 7 & 1 == 1,
            addr >> 8 & 1 == 1,
        );
        let w: u16 = match (step, op) {
            (0, _) => CO | MI,
            (1, _) => RO | II | CE,
            (2, OP_LDA | OP_ADD | OP_SUB | OP_STA) => IO | MI,
            (3, OP_LDA) => RO | AI,
            (3, OP_ADD | OP_SUB) => RO | BI,
            (4, OP_ADD) => EO | AI | FI,
            (4, OP_SUB) => EO | AI | SU | FI,
            (3, OP_STA) => AO | RI,
            (2, OP_LDI) => IO | AI,
            (2, OP_JMP) => IO | J,
            (2, OP_JC) if cf => IO | J,
            (2, OP_JZ) if zf => IO | J,
            (2, OP_OUT) => AO | OI,
            (2, OP_HLT) => HLT,
            _ => 0,
        };
        *word = w as u64;
    }
    rom
}

/// Counts by 3 on the display until the 8-bit sum overflows, then halts.
pub fn cpu_program() -> Vec<u64> {
    let i = |op: u8, arg: u8| ((op << 4) | arg) as u64;
    let mut ram = vec![0u64; 16];
    let prog = [
        i(OP_LDA, 15),
        i(OP_ADD, 14),
        i(OP_OUT, 0),
        i(OP_STA, 15),
        i(OP_JC, 6),
        i(OP_JMP, 0),
        i(OP_HLT, 0),
    ];
    ram[..prog.len()].copy_from_slice(&prog);
    ram[14] = 3;
    ram[15] = 0;
    ram
}

/// 8-bit SAP-1 style CPU: tri-state bus, microcoded control. Returns
/// (project, output register display id, halt led id).
pub fn cpu8() -> (Project, Id, Id) {
    let mut b = MB::new("main");
    b.col(60, 60);
    let clk = b.add("clock", json!({"half": 1}));
    b.col(540, 20);
    let bus = b.add("junction", json!({}));
    b.col(60, 140);
    let rom = b.add(
        "rom",
        json!({"addrBits": 9, "width": 16, "data": microcode()}),
    );
    let ctl = b.add("splitter", json!({"width": 16}));
    b.wire((rom, "d"), (ctl, "a"));
    let c = |bit: u16| -> String { format!("b{}", bit.trailing_zeros()) };
    let nhlt = b.gate("not", &[(ctl, &c(HLT))]);
    let cclk = b.gate("and", &[(clk, "y"), nhlt]);

    // Drives `src` (8 bits) onto the bus while control bit `en` is set.
    let drive = |b: &mut MB, src: (Id, &str), en: u16| {
        let t = b.add("tri", json!({"width": 8}));
        b.wire(src, (t, "a"));
        b.wire((ctl, &c(en)), (t, "en"));
        b.wire((t, "y"), (bus, "p"));
    };
    let reg = |b: &mut MB, width: u8, d: (Id, &str), en: u16| -> Id {
        let r = b.add("register", json!({"width": width}));
        b.wire(d, (r, "d"));
        b.wire(cclk, (r, "clk"));
        b.wire((ctl, &c(en)), (r, "en"));
        r
    };
    b.col(600, 60);
    let low = b.add("slice", json!({"width": 8, "lo": 0, "out": 4}));
    b.wire((bus, "p"), (low, "a"));
    let widen = |b: &mut MB, src: (Id, &str)| -> Id {
        let s = b.add("slice", json!({"width": 4, "lo": 0, "out": 8}));
        b.wire(src, (s, "a"));
        s
    };

    b.col(760, 60);
    // Program counter (JMP loads it from the bus).
    let pc = b.add("counter", json!({"width": 4}));
    let pc_en = b.gate("or", &[(ctl, &c(CE)), (ctl, &c(J))]);
    b.wire(cclk, (pc, "clk"));
    b.wire(pc_en, (pc, "en"));
    b.wire((ctl, &c(J)), (pc, "load"));
    b.wire((low, "y"), (pc, "d"));
    let pc8 = widen(&mut b, (pc, "q"));
    drive(&mut b, (pc8, "y"), CO);

    b.col(960, 60);
    // Memory.
    let mar = reg(&mut b, 4, (low, "y"), MI);
    let ram = b.add(
        "ram",
        json!({"addrBits": 4, "width": 8, "data": cpu_program()}),
    );
    b.wire((mar, "q"), (ram, "addr"));
    b.wire((bus, "p"), (ram, "din"));
    b.wire((ctl, &c(RI)), (ram, "we"));
    b.wire(cclk, (ram, "clk"));
    drive(&mut b, (ram, "dout"), RO);

    b.col(1160, 60);
    // Instruction register: high nibble = opcode, low nibble = operand.
    let ir = reg(&mut b, 8, (bus, "p"), II);
    let (opc, arg) = (
        b.add("slice", json!({"width": 8, "lo": 4, "out": 4})),
        b.add("slice", json!({"width": 8, "lo": 0, "out": 4})),
    );
    b.wire((ir, "q"), (opc, "a"));
    b.wire((ir, "q"), (arg, "a"));
    let arg8 = widen(&mut b, (arg, "y"));
    drive(&mut b, (arg8, "y"), IO);

    b.col(760, 380);
    // Accumulator, B and the adder/subtractor.
    let a = reg(&mut b, 8, (bus, "p"), AI);
    drive(&mut b, (a, "q"), AO);
    b.col(960, 380);
    let breg = reg(&mut b, 8, (bus, "p"), BI);
    let nb = b.gate_w("not", 8, &[(breg, "q")]);
    let bsel = b.add("mux", json!({"width": 8, "sel": 1}));
    b.wire((breg, "q"), (bsel, "i0"));
    b.wire(nb, (bsel, "i1"));
    b.wire((ctl, &c(SU)), (bsel, "sel"));
    let alu = b.add("adder", json!({"width": 8}));
    b.wire((a, "q"), (alu, "a"));
    b.wire((bsel, "y"), (alu, "b"));
    b.wire((ctl, &c(SU)), (alu, "cin"));
    drive(&mut b, (alu, "s"), EO);
    b.col(1160, 440);
    let zero = b.add("const", json!({"width": 8, "value": 0}));
    let isz = b.add("cmp", json!({"width": 8}));
    b.wire((alu, "s"), (isz, "a"));
    b.wire((zero, "y"), (isz, "b"));
    let fl_in = b.add("merger", json!({"width": 2}));
    b.wire((alu, "cout"), (fl_in, "b0"));
    b.wire((isz, "eq"), (fl_in, "b1"));
    let flags = reg(&mut b, 2, (fl_in, "y"), FI);
    let fl = b.add("splitter", json!({"width": 2}));
    b.wire((flags, "q"), (fl, "a"));

    b.col(1360, 60);
    // Output register and display.
    let out = reg(&mut b, 8, (bus, "p"), OI);
    let disp = b.add("number_out", json!({"width": 8}));
    b.wire((out, "q"), (disp, "a"));
    let halt = b.add("led", json!({"color": "red"}));
    b.wire((ctl, &c(HLT)), (halt, "a"));

    b.col(300, 60);
    // Control: step counter + microcode address.
    let step = b.add("counter", json!({"width": 3, "mod": 5}));
    b.wire(cclk, (step, "clk"));
    let addr = b.add("merger", json!({"parts": "3,4,1,1"}));
    b.wire((step, "q"), (addr, "b0"));
    b.wire((opc, "y"), (addr, "b1"));
    b.wire((fl, "b0"), (addr, "b2"));
    b.wire((fl, "b1"), (addr, "b3"));
    b.wire((addr, "y"), (rom, "addr"));

    (project("CPU de 8 bits", b), disp, halt)
}

/// Layered random netlist for benchmarks: `n` 2-input gates fed by `inputs`
/// switches. Returns the project and the switch ids.
pub fn random_netlist(n: usize, inputs: usize, seed: u64) -> (Project, Vec<Id>) {
    let mut rng = seed | 1;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let mut b = MB::new("main");
    let ins: Vec<Id> = (0..inputs).map(|_| b.add("switch", json!({}))).collect();
    let mut sig: Vec<Id> = ins.clone();
    let kinds = ["and", "or", "xor", "nand", "nor", "xnor"];
    for _ in 0..n {
        let g = b.add(kinds[(next() % 6) as usize], json!({}));
        for pin in ["i0", "i1"] {
            let window = sig.len().min(4096);
            let src = sig[sig.len() - 1 - (next() as usize % window)];
            b.wire((src, "y"), (g, pin));
        }
        sig.push(g);
    }
    let mut p = Project::new("random");
    p.modules.insert("main".into(), b.finish_raw());
    (p, ins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use circ_compile::compile;
    use circ_engine::Sim;

    #[test]
    fn all_examples_compile_cleanly() {
        for e in all() {
            let nl = compile(&e.project);
            assert!(nl.diags.is_empty(), "{}: {:?}", e.id, nl.diags);
            let mut sim = Sim::new(nl);
            sim.run(200);
            let json = e.project.to_json();
            assert_eq!(Project::from_json(&json).unwrap(), e.project, "{}", e.id);
        }
    }

    #[test]
    fn adder_adds() {
        let (p, a, b, out) = adder8();
        let nl = compile(&p);
        let mut sim = Sim::new(nl);
        let net = sim.nl.pin_net(&p, "", out, "a").unwrap();
        assert_eq!(sim.get(net), Some(123));
        let (ga, gb) = (
            sim.nl.gate_of("", a).unwrap(),
            sim.nl.gate_of("", b).unwrap(),
        );
        sim.set_input(ga, 255);
        sim.set_input(gb, 255);
        sim.settle();
        assert_eq!(sim.get(net), Some(510));
    }

    #[test]
    fn cpu_runs_its_program_and_halts() {
        let (p, disp, halt) = cpu8();
        let nl = compile(&p);
        let mut sim = Sim::new(nl);
        let (out, hlt) = (
            sim.nl.pin_net(&p, "", disp, "a").unwrap(),
            sim.nl.pin_net(&p, "", halt, "a").unwrap(),
        );
        let mut seen = vec![];
        for _ in 0..20_000 {
            sim.tick();
            let v = sim.get(out).unwrap();
            if seen.last() != Some(&v) {
                seen.push(v);
            }
            if sim.get(hlt) == Some(1) {
                break;
            }
        }
        assert_eq!(sim.get(hlt), Some(1), "program halts on overflow");
        assert_eq!(&seen[..5], &[0, 3, 6, 9, 12]);
        assert_eq!(*seen.last().unwrap(), 2, "255 + 3 wraps to 2 with carry");
        assert_eq!(seen.len(), 87);
        let h = sim.hash();
        sim.run(100);
        assert_eq!(sim.hash() != h, true, "tick counter still advances");
        assert_eq!(sim.get(out), Some(2), "but the halted CPU does not");
    }
}
