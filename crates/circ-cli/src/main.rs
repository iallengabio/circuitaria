//! Headless runner: validate projects, verify challenges, write examples,
//! benchmark the engine. Used locally and in CI.

use circ_compile::compile;
use circ_engine::Sim;
use circ_model::Project;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: usize| args.get(k).map(String::as_str);
    let code = match arg(0) {
        Some("check") => check(arg(1).unwrap_or_else(|| usage())),
        Some("run") => run(arg(1).unwrap_or_else(|| usage()), arg(2).and_then(|s| s.parse().ok()).unwrap_or(1000)),
        Some("examples") => examples(arg(1).unwrap_or("content/examples")),
        Some("solutions") => solutions(arg(1).unwrap_or_else(|| usage())),
        Some("challenges") => challenges(),
        Some("hash") => {
            println!("{:016x}", selftest_hash());
            0
        }
        Some("bench") => bench(arg(1) == Some("--quick")),
        _ => usage(),
    };
    std::process::exit(code);
}

fn usage() -> ! {
    eprintln!("circ-cli <command>\n  check <file.circuito>      compile and report diagnostics\n  run <file.circuito> [n]    simulate n ticks, print state hash\n  examples [dir]             write reference projects\n  solutions <dir>            write challenge reference solutions\n  challenges                 verify every reference solution\n  hash                       determinism hash (must match the WASM build)\n  bench [--quick]            engine benchmarks");
    std::process::exit(2)
}

fn load(path: &str) -> Project {
    let s = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(1)
    });
    Project::from_json(&s).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(1)
    })
}

fn check(path: &str) -> i32 {
    let nl = compile(&load(path));
    for d in &nl.diags {
        println!("{}: {} at '{}' #{} ({})", d.level, d.code, d.path, d.comp, d.detail);
    }
    println!("{} nets, {} gates, {} levels, {} parts", nl.net_width.len(), nl.gates.len(), nl.levels, nl.parts);
    nl.has_errors() as i32
}

fn run(path: &str, ticks: u64) -> i32 {
    let mut sim = Sim::new(compile(&load(path)));
    sim.run(ticks);
    println!("tick {} hash {:016x}", sim.tick, sim.hash());
    0
}

fn examples(dir: &str) -> i32 {
    std::fs::create_dir_all(dir).unwrap();
    let mut index = vec![];
    for e in circ_examples::all() {
        std::fs::write(format!("{dir}/{}.circuito", e.id), e.project.to_json()).unwrap();
        index.push(serde_json::json!({"id": e.id, "title": {"pt": e.title_pt, "en": e.title_en}}));
    }
    std::fs::write(format!("{dir}/index.json"), serde_json::to_string_pretty(&index).unwrap()).unwrap();
    println!("wrote {} examples to {dir}", index.len());
    0
}

fn solutions(dir: &str) -> i32 {
    std::fs::create_dir_all(dir).unwrap();
    for ch in circ_challenges::campaign().challenges {
        std::fs::write(format!("{dir}/{}.circuito", ch.id), circ_challenges::solutions::solution(&ch).to_json()).unwrap();
    }
    0
}

fn challenges() -> i32 {
    let mut failed = 0;
    for ch in circ_challenges::campaign().challenges {
        let rep = circ_challenges::verify(&circ_challenges::solutions::solution(&ch), &ch);
        let pass = rep.scenarios.iter().filter(|s| s.passed).count();
        println!("{} {:<24} {}/{} scenarios  {} parts  {}", if rep.ok { "ok  " } else { "FAIL" }, ch.id, pass, rep.scenarios.len(), rep.parts, "★".repeat(rep.stars as usize));
        failed += !rep.ok as i32;
    }
    failed
}

/// Hash over traces of all reference projects and challenge runs. Must be
/// identical on every platform and in the WASM build.
pub fn selftest_hash() -> u64 {
    circ_challenges::selftest_hash()
}

fn bench(quick: bool) -> i32 {
    println!("| benchmark | result |\n|---|---|");
    for n in if quick { vec![100_000] } else { vec![100_000, 1_000_000] } {
        let (p, ins) = circ_examples::random_netlist(n, 64, 42);
        let t = Instant::now();
        let nl = compile(&p);
        let compile_ms = t.elapsed().as_secs_f64() * 1e3;
        let (gates, nets, levels) = (nl.gates.len(), nl.net_width.len(), nl.levels);
        let mut sim = Sim::new(nl);
        let bytes = sim.mem_bytes();
        let gs: Vec<u32> = ins.iter().map(|i| sim.nl.gate_of("", *i).unwrap()).collect();
        let mut rng = 0x1234_5678_9abc_def1u64;
        let ticks = if quick { 200 } else { 300 };
        sim.evals = 0;
        let t = Instant::now();
        for _ in 0..ticks {
            for _ in 0..3 {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                let g = gs[(rng % 64) as usize];
                sim.set_input(g, (rng >> 8) & 1);
            }
            sim.tick();
        }
        let dt = t.elapsed().as_secs_f64();
        println!("| compile {n} gates | {compile_ms:.0} ms ({levels} levels) |");
        println!("| random netlist {n} gates: throughput | {:.1} M gate evals/s |", sim.evals as f64 / dt / 1e6);
        println!("| random netlist {n} gates: activity | {:.1}% of gates per tick |", sim.evals as f64 / ticks as f64 / gates as f64 * 100.0);
        println!("| runtime memory | {:.0} B/gate ({gates} gates, {nets} nets) |", bytes as f64 / gates as f64);
    }
    let (p, _, _) = circ_examples::cpu8();
    let mut p = p;
    // Replace the program with an endless loop so the CPU never halts.
    for c in &mut p.modules.get_mut("main").unwrap().components {
        if c.kind == "ram" {
            c.params.insert("data".into(), serde_json::json!([0x2e, 0xe0, 0x60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0]));
        }
    }
    let mut sim = Sim::new(compile(&p));
    let ticks = if quick { 400_000 } else { 4_000_000 };
    let t = Instant::now();
    sim.run(ticks);
    let dt = t.elapsed().as_secs_f64();
    println!("| 8-bit CPU ({} gates): effective clock | {:.0} kHz |", sim.nl.gates.len(), ticks as f64 / 2.0 / dt / 1e3);
    println!("| determinism hash | {:016x} |", selftest_hash());
    0
}
