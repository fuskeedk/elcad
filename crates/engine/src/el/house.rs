//! A house on the current drawing, using the same wall technique as
//! [`crate::sample::floor_plan`]: a double rectangle for the outer wall, an
//! interior wall, a door swing and windows.

use cadcraft_color::Color;
use cadcraft_doc::{Common, Drawing, EntityKind, Layer, Lineweight, Space, Text};
use cadcraft_geom::Vec2;

use crate::cmd::helpers::{arc, line, lwpoly, rect_vertices, v3};
use crate::{Result, Session};

fn on(layer: &str) -> Common {
    Common { layer: layer.into(), ..Common::default() }
}

fn text(at: Vec2, h: f64, s: &str) -> EntityKind {
    EntityKind::Text(Text {
        insert: v3(at),
        align_pt: None,
        height: h,
        value: s.into(),
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        style: "Standard".into(),
        halign: Default::default(),
        valign: Default::default(),
    })
}

fn layer(name: &str, color: u8) -> Layer {
    Layer { name: name.into(), color: Color::Index(color), linetype: "Continuous".into(), lineweight: Lineweight::Mm100(50), ..Layer::default() }
}

pub(crate) fn ensure_layers(d: &mut Drawing) {
    for (name, color) in [("A-WALL", 7u8), ("A-DOOR", 3), ("A-GLAZ", 4), ("A-ANNO", 2), ("E-SYMBOL", 5), ("E-KABEL", 1), ("E-FORSYN", 8)] {
        if d.layer(name).is_none() {
            let mut l = layer(name, color);
            if name == "E-FORSYN" {
                l.plot = false;
            }
            d.layers.push(l);
        }
    }
}

fn copy_floor_plan(s: &mut Session) -> Result<usize> {
    let src = crate::sample::floor_plan();
    let space = s.space();
    let extras: Vec<Layer> = {
        let d = s.doc()?;
        src.layers.iter().filter(|l| d.layer(&l.name).is_none()).cloned().collect()
    };
    {
        let d = s.doc_mut()?;
        d.layers.extend(extras);
        ensure_layers(d);
    }
    let ents: Vec<(Common, EntityKind)> = src.model.iter().map(|e| (e.common.clone(), e.kind.clone())).collect();
    let n = ents.len();
    for (c, k) in ents {
        s.doc_mut()?.add(&space, c, k)?;
    }
    Ok(n)
}

/// Metric house in millimetres, or the inch floor plan's proportions when the
/// drawing is in inches or feet but already has geometry (so the sample is not
/// stacked on itself).
fn draw_generated(s: &mut Session) -> Result<usize> {
    let ins = s.doc()?.header.i64("INSUNITS", 1);
    let (w, h, t, door, win, label) = if ins == 1 {
        (30.0 * 12.0, 20.0 * 12.0, 6.0, 36.0, 48.0, 9.0)
    } else if ins == 2 {
        (30.0, 20.0, 0.5, 3.0, 4.0, 0.75)
    } else {
        (9000.0, 6000.0, 150.0, 900.0, 1200.0, 250.0)
    };
    let ext = s.doc()?.extents(&s.space());
    let origin = if ext.is_empty() { Vec2::ZERO } else { Vec2::new(ext.max.x + w * 0.15, ext.min.y) };
    s.doc_mut().map(ensure_layers)?;
    let mut n = 0;
    let mut add = |c: Common, k: EntityKind| -> Result<()> {
        let space = s.space();
        s.doc_mut()?.add(&space, c, k)?;
        n += 1;
        Ok(())
    };
    let p = |x: f64, y: f64| origin + Vec2::new(x, y);
    add(on("A-WALL"), lwpoly(rect_vertices(p(0.0, 0.0), p(w, h)), true))?;
    add(on("A-WALL"), lwpoly(rect_vertices(p(t, t), p(w - t, h - t)), true))?;
    let mid = w * 0.48;
    add(on("A-WALL"), lwpoly(rect_vertices(p(mid, t), p(mid + t, h - t)), true))?;
    let hinge = p(mid + t, h * 0.28);
    add(on("A-DOOR"), line(hinge, hinge + Vec2::X * door))?;
    add(on("A-DOOR"), arc(&cadcraft_geom::Arc::new(hinge, door, 0.0, std::f64::consts::FRAC_PI_2)))?;
    for x in [w * 0.16, w * 0.66] {
        add(on("A-GLAZ"), line(p(x, 0.0), p(x + win, 0.0)))?;
        add(on("A-GLAZ"), line(p(x, t), p(x + win, t)))?;
        add(on("A-GLAZ"), line(p(x, t / 2.0), p(x + win, t / 2.0)))?;
    }
    add(on("A-ANNO"), text(p(w * 0.12, h * 0.5), label, "STUE"))?;
    add(on("A-ANNO"), text(p(w * 0.62, h * 0.5), label, "SOVEVÆRELSE"))?;
    Ok(n)
}

/// Draw a house into the current drawing.
///
/// An empty inch drawing receives [`crate::sample::floor_plan`] itself (the
/// house CADCraft already draws). Any other drawing gets the same wall, door
/// and window construction, in that drawing's units.
pub fn draw_house(s: &mut Session) -> Result<usize> {
    let ins = s.doc()?.header.i64("INSUNITS", 1);
    let empty = s.doc()?.model.iter().next().is_none() && matches!(s.space(), Space::Model);
    if empty && ins == 1 { copy_floor_plan(s) } else { draw_generated(s) }
}
