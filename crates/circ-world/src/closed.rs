//! Closed-loop worlds: the circuit's outputs move things, and what it senses
//! depends on what it did. Integer physics only.

use crate::draw::*;
use crate::{Port, Rng, Scenario, Sig, Status, World};

const fn p(name: &'static str, width: u8) -> Port {
    Port { name, width }
}
fn lost(code: &'static str) -> Status {
    Status::Lost { code, detail: String::new() }
}
fn scenarios(kind: &str, visible: usize, hidden: usize) -> Vec<Scenario> {
    (0..visible + hidden).map(|k| Scenario { key: format!("{kind}.{}", k + 1), hidden: k >= visible }).collect()
}

// ---------- Elevator: 4 floors, motor up/down, floor sensor ----------

pub struct Elevator {
    pos: i32,
    floor: u64,
    calls: Vec<u64>,
    i: usize,
    wait: u32,
    steps: u32,
    motor: i32,
    status: Status,
}

impl Default for Elevator {
    fn default() -> Self {
        let mut e = Elevator { pos: 0, floor: 0, calls: vec![], i: 0, wait: 0, steps: 0, motor: 0, status: Status::Running };
        e.reset(0);
        e
    }
}

impl World for Elevator {
    fn inputs(&self) -> Vec<Port> {
        vec![p("andar", 2), p("chamada", 2)]
    }
    fn outputs(&self) -> Vec<Port> {
        vec![p("subir", 1), p("descer", 1)]
    }
    fn scenarios(&self) -> Vec<Scenario> {
        scenarios("elevator", 1, 2)
    }
    fn reset(&mut self, scenario: usize) {
        let mut r = Rng::new(scenario as u64 + 99);
        self.calls = if scenario == 0 {
            vec![2, 0, 3, 1]
        } else {
            let mut v = vec![];
            let mut prev = 0;
            for _ in 0..6 {
                let next = (prev + 1 + r.below(3)) % 4;
                v.push(next);
                prev = next;
            }
            v
        };
        (self.pos, self.floor, self.i, self.wait, self.steps, self.motor) = (0, 0, 0, 0, 0, 0);
        self.status = Status::Running;
    }
    fn step(&mut self, out: &[Sig], inp: &mut [u64]) {
        if self.status == Status::Running {
            self.steps += 1;
            let (up, down) = (out[0].on(), out[1].on());
            self.motor = up as i32 - down as i32;
            if up && down {
                self.status = lost("motor_short");
            } else {
                self.pos += self.motor;
                if !(0..=30).contains(&self.pos) {
                    self.pos = self.pos.clamp(0, 30);
                    self.status = lost("elevator_crash");
                }
            }
            if self.pos % 10 == 0 {
                self.floor = (self.pos / 10) as u64;
            }
            if self.status == Status::Running {
                if self.motor == 0 && self.pos == self.calls[self.i] as i32 * 10 {
                    self.wait += 1;
                    if self.wait >= 4 {
                        self.wait = 0;
                        self.i += 1;
                        if self.i == self.calls.len() {
                            self.i -= 1;
                            self.status = Status::Won;
                        }
                    }
                } else {
                    self.wait = 0;
                }
                if self.steps > 600 {
                    self.status = lost("timeout");
                }
            }
        }
        inp[0] = self.floor;
        inp[1] = self.calls[self.i];
    }
    fn status(&self) -> Status {
        self.status.clone()
    }
    fn progress(&self) -> u32 {
        (self.i * 100 / self.calls.len()) as u32
    }
    fn draw(&self, d: &mut Draw) {
        d.bg();
        d.rect(130, 10, 60, 160, 4, PANEL);
        for f in 0..4 {
            let y = 150 - f * 40;
            d.line(100, y, 220, y, 1, DIM);
            d.text(88, y - 12, 12, FG, f.to_string());
            if self.calls[self.i] as i32 == f {
                d.text(238, y - 10, 16, FG, "🔔");
            }
        }
        let y = 150 - self.pos * 4;
        d.rect(136, y - 34, 48, 34, 4, if self.wait > 0 { GREEN } else { BLUE });
        d.text(160, y - 9, 18, FG, "🧑");
        d.text(160, 22, 12, ON, ["▼", "■", "▲"][(self.motor + 1) as usize]);
    }
}

// ---------- Line follower: 2 sensors, 2 motors ----------

pub struct Line {
    progress: i32,
    /// Lateral position of the line relative to the robot (negative = left).
    off: i32,
    curves: Vec<i32>,
    steps: u32,
    motors: (bool, bool),
    status: Status,
}

const TRACK: i32 = 240;

impl Default for Line {
    fn default() -> Self {
        let mut l = Line { progress: 0, off: 0, curves: vec![], steps: 0, motors: (false, false), status: Status::Running };
        l.reset(0);
        l
    }
}

impl Line {
    fn curve(&self, progress: i32) -> i32 {
        self.curves[(progress / 20) as usize % self.curves.len()]
    }
}

impl World for Line {
    fn inputs(&self) -> Vec<Port> {
        vec![p("sensorE", 1), p("sensorD", 1)]
    }
    fn outputs(&self) -> Vec<Port> {
        vec![p("motorE", 1), p("motorD", 1)]
    }
    fn scenarios(&self) -> Vec<Scenario> {
        scenarios("line", 1, 2)
    }
    fn reset(&mut self, scenario: usize) {
        let mut r = Rng::new(scenario as u64 + 7);
        self.curves = if scenario == 0 { vec![0, 1, 1, 0, -1, -1, 0, 1, 0, -1, 1, 0] } else { (0..12).map(|_| r.below(3) as i32 - 1).collect() };
        (self.progress, self.off, self.steps, self.motors) = (0, 0, 0, (false, false));
        self.status = Status::Running;
    }
    fn step(&mut self, out: &[Sig], inp: &mut [u64]) {
        if self.status == Status::Running {
            self.steps += 1;
            let (l, r) = (out[0].on(), out[1].on());
            self.motors = (l, r);
            match (l, r) {
                (true, true) => {
                    self.progress += 2;
                    // Going straight through a curve lets the line drift away.
                    if self.steps % 3 == 0 {
                        self.off += self.curve(self.progress);
                    }
                }
                // Left wheel only: the robot turns right, so the line slides left.
                (true, false) => {
                    self.progress += 1;
                    self.off -= 1;
                }
                (false, true) => {
                    self.progress += 1;
                    self.off += 1;
                }
                _ => {}
            }
            if self.off.abs() > 3 {
                self.status = lost("lost_line");
            } else if self.progress >= TRACK {
                self.status = Status::Won;
            } else if self.steps > 700 {
                self.status = lost("timeout");
            }
        }
        inp[0] = (self.off < 0) as u64;
        inp[1] = (self.off > 0) as u64;
    }
    fn status(&self) -> Status {
        self.status.clone()
    }
    fn progress(&self) -> u32 {
        (self.progress.min(TRACK) * 100 / TRACK) as u32
    }
    fn draw(&self, d: &mut Draw) {
        d.rect(0, 0, 320, 180, 0, "#e9ecf5");
        // The line ahead, drawn relative to the robot (which stays put).
        let (mut x, mut py) = (160 + self.off * 12, 140);
        for k in 0..7 {
            let nx = x + self.curve(self.progress + k * 20) * 14;
            d.line(x, py, nx, py - 24, 6, "#22263a");
            (x, py) = (nx, py - 24);
        }
        d.line(160 + self.off * 12, 140, 160 + self.off * 12, 180, 6, "#22263a");
        d.rect(138, 128, 44, 40, 8, BLUE);
        d.lamp(148, 132, 4, RED, self.off < 0);
        d.lamp(172, 132, 4, RED, self.off > 0);
        d.rect(130, 140, 8, 22, 3, if self.motors.0 { GREEN } else { DIM });
        d.rect(182, 140, 8, 22, 3, if self.motors.1 { GREEN } else { DIM });
        d.text(160, 160, 16, FG, "🤖");
        d.rect(10, 8, 300, 6, 3, "#c5cbe0");
        d.rect(10, 8, self.progress.min(TRACK) * 300 / TRACK, 6, 3, GREEN);
        d.text(304, 30, 14, FG, "🏁");
    }
}

// ---------- Maze: a ROM-programmed robot ----------

const MAZE: [&str; 7] = ["#########", "#...#...#", "###.#.#.#", "#...#.#.#", "#.###.#.#", "#.....#G#", "#########"];
const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

pub struct Maze {
    x: i32,
    y: i32,
    dir: usize,
    steps: u32,
    moves: u32,
    status: Status,
}

impl Default for Maze {
    fn default() -> Self {
        Maze { x: 1, y: 1, dir: 0, steps: 0, moves: 0, status: Status::Running }
    }
}

fn cell(x: i32, y: i32) -> u8 {
    MAZE[y as usize].as_bytes()[x as usize]
}

impl World for Maze {
    fn inputs(&self) -> Vec<Port> {
        vec![p("clk", 1), p("parede", 1)]
    }
    fn outputs(&self) -> Vec<Port> {
        vec![p("cmd", 2)]
    }
    fn scenarios(&self) -> Vec<Scenario> {
        scenarios("maze", 1, 0)
    }
    fn reset(&mut self, _scenario: usize) {
        *self = Maze::default();
    }
    fn step(&mut self, out: &[Sig], inp: &mut [u64]) {
        let act = self.steps % 2 == 0;
        if self.status == Status::Running {
            if act {
                // 0 = wait, 1 = forward, 2 = turn left, 3 = turn right.
                match if out[0].known { out[0].val } else { 0 } {
                    1 => {
                        let (nx, ny) = (self.x + DIRS[self.dir].0, self.y + DIRS[self.dir].1);
                        if cell(nx, ny) == b'#' {
                            self.status = lost("robot_crash");
                        } else {
                            (self.x, self.y) = (nx, ny);
                        }
                    }
                    2 => self.dir = (self.dir + 3) % 4,
                    3 => self.dir = (self.dir + 1) % 4,
                    _ => {}
                }
                self.moves += 1;
                if cell(self.x, self.y) == b'G' {
                    self.status = Status::Won;
                } else if self.moves > 80 && self.status == Status::Running {
                    self.status = lost("timeout");
                }
            }
            self.steps += 1;
        }
        inp[0] = act as u64;
        inp[1] = (cell(self.x + DIRS[self.dir].0, self.y + DIRS[self.dir].1) == b'#') as u64;
    }
    fn status(&self) -> Status {
        self.status.clone()
    }
    fn progress(&self) -> u32 {
        (self.moves * 100 / 80).min(100)
    }
    fn draw(&self, d: &mut Draw) {
        d.bg();
        let (s, ox, oy) = (24, 52, 6);
        for (y, row) in MAZE.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                let (px, py) = (ox + x as i32 * s, oy + y as i32 * s);
                match ch {
                    b'#' => d.rect(px, py, s - 1, s - 1, 3, "#3b4480"),
                    b'G' => d.text(px + s / 2, py + s - 5, 16, FG, "🏁"),
                    _ => d.rect(px, py, s - 1, s - 1, 3, "#1d2344"),
                }
            }
        }
        let (px, py) = (ox + self.x * s + s / 2, oy + self.y * s + s / 2);
        d.circle(px, py, 9, BLUE);
        let (dx, dy) = DIRS[self.dir];
        d.line(px, py, px + dx * 11, py + dy * 11, 3, ON);
    }
}
