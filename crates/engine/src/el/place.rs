//! Place symbols and cables on the current drawing, and read them back.

use std::sync::Arc;

use cadcraft_doc::{Block, Common, Drawing, Entity, EntityKind, Handle, Insert, Point};
use cadcraft_geom::{PolyVertex, Vec2, Vec3};
use serde_json::{Value, json};

use super::calc::{self, Z};
use super::house;
use super::model::{self, Cable, Supply, SymbolRec};
use super::symbols;
use crate::cmd::helpers::{lwpoly, v3};
use crate::{Result, Session};

pub fn ensure_el_layers(d: &mut Drawing) {
    house::ensure_layers(d);
}

fn block_name(kind: &str) -> String {
    format!("EL_{}", kind.to_ascii_uppercase())
}

pub fn ensure_symbol_block(d: &mut Drawing, kind: &str) -> String {
    let name = block_name(kind);
    if let Some(existing) = d.block(&name).map(|b| b.name.clone()) {
        return existing;
    }
    let mut block = Block::new(&name);
    block.explodable = true;
    block.description = model::symbol_by_id(kind).map(|s| s.name).unwrap_or(kind).to_string();
    for kind_ent in symbols::geometry(kind) {
        let h = d.new_handle();
        block.entities.push(Entity { handle: h, common: Common { layer: "0".into(), ..Common::default() }, kind: kind_ent });
    }
    d.blocks.insert(name.clone(), Arc::new(block));
    name
}

fn replace_xdata(tags: &mut Vec<cadcraft_doc::RawTag>, fresh: Vec<cadcraft_doc::RawTag>) {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tags.len() {
        if tags[i].code == 1001 && tags[i].value.trim().eq_ignore_ascii_case(model::APP) {
            i += 1;
            while i < tags.len() && tags[i].code != 1001 {
                i += 1;
            }
            continue;
        }
        out.push(tags[i].clone());
        i += 1;
    }
    out.extend(fresh);
    *tags = out;
}

pub fn place_symbol(s: &mut Session, kind: &str, at: Vec2, rotation_deg: f64) -> Result<Handle> {
    let spec = model::symbol_by_id(kind).ok_or_else(|| crate::EngineError::Other(format!("Ukendt symbol \"{kind}\".")))?;
    let insunits = s.doc()?.header.i64("INSUNITS", 0);
    let scale = model::symbol_world(insunits) / 32.0;
    if !scale.is_finite() || scale <= 0.0 || !at.is_finite() {
        return Err(crate::EngineError::Other("Punktet kan ikke bruges.".into()));
    }
    let space = s.space();
    let name = {
        let d = s.doc_mut()?;
        house::ensure_layers(d);
        ensure_symbol_block(d, spec.id)
    };
    let mut common = s.current_common();
    common.layer = "E-SYMBOL".into();
    common.xdata = model::pack_symbol(&SymbolRec { kind: spec.id.into(), name: spec.name.into() });
    let kind = EntityKind::Insert(Insert {
        block: name,
        insert: v3(at),
        scale: Vec3::new(scale, scale, 1.0),
        rotation: rotation_deg.to_radians(),
        attribs: Vec::new(),
        cols: 1,
        rows: 1,
        col_spacing: 0.0,
        row_spacing: 0.0,
    });
    let h = s.doc_mut()?.add(&space, common, kind)?;
    s.set_selection(vec![h]);
    s.last_point = at;
    Ok(h)
}

fn each_entity(d: &Drawing, mut f: impl FnMut(&Entity)) {
    for e in d.model.iter() {
        f(e.as_ref());
    }
    for layout in &d.layouts {
        for e in layout.entities.iter() {
            f(e.as_ref());
        }
    }
}

struct Item {
    handle: Handle,
    cable: Cable,
    drawn: Option<f64>,
}

fn poly_len(e: &Entity) -> Option<f64> {
    let EntityKind::LwPolyline(pl) = &e.kind else { return None };
    if pl.vertices.len() < 2 {
        return None;
    }
    let mut d = 0.0;
    for w in pl.vertices.windows(2) {
        d += w[0].p.dist(w[1].p);
    }
    if d.is_finite() && d > 0.0 { Some(d) } else { None }
}

fn collect(d: &Drawing) -> (Supply, Vec<Item>, Vec<(Handle, SymbolRec, Vec2)>) {
    let mut supply = Supply::default();
    let mut found_supply = false;
    let mut cables = Vec::new();
    let mut symbols = Vec::new();
    each_entity(d, |e| {
        if let Some(s) = model::read_supply(&e.common.xdata)
            && !found_supply
        {
            supply = s;
            found_supply = true;
        }
        if let Some(c) = model::read_cable(&e.common.xdata) {
            cables.push(Item { handle: e.handle, cable: c, drawn: poly_len(e) });
        }
        if let Some(sym) = model::read_symbol(&e.common.xdata) {
            let at = match &e.kind {
                EntityKind::Insert(i) => i.insert.xy(),
                EntityKind::Text(t) => t.insert.xy(),
                _ => Vec2::ZERO,
            };
            symbols.push((e.handle, sym, at));
        }
    });
    (supply, cables, symbols)
}

fn upstream_of(items: &[Item], handle: Handle, supply: Z, ins: i64, stack: &mut Vec<Handle>) -> Z {
    if stack.contains(&handle) || stack.len() > 40 {
        return supply;
    }
    let Some(item) = items.iter().find(|c| c.handle == handle) else { return supply };
    let Some(parent) = model::parent_handle(&item.cable) else { return supply };
    if !items.iter().any(|c| c.handle == parent) {
        return supply;
    }
    stack.push(handle);
    let pz = upstream_of(items, parent, supply, ins, stack);
    let Some(p) = items.iter().find(|c| c.handle == parent) else { return pz };
    let len = model::length_m(&p.cable, p.drawn, ins);
    let section = model::used_section(&p.cable, len);
    let imp = calc::impedance(section, len, &p.cable.material).unwrap_or(Z::ZERO);
    pz + imp
}

fn num(x: f64) -> Value {
    if x.is_finite() { json!(x) } else { Value::Null }
}

fn opt_num(x: Option<f64>) -> Value {
    x.filter(|v| v.is_finite()).map(|v| json!(v)).unwrap_or(Value::Null)
}

fn opt_bool(x: Option<bool>) -> Value {
    match x {
        Some(v) => json!(v),
        None => Value::Null,
    }
}

fn result_json(o: &calc::Output) -> Value {
    if !o.ok {
        return json!({ "ok": false, "error": o.error });
    }
    json!({
        "ok": true,
        "I": o.i,
        "L": o.l,
        "U": o.u,
        "U0": num(o.u0),
        "phase": o.phase,
        "nLoaded": o.n_loaded,
        "section": o.section,
        "locked": o.locked,
        "minSection": opt_num(o.min_section),
        "recommended": o.recommended,
        "recommendedMeets": o.recommended_meets,
        "capacity": opt_num(o.capacity),
        "Iz": opt_num(o.corrected_capacity),
        "correctedCapacity": opt_num(o.corrected_capacity),
        "dropV": num(o.drop_v),
        "dropPct": num(o.drop_pct),
        "dropLimitPct": o.drop_limit_pct,
        "dropOk": o.drop_ok,
        "currentOk": o.current_ok,
        "Ikmax": num(o.ikmax),
        "Ikmin": num(o.ikmin),
        "material": o.material,
        "materialLabel": o.material_label,
        "brandLabel": o.brand_label,
        "method": o.method,
        "overOK": opt_bool(o.over_ok),
        "earthOK": opt_bool(o.earth_ok),
        "shortOK": opt_bool(o.short_ok),
        "peSection": o.pe_section,
    })
}

pub fn snapshot(s: &Session) -> Result<Value> {
    let d = s.doc()?;
    let ins = d.header.i64("INSUNITS", 0);
    let (supply, items, symbols) = collect(d);
    let selected: Vec<String> = s.selection().iter().map(|h| h.hex()).collect();
    let mut cables = Vec::new();
    for item in &items {
        let len = model::length_m(&item.cable, item.drawn, ins);
        let mut stack = Vec::new();
        let up = upstream_of(&items, item.handle, supply.z(), ins, &mut stack);
        let out = model::analyze_cable(&item.cable, len, up);
        cables.push(json!({
            "handle": item.handle.hex(),
            "name": item.cable.name,
            "parent": item.cable.parent.clone().map(Value::from).unwrap_or(Value::Null),
            "designation": item.cable.designation,
            "cores": item.cable.cores.map(|n| json!(n)).unwrap_or(Value::Null),
            "phase": item.cable.phase,
            "voltage": item.cable.voltage,
            "loaded": item.cable.loaded,
            "section": item.cable.section,
            "sectionMode": item.cable.section_mode,
            "minSection": model::minimum_section(&item.cable).map(|s| json!(s)).unwrap_or(Value::Null),
            "I": item.cable.current,
            "lengthM": item.cable.length_m,
            "lengthMode": item.cable.length_mode,
            "drawnM": item.drawn.map(|u| u * model::metres_per_unit(ins)).filter(|m| m.is_finite()).map(|m| json!(m)).unwrap_or(Value::Null),
            "usedM": len,
            "cosphi": item.cable.cosphi,
            "dropPctLimit": item.cable.drop_pct,
            "material": item.cable.material,
            "brand": item.cable.brand,
            "method": item.cable.method,
            "temp": item.cable.temp,
            "sidelob": item.cable.sidelob,
            "sidelobType": item.cable.sidelob_type,
            "In": item.cable.in_amp,
            "breakerType": item.cable.breaker_type,
            "systemType": item.cable.system_type,
            "result": result_json(&out),
        }));
    }
    let symbols =
        symbols.iter().map(|(h, sym, at)| json!({ "handle": h.hex(), "kind": sym.kind, "name": sym.name, "at": [at.x, at.y] })).collect::<Vec<_>>();
    Ok(json!({
        "supply": { "R1": supply.r1, "X1": supply.x1, "R0": supply.r0, "X0": supply.x0 },
        "symbols": symbols,
        "cables": cables,
        "selected": selected,
        "insunits": ins,
    }))
}

const CABLE_COLORS: [u8; 8] = [1, 2, 3, 4, 5, 6, 30, 40];

pub fn place_cable(s: &mut Session, points: &[Vec2]) -> Result<Handle> {
    if points.len() < 2 || points.iter().any(|p| !p.is_finite()) {
        return Err(crate::EngineError::Other("Et kabel skal have mindst to punkter.".into()));
    }
    let mut verts: Vec<PolyVertex> = Vec::new();
    for p in points {
        if verts.last().is_some_and(|v: &PolyVertex| v.p.near(*p, 1e-9)) {
            continue;
        }
        verts.push(PolyVertex::new(*p));
    }
    if verts.len() < 2 {
        return Err(crate::EngineError::Other("Et kabel skal have mindst to punkter.".into()));
    }
    let space = s.space();
    let n = {
        let d = s.doc()?;
        let (_, items, _) = collect(d);
        items.len() + 1
    };
    let insunits = s.doc()?.header.i64("INSUNITS", 0);
    let width = (model::symbol_world(insunits) / 18.0).max(0.0);
    s.doc_mut().map(house::ensure_layers)?;
    let mut cable = Cable::new(n);
    cable.length_mode = "draw".into();
    let mut common = s.current_common();
    common.layer = "E-KABEL".into();
    common.color = cadcraft_color::Color::Index(CABLE_COLORS[(n - 1) % CABLE_COLORS.len()]);
    common.xdata = model::pack_cable(&cable);
    let mut pl = lwpoly(verts, false);
    if let EntityKind::LwPolyline(p) = &mut pl {
        p.const_width = width;
    }
    let h = s.doc_mut()?.add(&space, common, pl)?;
    s.set_selection(vec![h]);
    if let Some(p) = points.last() {
        s.last_point = *p;
    }
    Ok(h)
}

pub fn update_cable_points(s: &mut Session, h: Handle, points: &[Vec2]) -> Result<()> {
    let verts: Vec<PolyVertex> = points.iter().map(|p| PolyVertex::new(*p)).collect();
    s.doc_mut()?.modify_entity(h, |e| {
        if let EntityKind::LwPolyline(pl) = &mut e.kind {
            let w = pl.const_width;
            pl.vertices = verts;
            pl.const_width = w;
        }
    })?;
    Ok(())
}

pub fn write_cable(s: &mut Session, h: Handle, cable: &Cable) -> Result<()> {
    let tags = model::pack_cable(cable);
    let found = s.doc_mut()?.modify_entity(h, |e| {
        if model::read_cable(&e.common.xdata).is_some() {
            replace_xdata(&mut e.common.xdata, tags.clone());
        }
    });
    // modify_entity returns Result and always calls the closure if the entity exists.
    // Re-read to see whether it was a cable.
    found?;
    if s.doc()?.entity(h).and_then(|e| model::read_cable(&e.common.xdata)).is_none() {
        return Err(crate::EngineError::Other("Objektet er ikke et kabel.".into()));
    }
    Ok(())
}

pub fn cable_of(s: &Session, h: Handle) -> Result<Cable> {
    s.doc()?.entity(h).and_then(|e| model::read_cable(&e.common.xdata)).ok_or_else(|| crate::EngineError::Other("Objektet er ikke et kabel.".into()))
}

pub fn selected_cable(s: &Session) -> Result<(Handle, Cable)> {
    for h in s.selection() {
        if let Ok(c) = cable_of(s, h) {
            return Ok((h, c));
        }
    }
    Err(crate::EngineError::Other("Vælg et kabel først.".into()))
}

fn finite_pos(v: Option<f64>) -> Option<f64> {
    v.filter(|n| n.is_finite() && *n > 0.0)
}

pub fn patch_cable(cable: &mut Cable, p: &Value) -> std::result::Result<(), String> {
    if let Some(name) = p.get("name").and_then(Value::as_str) {
        let name = name.trim();
        if !name.is_empty() {
            cable.name = name.to_string();
        }
    }
    if p.get("parent").is_some() {
        cable.parent = match p.get("parent") {
            Some(Value::Null) => None,
            Some(Value::String(s)) if s.trim().is_empty() => None,
            Some(Value::String(s)) => Some(s.trim().to_string()),
            _ => cable.parent.clone(),
        };
    }
    if let Some(text) = p.get("designation").and_then(Value::as_str) {
        cable.apply_designation(text)?;
    }
    let keep_voltage = p.get("voltage").is_some();
    if let Some(phase) = p.get("phase").and_then(Value::as_str) {
        cable.set_phase(phase, keep_voltage);
    }
    if let Some(v) = p.get("voltage").and_then(Value::as_f64).filter(|v| v.is_finite() && *v > 0.0) {
        cable.voltage = v;
    }
    if let Some(loaded) = p.get("loaded").and_then(Value::as_str)
        && matches!(loaded, "2" | "3" | "4")
    {
        cable.loaded = loaded.into();
    } else if let Some(n) = p.get("loaded").and_then(Value::as_i64)
        && (2..=4).contains(&n)
    {
        cable.loaded = n.to_string();
    }
    if let Some(i) = finite_pos(p.get("I").and_then(Value::as_f64)) {
        cable.current = i;
    }
    if let Some(mode) = p.get("lengthMode").and_then(Value::as_str)
        && (mode == "draw" || mode == "manual")
    {
        cable.length_mode = mode.into();
    }
    if let Some(l) = finite_pos(p.get("lengthM").and_then(Value::as_f64)) {
        cable.length_m = l;
        if p.get("lengthMode").is_none() {
            cable.length_mode = "manual".into();
        }
    }
    if let Some(c) = p.get("cosphi").and_then(Value::as_f64).filter(|c| c.is_finite()) {
        cable.cosphi = c;
    }
    if let Some(d) = finite_pos(p.get("dropPct").and_then(Value::as_f64)) {
        cable.drop_pct = d;
    }
    if let Some(m) = p.get("material").and_then(Value::as_str) {
        cable.material = if m == "al" { "al" } else { "cu" }.into();
    }
    if let Some(b) = p.get("brand").and_then(Value::as_str)
        && matches!(b, "nkt" | "waskoenig" | "prysmian")
    {
        cable.brand = b.into();
    }
    if let Some(m) = p.get("method").and_then(Value::as_str)
        && matches!(m, "A1" | "A2" | "B1" | "B2" | "C" | "D")
    {
        cable.method = m.into();
        if m == "D" && (cable.temp - 30.0).abs() < 1e-9 {
            cable.temp = 20.0;
        }
        if m != "D" && (cable.temp - 20.0).abs() < 1e-9 {
            cable.temp = 30.0;
        }
    }
    if let Some(t) = p.get("temp").and_then(Value::as_f64).filter(|t| t.is_finite()) {
        cable.temp = t;
    }
    if let Some(n) = p.get("sidelob").and_then(Value::as_str) {
        cable.sidelob = n.into();
    }
    if let Some(t) = p.get("sidelobType").and_then(Value::as_str) {
        cable.sidelob_type = t.into();
    }
    if let Some(mode) = p.get("sectionMode").and_then(Value::as_str)
        && (mode == "auto" || mode == "manual")
    {
        cable.section_mode = mode.into();
    }
    if let Some(sec) = finite_pos(p.get("section").and_then(Value::as_f64)) {
        cable.section = sec;
        if p.get("sectionMode").is_none() && p.get("designation").is_none() {
            cable.section_mode = "manual".into();
        }
    }
    if let Some(i) = p.get("In").and_then(Value::as_str) {
        cable.in_amp = i.into();
    } else if let Some(i) = p.get("In").and_then(Value::as_i64) {
        cable.in_amp = i.to_string();
    }
    if let Some(b) = p.get("breakerType").and_then(Value::as_str) {
        cable.breaker_type = b.into();
    }
    if let Some(sys) = p.get("systemType").and_then(Value::as_str)
        && (sys == "TN" || sys == "TT")
    {
        cable.system_type = sys.into();
    }
    Ok(())
}

pub fn store_supply(s: &mut Session, supply: &Supply) -> Result<Handle> {
    let space = s.space();
    let existing = {
        let d = s.doc()?;
        let mut found = None;
        each_entity(d, |e| {
            if found.is_none() && model::read_supply(&e.common.xdata).is_some() {
                found = Some(e.handle);
            }
        });
        found
    };
    let tags = model::pack_supply(supply);
    if let Some(h) = existing {
        s.doc_mut()?.modify_entity(h, |e| replace_xdata(&mut e.common.xdata, tags.clone()))?;
        return Ok(h);
    }
    s.doc_mut().map(house::ensure_layers)?;
    let mut common = s.current_common();
    common.layer = "E-FORSYN".into();
    common.visible = false;
    common.xdata = tags;
    let h = s.doc_mut()?.add(&space, common, EntityKind::Point(Point { p: v3(Vec2::ZERO), angle: 0.0 }))?;
    Ok(h)
}

pub fn draw_house(s: &mut Session) -> Result<usize> {
    house::draw_house(s)
}

/// Open CADCraft's own floor-plan sample as a drawing.
pub fn open_floor_plan(s: &mut Session) {
    s.open_drawing(crate::sample::floor_plan(), "Plantegning", None);
}
