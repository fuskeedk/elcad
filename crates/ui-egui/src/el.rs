//! El-tegning: symbols, cables and the calculation from the El-beregner.
//!
//! The panel reads `el.state` and writes through `el.set`. The number after
//! G or X is the minimum section for every core count. It does not lock the
//! section and does not change phase, voltage or loaded conductors.

use cadcraft_doc::Handle;
use egui::{Color32, RichText};
use serde_json::{Value, json};

use crate::CadApp;
use crate::theme::Tokens;

#[derive(Clone, Debug)]
struct Form {
    handle: String,
    designation: String,
    voltage: String,
    current: String,
    length: String,
    cosphi: String,
    drop: String,
    temp: String,
    r1: String,
    x1: String,
    r0: String,
    x0: String,
    ik: String,
    u0: String,
}

impl Default for Form {
    fn default() -> Self {
        Form {
            handle: String::new(),
            designation: String::new(),
            voltage: "230".into(),
            current: "16".into(),
            length: "25".into(),
            cosphi: "0,9".into(),
            drop: "3".into(),
            temp: "30".into(),
            r1: "0".into(),
            x1: "0".into(),
            r0: "0".into(),
            x0: "0".into(),
            ik: String::new(),
            u0: "230".into(),
        }
    }
}

fn form_id() -> egui::Id {
    egui::Id::new("cadcraft-el-form")
}

/// Open the El palette. The first time it floats beside the drawing.
pub fn open(app: &mut CadApp) -> Result<Value, String> {
    let panel = crate::docking::Panel::Electrical;
    let first = !app.ui.docking.layout.contains(&panel) && !app.ui.docking.hidden.iter().any(|(p, _)| *p == panel);
    let r = crate::docking::command(app, false, &json!({ "operation": "open", "panel": panel.id() }))?;
    if !first {
        return Ok(r);
    }
    let area = app.canvas.rect.unwrap_or(egui::Rect::from_min_size(egui::pos2(220.0, 80.0), egui::vec2(1000.0, 800.0)));
    let w = 360.0;
    let h = (area.height() - 48.0).clamp(320.0, 760.0);
    let rect = [area.right() - w - 12.0, area.top() + 16.0, w, h];
    crate::docking::command(app, false, &json!({ "operation": "float", "panel": panel.id(), "rect": rect }))
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let state = app.session.execute("el.state", &json!({})).unwrap_or(Value::Null);
    let mut form: Form = ui.ctx().data(|d| d.get_temp(form_id())).unwrap_or_default();
    egui::ScrollArea::vertical().id_salt("dock-el").auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("El-tegning").strong().color(t.text));
        ui.label(RichText::new("Tegn huset, sæt symboler, beregn Ik og spændingsfald.").color(t.text_dim).size(12.0));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Tegn hus").clicked() {
                let _ = app.run("el.house", json!({}));
                let _ = app.run("zoom.extents", json!({}));
            }
            if ui.button("Åbn plantegning").clicked() {
                let _ = app.run("el.floorplan", json!({}));
                app.ui.start_tab = false;
                app.canvas.zoom_pending = true;
            }
            if ui.button("Kabel").clicked() {
                app.start("el.cable");
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Symboler").strong().color(t.text));
        ui.horizontal_wrapped(|ui| {
            for spec in cadcraft_engine::el::model::SYMBOLS {
                if ui.button(spec.name).clicked() {
                    let _ = app.run("el.arm", json!({ "kind": spec.id }));
                    app.start("el.symbol");
                }
            }
        });
        ui.add_space(8.0);
        cable_list(app, ui, &state);
        ui.add_space(6.0);
        if let Some(cable) = selected_cable(&state) {
            sync_form(&mut form, cable);
            cable_form(app, ui, &mut form, cable);
        } else {
            ui.label(RichText::new("Vælg et kabel i listen, eller tegn et på planen.").color(t.text_dim));
        }
        ui.add_space(8.0);
        supply_form(app, ui, &mut form, &state);
        ui.add_space(12.0);
    });
    ui.ctx().data_mut(|d| d.insert_temp(form_id(), form));
}

fn cable_list(app: &mut CadApp, ui: &mut egui::Ui, state: &Value) {
    let t = Tokens::get();
    let Some(cables) = state.get("cables").and_then(Value::as_array) else { return };
    if cables.is_empty() {
        return;
    }
    ui.label(RichText::new("Kabler").strong().color(t.text));
    let selected = state.get("selected").and_then(Value::as_array);
    for c in cables {
        let handle = c.get("handle").and_then(Value::as_str).unwrap_or("");
        let name = c.get("name").and_then(Value::as_str).unwrap_or("Kabel");
        let on = selected.is_some_and(|s| s.iter().any(|v| v.as_str() == Some(handle)));
        if ui.selectable_label(on, name).clicked()
            && let Some(h) = Handle::parse_hex(handle)
        {
            app.session.set_selection(vec![h]);
        }
    }
}

fn selected_cable(state: &Value) -> Option<&Value> {
    let cables = state.get("cables")?.as_array()?;
    let selected = state.get("selected").and_then(Value::as_array);
    if let Some(sel) = selected {
        for h in sel.iter().filter_map(Value::as_str) {
            if let Some(c) = cables.iter().find(|c| c.get("handle").and_then(Value::as_str) == Some(h)) {
                return Some(c);
            }
        }
    }
    cables.first()
}

fn sync_form(form: &mut Form, cable: &Value) {
    let handle = cable.get("handle").and_then(Value::as_str).unwrap_or("").to_string();
    if form.handle == handle {
        return;
    }
    form.handle = handle;
    form.designation = cable.get("designation").and_then(Value::as_str).unwrap_or("").to_string();
    form.voltage = fmt_num(cable.get("voltage").and_then(Value::as_f64).unwrap_or(230.0), 0);
    form.current = fmt_num(cable.get("I").and_then(Value::as_f64).unwrap_or(16.0), 1);
    let shown = if cable.get("lengthMode").and_then(Value::as_str) == Some("draw") {
        cable.get("usedM").and_then(Value::as_f64).unwrap_or(0.0)
    } else {
        cable.get("lengthM").and_then(Value::as_f64).unwrap_or(25.0)
    };
    form.length = fmt_num(shown, 2);
    form.cosphi = fmt_num(cable.get("cosphi").and_then(Value::as_f64).unwrap_or(0.9), 2);
    form.drop = fmt_num(cable.get("dropPctLimit").and_then(Value::as_f64).unwrap_or(3.0), 1);
    form.temp = fmt_num(cable.get("temp").and_then(Value::as_f64).unwrap_or(30.0), 0);
}

fn cable_form(app: &mut CadApp, ui: &mut egui::Ui, form: &mut Form, cable: &Value) {
    let t = Tokens::get();
    let handle = form.handle.clone();
    ui.label(RichText::new(cable.get("name").and_then(Value::as_str).unwrap_or("Kabel")).strong().color(t.text));
    ui.label("Kabeltype");
    let edit = ui.add(egui::TextEdit::singleline(&mut form.designation).hint_text("5G1,5").desired_width(160.0).id(egui::Id::new("el-kabeltype")));
    if edit.lost_focus() && edit.changed() {
        set(app, &handle, json!({ "designation": form.designation }));
    }
    ui.label(
        RichText::new("Tallet efter G/X er mindste tværsnit for enhver betegnelse, også 3G1,5 og 7G1,5. Et langt kabel kan gå op til 2,5, 4, 6 mm² eller mere, når spændingsfald eller Iz kræver det. Tallet før G/X ændrer ikke fase, spænding eller belastede ledere.")
            .color(t.text_dim)
            .size(11.5),
    );
    let phase = cable.get("phase").and_then(Value::as_str).unwrap_or("1").to_string();
    let loaded = cable.get("loaded").and_then(Value::as_str).unwrap_or("2").to_string();
    ui.horizontal(|ui| {
        ui.label("Fase");
        let mut phase_now = phase.clone();
        egui::ComboBox::from_id_salt("el-phase").selected_text(if phase == "3" { "3-fase" } else { "1-fase" }).show_ui(ui, |ui| {
            ui.selectable_value(&mut phase_now, "1".into(), "1-fase");
            ui.selectable_value(&mut phase_now, "3".into(), "3-fase");
        });
        if phase_now != phase {
            set(app, &handle, json!({ "phase": phase_now }));
            form.voltage = if phase_now == "3" { "400".into() } else { "230".into() };
        }
    });
    ui.horizontal(|ui| {
        ui.label("Spænding (V)");
        let v = ui.add(egui::TextEdit::singleline(&mut form.voltage).desired_width(70.0).id(egui::Id::new("el-u")));
        if v.lost_focus()
            && v.changed()
            && let Some(n) = parse_da(&form.voltage)
        {
            set(app, &handle, json!({ "voltage": n }));
        }
        ui.label("Belastede ledere");
    });
    let mut loaded_now = loaded.clone();
    egui::ComboBox::from_id_salt("el-loaded").selected_text(loaded_label(&loaded)).show_ui(ui, |ui| {
        ui.selectable_value(&mut loaded_now, "2".into(), "2 (L+N)");
        ui.selectable_value(&mut loaded_now, "3".into(), "3 (L1 L2 L3)");
        ui.selectable_value(&mut loaded_now, "4".into(), "4 (L1 L2 L3 N)");
    });
    if loaded_now != loaded {
        set(app, &handle, json!({ "loaded": loaded_now }));
    }
    ui.horizontal(|ui| {
        ui.label("Strøm Ib (A)");
        let e = ui.add(egui::TextEdit::singleline(&mut form.current).desired_width(60.0).id(egui::Id::new("el-i")));
        if e.lost_focus()
            && e.changed()
            && let Some(n) = parse_da(&form.current)
        {
            set(app, &handle, json!({ "I": n }));
        }
        ui.label("Længde (m)");
        let e = ui.add(egui::TextEdit::singleline(&mut form.length).desired_width(70.0).id(egui::Id::new("el-l")));
        if e.lost_focus()
            && e.changed()
            && let Some(n) = parse_da(&form.length)
        {
            set(app, &handle, json!({ "lengthM": n, "lengthMode": "manual" }));
        }
    });
    let draw = cable.get("lengthMode").and_then(Value::as_str) == Some("draw");
    let mut use_draw = draw;
    if ui.checkbox(&mut use_draw, "Længde fra tegningen").changed() {
        if use_draw {
            set(app, &handle, json!({ "lengthMode": "draw" }));
        } else if let Some(n) = parse_da(&form.length) {
            set(app, &handle, json!({ "lengthMode": "manual", "lengthM": n }));
        }
    }
    ui.horizontal(|ui| {
        ui.label("cos φ");
        let e = ui.add(egui::TextEdit::singleline(&mut form.cosphi).desired_width(50.0).id(egui::Id::new("el-cos")));
        if e.lost_focus()
            && e.changed()
            && let Some(n) = parse_da(&form.cosphi)
        {
            set(app, &handle, json!({ "cosphi": n }));
        }
        ui.label("ΔU max %");
        let e = ui.add(egui::TextEdit::singleline(&mut form.drop).desired_width(40.0).id(egui::Id::new("el-drop")));
        if e.lost_focus()
            && e.changed()
            && let Some(n) = parse_da(&form.drop)
        {
            set(app, &handle, json!({ "dropPct": n }));
        }
    });
    choice_row(app, ui, &handle, "Materiale", "material", cable, &[("cu", "Kobber"), ("al", "Aluminium")]);
    choice_row(app, ui, &handle, "Fabrikat", "brand", cable, &[("nkt", "NKT"), ("waskoenig", "Waskönig"), ("prysmian", "Prysmian Afumex")]);
    choice_row(
        app,
        ui,
        &handle,
        "Oplægning",
        "method",
        cable,
        &[
            ("A1", "A1 – rør i isoleret væg"),
            ("A2", "A2 – kabel i rør"),
            ("B1", "B1 – rør på væg"),
            ("B2", "B2 – flerleder i rør"),
            ("C", "C – på væg"),
            ("D", "D – i jord"),
        ],
    );
    let locked = cable.get("sectionMode").and_then(Value::as_str) == Some("manual");
    let section = cable
        .get("result")
        .and_then(|r| r.get("section"))
        .and_then(Value::as_f64)
        .or_else(|| cable.get("section").and_then(Value::as_f64))
        .unwrap_or(2.5);
    ui.horizontal(|ui| {
        ui.label(format!("Tværsnit  {} mm²", fmt_num(section, 1)));
        let mut lock = locked;
        if ui.checkbox(&mut lock, "Lås").changed() {
            if lock {
                set(app, &handle, json!({ "sectionMode": "manual", "section": section }));
            } else {
                set(app, &handle, json!({ "sectionMode": "auto" }));
            }
        }
    });
    if let Some(min) = cable.get("minSection").and_then(Value::as_f64) {
        let note = if section > min + 1e-6 {
            format!("Mindst {} mm² fra kabeltypen. Valgt {} mm², fordi spændingsfald eller Iz kræver mere.", fmt_num(min, 1), fmt_num(section, 1))
        } else {
            format!("Mindst {} mm² fra kabeltypen. Beregningen går ikke under det.", fmt_num(min, 1))
        };
        ui.label(RichText::new(note).color(t.text_dim).size(11.5));
    }
    if let Some(cores) = cable.get("cores").and_then(Value::as_u64) {
        ui.label(
            RichText::new(format!("{cores} ledere i betegnelsen. Kredsen er stadig {}-fase {} V, {} belastede ledere.", phase, form.voltage, loaded))
                .color(t.text_dim)
                .size(11.5),
        );
    }
    ui.add_space(6.0);
    results(ui, cable.get("result"));
}

fn choice_row(app: &mut CadApp, ui: &mut egui::Ui, handle: &str, label: &str, key: &str, cable: &Value, options: &[(&str, &str)]) {
    let current = cable.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    let shown = options.iter().find(|(id, _)| *id == current).map(|(_, n)| *n).unwrap_or(current.as_str());
    ui.horizontal(|ui| {
        ui.label(label);
        let mut next = current.clone();
        egui::ComboBox::from_id_salt(("el", key)).selected_text(shown).show_ui(ui, |ui| {
            for (id, name) in options {
                ui.selectable_value(&mut next, (*id).to_string(), *name);
            }
        });
        if next != current {
            let mut body = serde_json::Map::new();
            body.insert(key.to_string(), Value::String(next));
            set(app, handle, Value::Object(body));
        }
    });
}

fn results(ui: &mut egui::Ui, result: Option<&Value>) {
    let t = Tokens::get();
    ui.label(RichText::new("Beregning").strong().color(t.text));
    let Some(r) = result else {
        ui.label("Ingen beregning endnu.");
        return;
    };
    if r.get("ok").and_then(Value::as_bool) == Some(false) {
        ui.label(
            RichText::new(r.get("error").and_then(Value::as_str).unwrap_or("Beregningen kunne ikke udføres.")).color(Color32::from_rgb(220, 120, 90)),
        );
        return;
    }
    let iz = r.get("Iz").and_then(Value::as_f64);
    let drop_v = r.get("dropV").and_then(Value::as_f64);
    let drop_pct = r.get("dropPct").and_then(Value::as_f64);
    let ikmax = r.get("Ikmax").and_then(Value::as_f64);
    let ikmin = r.get("Ikmin").and_then(Value::as_f64);
    let phase = r.get("phase").and_then(Value::as_str).unwrap_or("1");
    let u = r.get("U").and_then(Value::as_f64).unwrap_or(230.0);
    let loaded = r.get("nLoaded").and_then(Value::as_i64).unwrap_or(2);
    let section = r.get("section").and_then(Value::as_f64).unwrap_or(0.0);
    line(ui, "Iz", &fmt_a(iz), r.get("currentOk").and_then(Value::as_bool));
    line(ui, "Spændingsfald", &format!("{} ({})", fmt_v(drop_v), fmt_pct(drop_pct)), r.get("dropOk").and_then(Value::as_bool));
    line(ui, "Ikmax", &fmt_a(ikmax), None);
    line(ui, "Ikmin", &fmt_a(ikmin), r.get("earthOk").and_then(Value::as_bool));
    ui.label(RichText::new(format!("{phase}-fase · {u:.0} V · {loaded} belastede ledere · {} mm²", fmt_num(section, 1))).color(t.text).size(12.5));
}

fn line(ui: &mut egui::Ui, name: &str, value: &str, ok: Option<bool>) {
    let t = Tokens::get();
    let mark = match ok {
        Some(true) => "  ok",
        Some(false) => "  ikke ok",
        None => "",
    };
    let color = match ok {
        Some(true) => Color32::from_rgb(90, 170, 110),
        Some(false) => Color32::from_rgb(210, 110, 90),
        None => t.text,
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).color(t.text_dim));
        ui.label(RichText::new(format!("{value}{mark}")).color(color));
    });
}

fn supply_form(app: &mut CadApp, ui: &mut egui::Ui, form: &mut Form, _state: &Value) {
    let t = Tokens::get();
    ui.collapsing(RichText::new("Forsyning ved tavlen").color(t.text), |ui| {
        ui.label(RichText::new("0 Ω betyder, at kun kablet begrænser kortslutningsstrømmen.").color(t.text_dim).size(11.5));
        impedance_row(app, ui, form);
        ui.horizontal(|ui| {
            ui.label("Ikmax (A)");
            ui.add(egui::TextEdit::singleline(&mut form.ik).desired_width(70.0).hint_text("6000").id(egui::Id::new("el-ik")));
            ui.label("U0");
            ui.add(egui::TextEdit::singleline(&mut form.u0).desired_width(50.0).id(egui::Id::new("el-u0")));
            if ui.button("Udfyld R/X").clicked()
                && let Some(ik) = parse_da(&form.ik)
            {
                let u0 = parse_da(&form.u0).unwrap_or(230.0);
                let _ = app.run("el.supply", json!({ "Ikmax": ik, "U0": u0 }));
                if let Some(z) = cadcraft_engine::el::calc::impedance_from_ik(ik, u0) {
                    form.r1 = fmt_num(z.r1, 4);
                    form.x1 = fmt_num(z.x1, 4);
                    form.r0 = fmt_num(z.r0, 4);
                    form.x0 = fmt_num(z.x0, 4);
                }
            }
        });
    });
}

fn impedance_row(app: &mut CadApp, ui: &mut egui::Ui, form: &mut Form) {
    let mut commit = false;
    ui.horizontal(|ui| {
        for (label, buf, id) in
            [("R1", &mut form.r1, "el-r1"), ("X1", &mut form.x1, "el-x1"), ("R0", &mut form.r0, "el-r0"), ("X0", &mut form.x0, "el-x0")]
        {
            ui.label(label);
            let e = ui.add(egui::TextEdit::singleline(buf).desired_width(52.0).id(egui::Id::new(id)));
            if e.lost_focus() && e.changed() {
                commit = true;
            }
        }
    });
    if commit {
        let _ = app.run(
            "el.supply",
            json!({
                "R1": parse_da(&form.r1).unwrap_or(0.0),
                "X1": parse_da(&form.x1).unwrap_or(0.0),
                "R0": parse_da(&form.r0).unwrap_or(0.0),
                "X0": parse_da(&form.x0).unwrap_or(0.0),
            }),
        );
    }
}

fn set(app: &mut CadApp, handle: &str, mut body: Value) {
    if let Some(obj) = body.as_object_mut() {
        obj.insert("handle".into(), Value::String(handle.into()));
    }
    let _ = app.run("el.set", body);
}

fn loaded_label(loaded: &str) -> &'static str {
    match loaded {
        "3" => "3 (L1 L2 L3)",
        "4" => "4 (L1 L2 L3 N)",
        _ => "2 (L+N)",
    }
}

fn parse_da(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn fmt_num(n: f64, digits: usize) -> String {
    if !n.is_finite() {
        return "—".into();
    }
    format!("{n:.digits$}").replace('.', ",")
}

fn fmt_a(v: Option<f64>) -> String {
    match v {
        Some(a) if a.is_finite() && a.abs() >= 1000.0 => format!("{} kA", fmt_num(a / 1000.0, 2)),
        Some(a) if a.is_finite() => format!("{} A", fmt_num(a, 1)),
        _ => "ikke begrænset".into(),
    }
}

fn fmt_v(v: Option<f64>) -> String {
    match v {
        Some(n) if n.is_finite() => format!("{} V", fmt_num(n, 2)),
        _ => "—".into(),
    }
}

fn fmt_pct(v: Option<f64>) -> String {
    match v {
        Some(n) if n.is_finite() => format!("{} %", fmt_num(n, 2)),
        _ => "—".into(),
    }
}
