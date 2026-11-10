//! Display list on a virtual 320x180 canvas. The UI renders it; worlds never
//! touch pixels.

use serde::Serialize;

pub const BG: &str = "#151a33";
pub const PANEL: &str = "#232a4d";
pub const FG: &str = "#e8ecff";
pub const DIM: &str = "#4a5482";
pub const ON: &str = "#ffd84a";
pub const RED: &str = "#ff5470";
pub const GREEN: &str = "#3ddc97";
pub const BLUE: &str = "#4cc9f0";

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "t")]
pub enum Shape {
    #[serde(rename = "r")]
    Rect {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        r: i32,
        fill: String,
    },
    #[serde(rename = "c")]
    Circle {
        x: i32,
        y: i32,
        r: i32,
        fill: String,
        glow: bool,
    },
    #[serde(rename = "l")]
    Line {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        w: i32,
        color: String,
        glow: bool,
    },
    #[serde(rename = "t")]
    Text {
        x: i32,
        y: i32,
        size: i32,
        color: String,
        s: String,
    },
}

#[derive(Default)]
pub struct Draw(pub Vec<Shape>);

impl Draw {
    pub fn bg(&mut self) {
        self.rect(0, 0, 320, 180, 0, BG);
        self.rect(0, 150, 320, 30, 0, PANEL);
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, fill: &str) {
        self.0.push(Shape::Rect {
            x,
            y,
            w,
            h,
            r,
            fill: fill.into(),
        });
    }
    pub fn circle(&mut self, x: i32, y: i32, r: i32, fill: &str) {
        self.0.push(Shape::Circle {
            x,
            y,
            r,
            fill: fill.into(),
            glow: false,
        });
    }
    pub fn lamp(&mut self, x: i32, y: i32, r: i32, color: &str, on: bool) {
        self.0.push(Shape::Circle {
            x,
            y,
            r,
            fill: if on { color } else { DIM }.into(),
            glow: on,
        });
    }
    pub fn line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, w: i32, color: &str) {
        self.0.push(Shape::Line {
            x1,
            y1,
            x2,
            y2,
            w,
            color: color.into(),
            glow: false,
        });
    }
    pub fn beam(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: &str) {
        self.0.push(Shape::Line {
            x1,
            y1,
            x2,
            y2,
            w: 2,
            color: color.into(),
            glow: true,
        });
    }
    /// Centered text (also used for emoji sprites).
    pub fn text(&mut self, x: i32, y: i32, size: i32, color: &str, s: impl Into<String>) {
        self.0.push(Shape::Text {
            x,
            y,
            size,
            color: color.into(),
            s: s.into(),
        });
    }
}
