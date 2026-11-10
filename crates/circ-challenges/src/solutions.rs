//! Reference solutions for every campaign challenge. CI verifies that each
//! one passes all scenarios with three stars. Not shipped in the app build.

use crate::{template, Challenge};
use circ_model::builder::{Pin, MB};
use circ_model::Project;
use serde_json::json;

pub fn solution(ch: &Challenge) -> Project {
    let mut p = template(ch);
    let mut b = MB::from_module(p.modules.remove("main").unwrap());
    build(&ch.kind, &mut b);
    p.modules.insert("main".into(), b.finish());
    p
}

fn i(b: &MB, name: &str) -> Pin<'static> {
    (b.named(name), "y")
}
fn o(b: &MB, name: &str) -> Pin<'static> {
    (b.named(name), "a")
}

fn build(kind: &str, b: &mut MB) {
    match kind {
        "lamp" => {
            let n = b.gate("not", &[i(b, "interruptor")]);
            b.wire(n, o(b, "lampada"));
        }
        "vault" => {
            let n = b.gate("not", &[i(b, "b2")]);
            let a = b.gate("and", &[i(b, "b1"), n, i(b, "b3"), i(b, "b4")]);
            b.wire(a, o(b, "abrir"));
        }
        "door" => {
            let a = b.gate("and", &[i(b, "guardaA"), i(b, "guardaB")]);
            let r = b.gate("or", &[a, i(b, "mestre")]);
            b.wire(r, o(b, "porta"));
        }
        "museum" => {
            let ok = b.gate("and", &[i(b, "feixe1"), i(b, "feixe2"), i(b, "feixe3")]);
            let s = b.gate("nor", &[ok, i(b, "chave")]);
            b.wire(s, o(b, "sirene"));
        }
        "council" => {
            let (v1, v2, v3) = (i(b, "voto1"), i(b, "voto2"), i(b, "voto3"));
            let (a, c, d) = (
                b.gate("and", &[v1, v2]),
                b.gate("and", &[v1, v3]),
                b.gate("and", &[v2, v3]),
            );
            let r = b.gate("or", &[a, c, d]);
            b.wire(r, o(b, "aprovado"));
        }
        "market" => {
            let (sa, sb) = (
                b.add("splitter", json!({"width": 4})),
                b.add("splitter", json!({"width": 4})),
            );
            let mg = b.add("merger", json!({"width": 5}));
            b.wire(i(b, "precoA"), (sa, "a"));
            b.wire(i(b, "precoB"), (sb, "a"));
            let mut carry: Option<Pin> = None;
            for k in 0..4 {
                let (pa, pb) = (format!("b{k}"), format!("b{k}"));
                let (x, y) = ((sa, pa.as_str()), (sb, pb.as_str()));
                let x1 = b.gate("xor", &[x, y]);
                let a1 = b.gate("and", &[x, y]);
                let (sum, cout) = match carry {
                    None => (x1, a1),
                    Some(c) => {
                        let sum = b.gate("xor", &[x1, c]);
                        let a2 = b.gate("and", &[x1, c]);
                        (sum, b.gate("or", &[a1, a2]))
                    }
                };
                b.wire(sum, (mg, &format!("b{k}")));
                carry = Some(cout);
            }
            b.wire(carry.unwrap(), (mg, "b4"));
            b.wire((mg, "y"), o(b, "total"));
        }
        "lock" => {
            let t = b.add("tff", json!({}));
            b.wire(i(b, "botao"), (t, "clk"));
            b.wire((t, "q"), o(b, "trava"));
        }
        "turret" => {
            let t = b.add("tff", json!({}));
            b.wire(i(b, "sensor"), (t, "clk"));
            let e = b.gate("and", &[i(b, "sensor"), (t, "q")]);
            let d = b.gate("and", &[i(b, "sensor"), (t, "nq")]);
            b.wire(e, o(b, "canhaoE"));
            b.wire(d, o(b, "canhaoD"));
        }
        "factory" => {
            let c = b.add("counter", json!({"width": 4}));
            let ten = b.add("const", json!({"width": 4, "value": 10}));
            let cmp = b.add("cmp", json!({"width": 4}));
            b.wire(i(b, "sensor"), (c, "clk"));
            b.wire(i(b, "zerar"), (c, "clr"));
            b.wire((c, "q"), (cmp, "a"));
            b.wire((ten, "y"), (cmp, "b"));
            b.wire((c, "q"), o(b, "contagem"));
            b.wire((cmp, "eq"), o(b, "parar"));
        }
        "traffic" => {
            let c = b.add("counter", json!({"width": 3}));
            let s = b.add("splitter", json!({"width": 3}));
            b.wire(i(b, "clk"), (c, "clk"));
            b.wire((c, "q"), (s, "a"));
            let green = b.gate("not", &[(s, "b2")]);
            let none = b.gate("nor", &[(s, "b0"), (s, "b1")]);
            let yellow = b.gate("and", &[(s, "b2"), none]);
            let some = b.gate("or", &[(s, "b0"), (s, "b1")]);
            let red = b.gate("and", &[(s, "b2"), some]);
            b.wire(green, o(b, "verde"));
            b.wire(yellow, o(b, "amarelo"));
            b.wire(red, o(b, "vermelho"));
            b.wire(red, o(b, "pedestre"));
        }
        "vending" => {
            let (m1, m2) = (i(b, "moeda1"), i(b, "moeda2"));
            let clk = b.gate("or", &[m1, m2]);
            let nm2 = b.gate("not", &[m2]);
            let zero = b.add("const", json!({"value": 0}));
            let ffs: Vec<_> = (0..4).map(|_| b.add("dff", json!({}))).collect();
            for f in &ffs {
                b.wire(clk, (*f, "clk"));
            }
            let (acc, coin) = (
                b.add("merger", json!({"width": 3})),
                b.add("merger", json!({"width": 3})),
            );
            b.wire((ffs[0], "q"), (acc, "b0"));
            b.wire((ffs[1], "q"), (acc, "b1"));
            b.wire((zero, "y"), (acc, "b2"));
            b.wire(nm2, (coin, "b0"));
            b.wire(m2, (coin, "b1"));
            b.wire((zero, "y"), (coin, "b2"));
            let add = b.add("adder", json!({"width": 3}));
            b.wire((acc, "y"), (add, "a"));
            b.wire((coin, "y"), (add, "b"));
            let s = b.add("splitter", json!({"width": 3}));
            b.wire((add, "s"), (s, "a"));
            let three = b.gate("and", &[(s, "b0"), (s, "b1")]);
            let reach = b.gate("or", &[(s, "b2"), three]);
            let keep = b.gate("not", &[reach]);
            let n0 = b.gate("and", &[(s, "b0"), keep]);
            let n1 = b.gate("and", &[(s, "b1"), keep]);
            b.wire(n0, (ffs[0], "d"));
            b.wire(n1, (ffs[1], "d"));
            b.wire(reach, (ffs[2], "d"));
            b.wire((s, "b2"), (ffs[3], "d"));
            b.wire((ffs[2], "q"), o(b, "liberar"));
            b.wire((ffs[3], "q"), o(b, "troco"));
        }
        "elevator" => {
            let c = b.add("cmp", json!({"width": 2}));
            b.wire(i(b, "chamada"), (c, "a"));
            b.wire(i(b, "andar"), (c, "b"));
            b.wire((c, "gt"), o(b, "subir"));
            b.wire((c, "lt"), o(b, "descer"));
        }
        "line" => {
            let l = b.gate("not", &[i(b, "sensorE")]);
            let r = b.gate("not", &[i(b, "sensorD")]);
            b.wire(l, o(b, "motorE"));
            b.wire(r, o(b, "motorD"));
        }
        "telegraph" => {
            let s = b.add("shiftreg", json!({"width": 8}));
            b.wire(i(b, "dado"), (s, "d"));
            b.wire(i(b, "enviar"), (s, "load"));
            b.wire(i(b, "clk"), (s, "clk"));
            b.wire((s, "sout"), o(b, "laser"));
        }
        "simon" => {
            let clk = b.gate("or", &[i(b, "gravar"), i(b, "tocar")]);
            let c = b.add("counter", json!({"width": 4}));
            let ram = b.add("ram", json!({"addrBits": 4, "width": 2}));
            b.wire(clk, (c, "clk"));
            b.wire(i(b, "reiniciar"), (c, "clr"));
            b.wire((c, "q"), (ram, "addr"));
            b.wire(i(b, "cor"), (ram, "din"));
            b.wire(i(b, "gravar"), (ram, "we"));
            b.wire(i(b, "gravar"), (ram, "clk"));
            b.wire((ram, "dout"), o(b, "saida"));
        }
        "maze" => {
            let route = [
                1, 1, 3, 1, 1, 3, 1, 1, 2, 1, 1, 2, 1, 1, 1, 1, 2, 1, 1, 1, 1, 3, 1, 1, 3, 1, 1, 1,
                1,
            ];
            let c = b.add("counter", json!({"width": 5}));
            let rom = b.add("rom", json!({"addrBits": 5, "width": 2, "data": route}));
            b.wire(i(b, "clk"), (c, "clk"));
            b.wire((c, "q"), (rom, "addr"));
            b.wire((rom, "d"), o(b, "cmd"));
        }
        k => panic!("no reference solution for {k}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{campaign, verify};

    #[test]
    fn every_reference_solution_earns_three_stars() {
        let c = campaign();
        assert_eq!(c.challenges.len(), 16);
        for ch in &c.challenges {
            let rep = verify(&solution(ch), ch);
            assert!(rep.ok, "{}: {:?}", ch.id, rep);
            assert_eq!(
                (rep.stars, rep.parts),
                (3, ch.par[1]),
                "{}: par[1] must be the reference part count",
                ch.id
            );
            assert!(ch
                .unlocks
                .iter()
                .all(|k| circ_model::catalog::pins(k, &Default::default()).is_some()));
            for lang in ["pt", "en"] {
                assert!(
                    ch.title.contains_key(lang)
                        && ch.story.contains_key(lang)
                        && ch.hints[lang].len() == 2,
                    "{}",
                    ch.id
                );
            }
        }
    }

    #[test]
    fn empty_boards_and_wrong_circuits_fail() {
        for ch in &campaign().challenges {
            let rep = verify(&template(ch), ch);
            assert!(
                !rep.ok && rep.stars == 0,
                "{} passes with an empty board",
                ch.id
            );
        }
        // A buffer instead of an inverter: uses a forbidden part and is wrong.
        let ch = crate::find("w1-l01-lampada").unwrap();
        let mut b = MB::from_module(template(&ch).modules.remove("main").unwrap());
        let g = b.gate("buf", &[i(&b, "interruptor")]);
        b.wire(g, o(&b, "lampada"));
        let mut p = template(&ch);
        p.modules.insert("main".into(), b.finish());
        let rep = verify(&p, &ch);
        assert_eq!(rep.illegal, ["buf"]);
        assert!(rep
            .scenarios
            .iter()
            .all(|s| !s.passed && s.code == "wrong_output"));
        // Crossed motors lose the line.
        let ch = crate::find("w4-l13-seguidor").unwrap();
        let mut b = MB::from_module(template(&ch).modules.remove("main").unwrap());
        let l = b.gate("not", &[i(&b, "sensorE")]);
        let r = b.gate("not", &[i(&b, "sensorD")]);
        b.wire(l, o(&b, "motorD"));
        b.wire(r, o(&b, "motorE"));
        let mut p = template(&ch);
        p.modules.insert("main".into(), b.finish());
        assert!(verify(&p, &ch)
            .scenarios
            .iter()
            .any(|s| s.code == "lost_line"));
    }

    #[test]
    fn verification_is_deterministic() {
        let ch = crate::find("w4-l12-elevador").unwrap();
        let (a, b) = (verify(&solution(&ch), &ch), verify(&solution(&ch), &ch));
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}
