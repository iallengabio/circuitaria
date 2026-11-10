//! Component catalog: pin interface of every primitive kind and the palette
//! metadata consumed by the UI. This is the only place where pins are defined.

use crate::{Module, Params};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    In,
    Out,
    /// Net-merging pin without direction (junction, tunnel, pull). Width 0 = any.
    Passive,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PinDesc {
    pub name: String,
    pub dir: Dir,
    pub width: u8,
    /// Value assumed when the pin is left unconnected (control pins). None = floats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<u8>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub clock: bool,
    /// Human label for IC pins (the inner pin's name); empty otherwise.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
}

fn i(name: &str, width: u8) -> PinDesc {
    PinDesc { name: name.into(), dir: Dir::In, width, default: None, clock: false, label: String::new() }
}
fn id(name: &str, width: u8, d: u8) -> PinDesc {
    PinDesc { default: Some(d), ..i(name, width) }
}
fn ck(name: &str) -> PinDesc {
    PinDesc { clock: true, ..i(name, 1) }
}
fn o(name: &str, width: u8) -> PinDesc {
    PinDesc { dir: Dir::Out, ..i(name, width) }
}

pub fn pu(p: &Params, key: &str, def: u64) -> u64 {
    match p.get(key) {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(def),
        Some(Value::String(s)) => parse_num(s).unwrap_or(def),
        Some(Value::Bool(b)) => *b as u64,
        _ => def,
    }
}
pub fn pb(p: &Params, key: &str, def: bool) -> bool {
    match p.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0) != 0,
        _ => def,
    }
}
pub fn ps<'a>(p: &'a Params, key: &str, def: &'a str) -> &'a str {
    p.get(key).and_then(Value::as_str).unwrap_or(def)
}

/// Accepts decimal, 0x hex and 0b binary.
pub fn parse_num(s: &str) -> Option<u64> {
    let s = s.trim().replace('_', "");
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else if let Some(b) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        u64::from_str_radix(b, 2).ok()
    } else {
        s.parse().ok()
    }
}

pub fn width(p: &Params) -> u8 {
    pu(p, "width", 1).clamp(1, 64) as u8
}

/// Piece widths of a splitter/merger, LSB first. `parts: "3,4,1"` or all 1-bit.
pub fn parts(p: &Params) -> Vec<u8> {
    let s = ps(p, "parts", "");
    let v: Vec<u8> = s.split(',').filter_map(|t| t.trim().parse::<u8>().ok()).filter(|w| *w >= 1).collect();
    let total: u32 = v.iter().map(|w| *w as u32).sum();
    if !v.is_empty() && total <= 64 && v.len() <= 32 {
        v
    } else {
        vec![1; pu(p, "width", 2).clamp(2, 32) as usize]
    }
}

/// Memory contents: JSON array of numbers, or a string of hex words.
pub fn mem_data(p: &Params) -> Vec<u64> {
    match p.get("data") {
        Some(Value::Array(a)) => a.iter().map(|v| v.as_u64().unwrap_or(0)).collect(),
        Some(Value::String(s)) => s
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .map(|t| u64::from_str_radix(t.trim_start_matches("0x"), 16).unwrap_or(0))
            .collect(),
        _ => vec![],
    }
}

/// Pins of a primitive kind, in canonical order (inputs first, then outputs).
/// Returns None for unknown kinds and for `ic` (see [`module_pins`]).
pub fn pins(kind: &str, p: &Params) -> Option<Vec<PinDesc>> {
    let w = width(p);
    let passive = |n: &str| PinDesc { dir: Dir::Passive, ..i(n, 0) };
    Some(match kind {
        "buf" | "not" => vec![i("a", w), o("y", w)],
        "and" | "nand" | "or" | "nor" | "xor" | "xnor" => {
            let n = pu(p, "inputs", 2).clamp(2, 8);
            let mut v: Vec<_> = (0..n).map(|k| i(&format!("i{k}"), w)).collect();
            v.push(o("y", w));
            v
        }
        "tri" => vec![i("a", w), i("en", 1), o("y", w)],
        "const" => vec![o("y", w)],
        "pull" | "junction" | "tunnel" => vec![passive("p")],
        "mux" => {
            let s = pu(p, "sel", 1).clamp(1, 3) as u8;
            let mut v: Vec<_> = (0..1u32 << s).map(|k| i(&format!("i{k}"), w)).collect();
            v.push(i("sel", s));
            v.push(o("y", w));
            v
        }
        "demux" => {
            let s = pu(p, "sel", 1).clamp(1, 3) as u8;
            let mut v = vec![i("a", w), i("sel", s)];
            v.extend((0..1u32 << s).map(|k| o(&format!("o{k}"), w)));
            v
        }
        "decoder" => {
            let n = pu(p, "bits", 2).clamp(1, 4) as u8;
            let mut v = vec![i("a", n)];
            v.extend((0..1u32 << n).map(|k| o(&format!("o{k}"), 1)));
            v
        }
        "splitter" => {
            let ps = parts(p);
            let mut v = vec![i("a", ps.iter().sum())];
            v.extend(ps.iter().enumerate().map(|(k, w)| o(&format!("b{k}"), *w)));
            v
        }
        "merger" => {
            let ps = parts(p);
            let mut v: Vec<_> = ps.iter().enumerate().map(|(k, w)| i(&format!("b{k}"), *w)).collect();
            v.push(o("y", ps.iter().sum()));
            v
        }
        "slice" => vec![i("a", w), o("y", pu(p, "out", 1).clamp(1, 64) as u8)],
        "adder" => vec![i("a", w), i("b", w), id("cin", 1, 0), o("s", w), o("cout", 1)],
        "cmp" => vec![i("a", w), i("b", w), o("eq", 1), o("gt", 1), o("lt", 1)],
        "latch_sr" => vec![i("s", 1), i("r", 1), o("q", 1), o("nq", 1)],
        "latch_d" => vec![i("d", 1), i("en", 1), o("q", 1), o("nq", 1)],
        "dff" => vec![i("d", 1), ck("clk"), id("rst", 1, 0), o("q", 1), o("nq", 1)],
        "tff" => vec![id("t", 1, 1), ck("clk"), id("rst", 1, 0), o("q", 1), o("nq", 1)],
        "jkff" => vec![i("j", 1), i("k", 1), ck("clk"), id("rst", 1, 0), o("q", 1), o("nq", 1)],
        "register" => vec![i("d", w), ck("clk"), id("en", 1, 1), id("clr", 1, 0), o("q", w)],
        "counter" => vec![ck("clk"), id("en", 1, 1), id("clr", 1, 0), id("load", 1, 0), id("d", w, 0), id("up", 1, 1), o("q", w), o("co", 1)],
        "shiftreg" => vec![id("sin", 1, 0), ck("clk"), id("en", 1, 1), id("clr", 1, 0), id("load", 1, 0), id("d", w, 0), o("q", w), o("sout", 1)],
        "rom" => vec![i("addr", pu(p, "addrBits", 4).clamp(1, 16) as u8), o("d", w)],
        "ram" => vec![i("addr", pu(p, "addrBits", 4).clamp(1, 16) as u8), i("din", w), id("we", 1, 0), ck("clk"), o("dout", w)],
        "switch" | "button" | "clock" => vec![o("y", 1)],
        "number_in" | "pin_in" | "port_in" => vec![o("y", w)],
        "led" | "buzzer" => vec![i("a", 1)],
        "seg7" => {
            if pb(p, "raw", false) {
                ["a", "b", "c", "d", "e", "f", "g"].iter().map(|n| i(n, 1)).collect()
            } else {
                vec![i("d", 4)]
            }
        }
        "number_out" | "pin_out" | "port_out" => vec![i("a", w)],
        "pixels" => vec![i("x", 5), i("y", 5), id("c", 4, 15), id("we", 1, 1), ck("clk")],
        "terminal" => vec![i("d", 8), ck("clk"), id("en", 1, 1)],
        "text" => vec![],
        _ => return None,
    })
}

/// Interface pins of a user IC, derived from its `pin_in` / `pin_out`
/// components, ordered top-to-bottom. Pin name is `p<inner id>` (stable).
pub fn module_pins(m: &Module) -> Vec<PinDesc> {
    let mut v: Vec<(&crate::Component, bool)> =
        m.components.iter().filter(|c| c.kind == "pin_in" || c.kind == "pin_out").map(|c| (c, c.kind == "pin_in")).collect();
    v.sort_by_key(|(c, is_in)| (!*is_in, c.y, c.x, c.id));
    v.iter()
        .map(|(c, is_in)| PinDesc {
            name: format!("p{}", c.id),
            dir: if *is_in { Dir::In } else { Dir::Out },
            width: width(&c.params),
            default: None,
            clock: false,
            label: ps(&c.params, "name", "").to_string(),
        })
        .collect()
}

/// Kinds that count as "parts" for challenge scoring (logic and memory).
pub const PART_KINDS: &[&str] = &[
    "buf", "not", "and", "nand", "or", "nor", "xor", "xnor", "tri", "mux", "demux", "decoder", "adder", "cmp", "latch_sr", "latch_d", "dff",
    "tff", "jkff", "register", "counter", "shiftreg", "rom", "ram",
];

/// Palette metadata. `params[].type`: int | bool | enum | string | data.
pub fn catalog() -> Value {
    let w = |d: u64| json!({"name":"width","type":"int","default":d,"min":1,"max":64});
    let inputs = json!({"name":"inputs","type":"int","default":2,"min":2,"max":8});
    let name = |d: &str| json!({"name":"name","type":"string","default":d});
    let gate = |k: &str| json!({"kind":k,"cat":"gates","params":[inputs.clone(), w(1)]});
    json!([
        {"kind":"switch","cat":"io","params":[{"name":"on","type":"bool","default":false}]},
        {"kind":"button","cat":"io","params":[]},
        {"kind":"number_in","cat":"io","params":[w(4), {"name":"value","type":"int","default":0,"min":0}]},
        {"kind":"clock","cat":"io","params":[{"name":"half","type":"int","default":4,"min":1,"max":100000}]},
        {"kind":"const","cat":"io","params":[w(1), {"name":"value","type":"int","default":1,"min":0}]},
        {"kind":"led","cat":"io","params":[
            {"name":"color","type":"enum","default":"red","options":["red","green","blue","yellow","purple","white"]},
            {"name":"shape","type":"enum","default":"circle","options":["circle","square","star","heart","triangle"]}]},
        {"kind":"seg7","cat":"io","params":[{"name":"raw","type":"bool","default":false},
            {"name":"color","type":"enum","default":"red","options":["red","green","blue","yellow"]}]},
        {"kind":"number_out","cat":"io","params":[w(4), {"name":"base","type":"enum","default":"dec","options":["dec","hex","bin","signed"]}]},
        {"kind":"pixels","cat":"io","params":[]},
        {"kind":"terminal","cat":"io","params":[]},
        {"kind":"buzzer","cat":"io","params":[{"name":"freq","type":"int","default":440,"min":60,"max":4000}]},
        {"kind":"buf","cat":"gates","params":[w(1)]},
        {"kind":"not","cat":"gates","params":[w(1)]},
        gate("and"), gate("nand"), gate("or"), gate("nor"), gate("xor"), gate("xnor"),
        {"kind":"tri","cat":"gates","params":[w(1)]},
        {"kind":"pull","cat":"gates","params":[{"name":"up","type":"bool","default":true}]},
        {"kind":"junction","cat":"wiring","params":[]},
        {"kind":"tunnel","cat":"wiring","params":[name("A")]},
        {"kind":"splitter","cat":"wiring","params":[{"name":"width","type":"int","default":4,"min":2,"max":32}, {"name":"parts","type":"string","default":""}]},
        {"kind":"merger","cat":"wiring","params":[{"name":"width","type":"int","default":4,"min":2,"max":32}, {"name":"parts","type":"string","default":""}]},
        {"kind":"slice","cat":"wiring","params":[w(8), {"name":"lo","type":"int","default":0,"min":0,"max":63}, {"name":"out","type":"int","default":4,"min":1,"max":64}]},
        {"kind":"mux","cat":"wiring","params":[{"name":"sel","type":"int","default":1,"min":1,"max":3}, w(1)]},
        {"kind":"demux","cat":"wiring","params":[{"name":"sel","type":"int","default":1,"min":1,"max":3}, w(1)]},
        {"kind":"decoder","cat":"wiring","params":[{"name":"bits","type":"int","default":2,"min":1,"max":4}]},
        {"kind":"text","cat":"wiring","params":[{"name":"text","type":"string","default":"Nota"}]},
        {"kind":"adder","cat":"arith","params":[w(4)]},
        {"kind":"cmp","cat":"arith","params":[w(4)]},
        {"kind":"latch_sr","cat":"memory","params":[]},
        {"kind":"latch_d","cat":"memory","params":[]},
        {"kind":"dff","cat":"memory","params":[]},
        {"kind":"tff","cat":"memory","params":[]},
        {"kind":"jkff","cat":"memory","params":[]},
        {"kind":"register","cat":"memory","params":[w(8)]},
        {"kind":"counter","cat":"memory","params":[w(4), {"name":"mod","type":"int","default":0,"min":0}]},
        {"kind":"shiftreg","cat":"memory","params":[w(8)]},
        {"kind":"rom","cat":"memory","params":[{"name":"addrBits","type":"int","default":4,"min":1,"max":16}, w(8), {"name":"data","type":"data","default":""}]},
        {"kind":"ram","cat":"memory","params":[{"name":"addrBits","type":"int","default":4,"min":1,"max":16}, w(8), {"name":"data","type":"data","default":""}]},
        {"kind":"pin_in","cat":"module","params":[name("in"), w(1)]},
        {"kind":"pin_out","cat":"module","params":[name("out"), w(1)]},
        {"kind":"port_in","cat":"hidden","params":[name("in"), w(1)]},
        {"kind":"port_out","cat":"hidden","params":[name("out"), w(1)]},
        {"kind":"ic","cat":"hidden","params":[{"name":"module","type":"string","default":""}]}
    ])
}
