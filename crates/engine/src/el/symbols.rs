//! Installation symbols drawn with CADCraft geometry (circle, line, polyline, text).
//!
//! The shapes follow the El-beregner's simplified plan symbols: a socket is a circle
//! with a stem, a lamp is a circle with a cross, a switch is a circle with an arm.
//! Y is up, so a stem that hung down on the web canvas hangs down here too.

use cadcraft_doc::{EntityKind, HAlign, Text, VAlign};
use cadcraft_geom::{PolyVertex, Vec2};

use crate::cmd::helpers::{arc, circle, line, lwpoly, rect_with_corners, v3};

fn text_at(at: Vec2, height: f64, value: &str) -> EntityKind {
    EntityKind::Text(Text {
        insert: v3(at),
        align_pt: Some(v3(at)),
        height,
        value: value.into(),
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        style: "Standard".into(),
        halign: HAlign::Center,
        valign: VAlign::Middle,
    })
}

fn closed(verts: Vec<PolyVertex>) -> EntityKind {
    lwpoly(verts, true)
}

fn ray(angle: f64, inner: f64, outer: f64) -> EntityKind {
    line(Vec2::new(angle.cos() * inner, angle.sin() * inner), Vec2::new(angle.cos() * outer, angle.sin() * outer))
}

/// Geometry in symbol units (about 32 across), centred on the origin.
pub fn geometry(kind: &str) -> Vec<EntityKind> {
    match kind {
        "tavle" => vec![
            closed(rect_with_corners(Vec2::new(-16.0, -11.0), Vec2::new(16.0, 11.0), 2.0, (0.0, 0.0))),
            line(Vec2::new(-16.0, 0.0), Vec2::new(16.0, 0.0)),
            text_at(Vec2::new(0.0, 5.0), 11.0, "T"),
        ],
        "stik" => vec![circle(Vec2::ZERO, 7.0), line(Vec2::new(0.0, -7.0), Vec2::new(0.0, -16.0))],
        "stik2" => vec![circle(Vec2::ZERO, 8.0), text_at(Vec2::ZERO, 11.0, "2"), line(Vec2::new(0.0, -8.0), Vec2::new(0.0, -16.0))],
        "afbryder" => vec![circle(Vec2::ZERO, 6.5), line(Vec2::ZERO, Vec2::new(13.0, 6.0))],
        "korr" => vec![circle(Vec2::ZERO, 6.5), line(Vec2::ZERO, Vec2::new(11.0, 7.0)), line(Vec2::ZERO, Vec2::new(11.0, -7.0))],
        "kryds" => vec![circle(Vec2::ZERO, 7.0), line(Vec2::new(-5.0, -5.0), Vec2::new(5.0, 5.0)), line(Vec2::new(5.0, -5.0), Vec2::new(-5.0, 5.0))],
        "lampe" => vec![circle(Vec2::ZERO, 8.0), line(Vec2::new(-5.5, -5.5), Vec2::new(5.5, 5.5)), line(Vec2::new(5.5, -5.5), Vec2::new(-5.5, 5.5))],
        "udtag" => vec![circle(Vec2::ZERO, 7.0), circle(Vec2::ZERO, 1.7), line(Vec2::new(0.0, -7.0), Vec2::new(0.0, -16.0))],
        "ude" => {
            let a0 = std::f64::consts::PI * 0.15;
            let a1 = std::f64::consts::PI * 0.85;
            vec![
                circle(Vec2::ZERO, 7.0),
                line(Vec2::new(-4.5, -4.5), Vec2::new(4.5, 4.5)),
                line(Vec2::new(4.5, -4.5), Vec2::new(-4.5, 4.5)),
                arc(&cadcraft_geom::Arc::new(Vec2::new(0.0, 2.0), 12.0, a0, a1)),
            ]
        }
        "daase" => vec![
            closed(rect_with_corners(Vec2::new(-7.0, -7.0), Vec2::new(7.0, 7.0), 0.0, (0.0, 0.0))),
            line(Vec2::new(-4.0, 0.0), Vec2::new(4.0, 0.0)),
            line(Vec2::new(0.0, -4.0), Vec2::new(0.0, 4.0)),
        ],
        "kraft" => {
            let step = std::f64::consts::TAU / 3.0;
            let a0 = std::f64::consts::FRAC_PI_2;
            vec![circle(Vec2::ZERO, 7.0), ray(a0, 3.0, 13.0), ray(a0 + step, 3.0, 13.0), ray(a0 + 2.0 * step, 3.0, 13.0)]
        }
        "komfur" => vec![
            closed(rect_with_corners(Vec2::new(-11.0, -11.0), Vec2::new(11.0, 11.0), 2.0, (0.0, 0.0))),
            circle(Vec2::new(-4.0, -4.0), 2.1),
            circle(Vec2::new(4.0, -4.0), 2.1),
            circle(Vec2::new(-4.0, 4.0), 2.1),
            circle(Vec2::new(4.0, 4.0), 2.1),
        ],
        "tekst" => vec![text_at(Vec2::ZERO, 10.0, "Tekst")],
        _ => vec![circle(Vec2::ZERO, 7.0)],
    }
}
