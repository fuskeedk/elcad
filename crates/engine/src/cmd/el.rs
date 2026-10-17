//! Electrical commands: symbols, cables and the Danish calculation panel's engine.

use cadcraft_doc::{EntityKind, Handle};
use cadcraft_geom::{PolyVertex, Vec2};
use serde_json::{Value, json};

use super::*;
use crate::el::model::{self, Supply};
use crate::el::{self, place};
use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("el.symbol", "Symbol", run_symbol)
            .menu(&["Draw", "El", "Symbol"])
            .alias(&["elsymbol"])
            .params("{kind: stik|afbryder|lampe|tavle|…, at: [x,y], rotation?: deg}")
            .interactive(|_| Ok(Box::new(SymbolM { kind: None }))),
        CommandSpec::new("el.cable", "Kabel", run_cable)
            .menu(&["Draw", "El", "Kabel"])
            .alias(&["elkabel", "kabel"])
            .params("{points: [[x,y], ...]}")
            .interactive(|_| Ok(Box::new(CableM::default()))),
        CommandSpec::new("el.house", "Tegn hus", run_house)
            .menu(&["Draw", "El", "Tegn hus"])
            .alias(&["elhus"])
            .params("{} — tegner vægge som Cadcrafts plantegning"),
        CommandSpec::new("el.floorplan", "Åbn plantegning", run_floorplan)
            .menu(&["Draw", "El", "Åbn plantegning"])
            .alias(&["elplan"])
            .params("{} — åbner sample::floor_plan")
            .noundo(),
        CommandSpec::new("el.set", "Sæt kabel", run_set).params("{handle?, designation?, phase?, voltage?, loaded?, I?, lengthM?, …}"),
        CommandSpec::new("el.supply", "Forsyning", run_supply).params("{R1?, X1?, R0?, X0?} eller {Ikmax, U0?}"),
        CommandSpec::new("el.state", "El-status", run_state).params("{} → symboler, kabler og beregning").noundo(),
        CommandSpec::new("el.arm", "Vælg symbol", run_arm).params("{kind}").noundo(),
    ]
}

fn run_arm(_s: &mut Session, p: &Value) -> Result<Value> {
    let kind = str_param(p, "kind").unwrap_or("stik");
    let spec = model::symbol_by_id(kind).ok_or_else(|| bad("el.arm", format!("ukendt symbol \"{kind}\"")))?;
    el::arm(spec.id);
    Ok(json!({ "kind": spec.id, "name": spec.name }))
}

fn run_symbol(s: &mut Session, p: &Value) -> Result<Value> {
    let kind = str_param(p, "kind").unwrap_or("stik");
    let Some(at) = point_param(p, "at") else {
        let spec = model::symbol_by_id(kind).ok_or_else(|| bad("el.symbol", format!("ukendt symbol \"{kind}\"")))?;
        el::arm(spec.id);
        return Ok(json!({ "message": format!("Symbol {}: vælg et punkt.", spec.name), "kind": spec.id }));
    };
    let rot = f64_or(p, "rotation", 0.0);
    let h = place::place_symbol(s, kind, at, rot)?;
    let name = model::symbol_by_id(kind).map(|k| k.name).unwrap_or(kind);
    s.echo(format!("{name} sat."));
    Ok(json!({ "handle": h.hex(), "kind": kind, "message": format!("{name} sat.") }))
}

fn run_cable(s: &mut Session, p: &Value) -> Result<Value> {
    let pts = points_param(p, "points").ok_or_else(|| bad("el.cable", "`points` [[x,y], ...] (2 eller flere) mangler"))?;
    let h = place::place_cable(s, &pts)?;
    let message = summary(s, h);
    s.echo(message.clone());
    Ok(json!({ "handle": h.hex(), "message": message }))
}

fn run_house(s: &mut Session, _p: &Value) -> Result<Value> {
    let n = place::draw_house(s)?;
    let message = format!("Hus tegnet ({n} objekter). Sæt et symbol, og tegn et kabel.");
    s.echo(message.clone());
    Ok(json!({ "entities": n, "message": message }))
}

fn run_floorplan(s: &mut Session, _p: &Value) -> Result<Value> {
    place::open_floor_plan(s);
    Ok(json!({ "message": "Åbnede Cadcrafts plantegning." }))
}

fn target_handle(s: &Session, p: &Value) -> Result<Handle> {
    if let Some(v) = p.get("handle") {
        let h = v.as_str().and_then(Handle::parse_hex).or_else(|| v.as_u64().map(Handle));
        return h.ok_or_else(|| bad("el.set", "`handle` er ikke et objekt"));
    }
    Ok(place::selected_cable(s)?.0)
}

fn run_set(s: &mut Session, p: &Value) -> Result<Value> {
    let h = target_handle(s, p)?;
    let mut cable = place::cable_of(s, h)?;
    place::patch_cable(&mut cable, p).map_err(|m| bad("el.set", m))?;
    place::write_cable(s, h, &cable)?;
    s.set_selection(vec![h]);
    let message = summary(s, h);
    Ok(
        json!({ "handle": h.hex(), "message": message, "phase": cable.phase, "voltage": cable.voltage, "loaded": cable.loaded, "section": cable.section, "sectionMode": cable.section_mode, "designation": cable.designation, "cores": cable.cores }),
    )
}

fn run_supply(s: &mut Session, p: &Value) -> Result<Value> {
    let mut supply = Supply::default();
    if let Some(state) = place::snapshot(s).ok().and_then(|v| v.get("supply").cloned()) {
        supply.r1 = state.get("R1").and_then(Value::as_f64).unwrap_or(0.0);
        supply.x1 = state.get("X1").and_then(Value::as_f64).unwrap_or(0.0);
        supply.r0 = state.get("R0").and_then(Value::as_f64).unwrap_or(0.0);
        supply.x0 = state.get("X0").and_then(Value::as_f64).unwrap_or(0.0);
    }
    let set = |cur: &mut f64, key: &str| {
        if let Some(v) = p.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()) {
            *cur = v;
        }
    };
    set(&mut supply.r1, "R1");
    set(&mut supply.x1, "X1");
    set(&mut supply.r0, "R0");
    set(&mut supply.x0, "X0");
    if let Some(ik) = p.get("Ikmax").and_then(Value::as_f64) {
        let u0 = p.get("U0").and_then(Value::as_f64).filter(|u| u.is_finite() && *u > 0.0).unwrap_or(230.0);
        if let Some(z) = crate::el::calc::impedance_from_ik(ik, u0) {
            supply.r1 = z.r1;
            supply.x1 = z.x1;
            supply.r0 = z.r0;
            supply.x0 = z.x0;
        }
    }
    place::store_supply(s, &supply)?;
    Ok(json!({ "R1": supply.r1, "X1": supply.x1, "R0": supply.r0, "X0": supply.x0 }))
}

fn run_state(s: &mut Session, _p: &Value) -> Result<Value> {
    place::snapshot(s)
}

fn summary(s: &Session, h: Handle) -> String {
    let Ok(state) = place::snapshot(s) else {
        return "Kabel tegnet.".into();
    };
    let Some(cables) = state.get("cables").and_then(Value::as_array) else {
        return "Kabel tegnet.".into();
    };
    let hex = h.hex();
    let Some(c) = cables.iter().find(|c| c.get("handle").and_then(Value::as_str) == Some(hex.as_str())) else {
        return "Kabel tegnet.".into();
    };
    let result = c.get("result");
    let phase = c.get("phase").and_then(Value::as_str).unwrap_or("1");
    let voltage = c.get("voltage").and_then(Value::as_f64).unwrap_or(230.0);
    let loaded = c.get("loaded").and_then(Value::as_str).unwrap_or("2");
    let section = result.and_then(|r| r.get("section")).and_then(Value::as_f64).unwrap_or(0.0);
    let designation = c.get("designation").and_then(Value::as_str).unwrap_or("");
    let iz = result.and_then(|r| r.get("Iz")).and_then(Value::as_f64);
    let drop = result.and_then(|r| r.get("dropPct")).and_then(Value::as_f64);
    let ik = result.and_then(|r| r.get("Ikmax")).and_then(Value::as_f64);
    let kind = if designation.is_empty() { format!("{section} mm²") } else { designation.to_string() };
    format!("{kind}: {phase}-fase {voltage:.0} V, {loaded} belastede ledere, Iz {}, ΔU {}, Ikmax {}.", fmt_a(iz), fmt_pct(drop), fmt_a(ik))
}

fn fmt_a(v: Option<f64>) -> String {
    match v {
        Some(a) if a.is_finite() && a.abs() >= 1000.0 => format!("{:.2} kA", a / 1000.0),
        Some(a) if a.is_finite() => format!("{a:.1} A"),
        _ => "ikke begrænset".into(),
    }
}

fn fmt_pct(v: Option<f64>) -> String {
    match v {
        Some(p) if p.is_finite() => format!("{p:.2} %"),
        _ => "—".into(),
    }
}

struct SymbolM {
    kind: Option<String>,
}

impl Interactive for SymbolM {
    fn name(&self) -> &'static str {
        "ELSYMBOL"
    }
    fn begin(&mut self, _s: &mut Session) -> Result<Step> {
        if let Some(kind) = el::take_armed()
            && model::symbol_by_id(&kind).is_some()
        {
            self.kind = Some(kind);
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.kind.is_none() {
            Prompt::new("Symbol", Accept::TEXT)
                .kw(&["Stik", "Stik2", "Afbryder", "Lampe", "Tavle", "Korr", "Kryds", "Udtag", "Ude", "Daase", "Kraft", "Komfur", "Tekst"])
        } else {
            let name = self.kind.as_deref().and_then(model::symbol_by_id).map(|s| s.name).unwrap_or("Symbol");
            Prompt::new(format!("Indsætningspunkt for {name}"), Accept::POINT)
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Keyword(k) | Input::Text(k) if self.kind.is_none() => {
                let spec = model::symbol_by_id(&k).ok_or_else(|| crate::EngineError::Other(format!("Ukendt symbol \"{k}\".")))?;
                self.kind = Some(spec.id.into());
                Ok(Step::Continue)
            }
            Input::Point(p) => {
                let kind = self.kind.clone().unwrap_or_else(|| "stik".into());
                place::place_symbol(s, &kind, p, 0.0)?;
                let name = model::symbol_by_id(&kind).map(|k| k.name).unwrap_or("Symbol");
                s.echo(format!("{name} sat."));
                Ok(Step::Done)
            }
            Input::Enter if self.kind.is_none() => {
                self.kind = Some("stik".into());
                Ok(Step::Continue)
            }
            Input::Enter | Input::Cancel => Ok(Step::Cancel),
            _ => Err(crate::EngineError::Other("Vælg et symbol eller et punkt.".into())),
        }
    }
    fn preview(&self, _s: &Session, cursor: Vec2) -> Vec<EntityKind> {
        let Some(kind) = &self.kind else { return Vec::new() };
        let scale = 1.0;
        symbols_preview(kind, cursor, scale)
    }
}

fn symbols_preview(_kind: &str, at: Vec2, _scale: f64) -> Vec<EntityKind> {
    vec![
        crate::cmd::helpers::line(at - Vec2::new(4.0, 0.0), at + Vec2::new(4.0, 0.0)),
        crate::cmd::helpers::line(at - Vec2::new(0.0, 4.0), at + Vec2::new(0.0, 4.0)),
    ]
}

#[derive(Default)]
struct CableM {
    pts: Vec<Vec2>,
    handle: Option<Handle>,
}

impl CableM {
    fn sync(&mut self, s: &mut Session) -> Result<()> {
        if self.pts.len() < 2 {
            if let Some(h) = self.handle.take() {
                s.doc_mut()?.remove_entity(h);
            }
            return Ok(());
        }
        if let Some(h) = self.handle {
            place::update_cable_points(s, h, &self.pts)?;
        } else {
            self.handle = Some(place::place_cable(s, &self.pts)?);
        }
        Ok(())
    }
}

impl Interactive for CableM {
    fn name(&self) -> &'static str {
        "ELKABEL"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.pts.is_empty() {
            Prompt::new("Første punkt på kablet", Accept::POINT)
        } else {
            Prompt::new("Næste punkt", Accept::POINT).kw(&["Undo"]).base_opt(self.pts.last().copied())
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Point(p) => {
                if self.pts.last().is_some_and(|l| l.near(p, 1e-9)) {
                    return Ok(Step::Continue);
                }
                self.pts.push(p);
                self.sync(s)?;
                Ok(Step::Continue)
            }
            Input::Keyword(k) if k == "Undo" => {
                self.pts.pop();
                self.sync(s)?;
                Ok(Step::Continue)
            }
            Input::Enter => {
                if self.pts.len() < 2 {
                    s.echo("Kablet skal have mindst to punkter.");
                    return Ok(Step::Cancel);
                }
                if let Some(h) = self.handle {
                    s.set_selection(vec![h]);
                    s.echo(summary(s, h));
                }
                Ok(Step::Done)
            }
            Input::Cancel => Ok(Step::Cancel),
            _ => Err(crate::EngineError::Other("Vælg et punkt, eller afslut med Enter.".into())),
        }
    }
    fn preview(&self, _s: &Session, cursor: Vec2) -> Vec<EntityKind> {
        let Some(last) = self.pts.last().copied() else { return Vec::new() };
        let mut verts: Vec<PolyVertex> = self.pts.iter().copied().map(PolyVertex::new).collect();
        verts.push(PolyVertex::new(cursor));
        if verts.len() < 2 || last.near(cursor, 1e-9) {
            return vec![crate::cmd::helpers::line(last, cursor)];
        }
        vec![crate::cmd::helpers::lwpoly(verts, false)]
    }
}
