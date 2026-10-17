//! Installation records stored on drawing entities.
//!
//! A cable's core count (the number before G or X) is only the designation.
//! It never changes phase, voltage or the number of loaded conductors.
//! The number after G or X is the minimum section for every core count.
//! Auto sizing may pick a larger section for spændingsfald or Iz.
//! 5G1,5 from a switch is often 230 V: mellemledning, fase, nul and jord.

use cadcraft_doc::{Handle, RawTag};
use serde::{Deserialize, Serialize};

use super::calc::{self, Input, Output, Z};

pub const APP: &str = "CADCRAFT_EL";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolKind {
    pub id: &'static str,
    pub name: &'static str,
}

pub const SYMBOLS: &[SymbolKind] = &[
    SymbolKind { id: "tavle", name: "Tavle" },
    SymbolKind { id: "stik", name: "Stikkontakt" },
    SymbolKind { id: "stik2", name: "Dobbelt stik" },
    SymbolKind { id: "afbryder", name: "Afbryder" },
    SymbolKind { id: "korr", name: "Korrespondance" },
    SymbolKind { id: "kryds", name: "Krydsning" },
    SymbolKind { id: "lampe", name: "Lampe" },
    SymbolKind { id: "udtag", name: "Lampeudtag" },
    SymbolKind { id: "ude", name: "Udendørs" },
    SymbolKind { id: "daase", name: "Dåse" },
    SymbolKind { id: "kraft", name: "Kraftudtag" },
    SymbolKind { id: "komfur", name: "Komfur" },
    SymbolKind { id: "tekst", name: "Tekst" },
];

pub fn symbol_by_id(id: &str) -> Option<&'static SymbolKind> {
    let t = id.trim();
    SYMBOLS.iter().find(|s| s.id.eq_ignore_ascii_case(t) || s.name.eq_ignore_ascii_case(t))
}

#[derive(Clone, Debug, PartialEq)]
pub enum Designation {
    Empty,
    Parsed { text: String, cores: u32, section: f64 },
}

/// `3G1,5`, `5G1.5`, `4G6`, `5x1,5` and the same with spaces.
/// `None` when the text is not a cable designation.
pub fn parse_cable_designation(text: &str) -> Option<Designation> {
    let raw = text.trim();
    if raw.is_empty() {
        return Some(Designation::Empty);
    }
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let cores: u32 = raw.get(..i)?.parse().ok()?;
    if cores < 1 {
        return None;
    }
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let mark = bytes.get(i).copied()?;
    if !matches!(mark, b'g' | b'G' | b'x' | b'X') {
        return None;
    }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let rest = raw.get(i..)?.trim();
    if rest.is_empty() {
        return None;
    }
    let section: f64 = rest.replace(',', ".").parse().ok()?;
    if !section.is_finite() || section <= 0.0 {
        return None;
    }
    // The whole remainder must be the section, not trailing junk.
    if rest.contains(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',')) {
        return None;
    }
    Some(Designation::Parsed { text: raw.to_string(), cores, section })
}

pub fn section_in_list(section: f64, material: &str) -> Option<f64> {
    calc::sections_for(material).iter().find(|s| (**s - section).abs() < 1e-6).copied()
}

/// Minimum section from any designation, or `None` when the cable has none.
/// The core count is ignored. The value is the first listed section at or above
/// the number after G or X.
pub fn minimum_section(cable: &Cable) -> Option<f64> {
    let Designation::Parsed { section, .. } = parse_cable_designation(&cable.designation)? else {
        return None;
    };
    let list = calc::sections_for(&cable.material);
    list.iter().copied().find(|s| *s + 1e-9 >= section).or_else(|| list.last().copied())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Supply {
    #[serde(rename = "R1", default)]
    pub r1: f64,
    #[serde(rename = "X1", default)]
    pub x1: f64,
    #[serde(rename = "R0", default)]
    pub r0: f64,
    #[serde(rename = "X0", default)]
    pub x0: f64,
}

impl Default for Supply {
    fn default() -> Self {
        Supply { r1: 0.0, x1: 0.0, r0: 0.0, x0: 0.0 }
    }
}

impl Supply {
    pub fn z(&self) -> Z {
        Z { r1: self.r1, x1: self.x1, r0: self.r0, x0: self.x0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cable {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    /// `"draw"` follows the polyline. `"manual"` uses [`Cable::length_m`].
    #[serde(default = "draw_mode")]
    pub length_mode: String,
    #[serde(default = "len_25")]
    pub length_m: f64,
    #[serde(rename = "I", default = "i_16")]
    pub current: f64,
    /// `"1"` is 230 V. `"3"` is 400 V. Core count does not write this.
    #[serde(default = "phase_1")]
    pub phase: String,
    #[serde(default = "volt_230")]
    pub voltage: f64,
    /// Loaded conductors: `"2"` (L+N), `"3"` or `"4"`. Not the core count.
    #[serde(default = "loaded_2")]
    pub loaded: String,
    #[serde(default = "cos_09")]
    pub cosphi: f64,
    #[serde(default = "drop_3")]
    pub drop_pct: f64,
    #[serde(default = "cu")]
    pub material: String,
    #[serde(default = "nkt")]
    pub brand: String,
    #[serde(default = "b2")]
    pub method: String,
    #[serde(default = "temp_30")]
    pub temp: f64,
    #[serde(default = "one_s")]
    pub sidelob: String,
    #[serde(default = "bundt")]
    pub sidelob_type: String,
    /// `"auto"` or `"manual"`. A designation leaves this on `"auto"`.
    #[serde(default = "auto_mode")]
    pub section_mode: String,
    #[serde(default = "sec_25")]
    pub section: f64,
    #[serde(default)]
    pub designation: String,
    /// Cores in the designation. Informational only.
    #[serde(default)]
    pub cores: Option<u32>,
    #[serde(rename = "In", default = "in_16")]
    pub in_amp: String,
    #[serde(default = "breaker_c")]
    pub breaker_type: String,
    #[serde(default = "tn")]
    pub system_type: String,
}

fn draw_mode() -> String {
    "draw".into()
}
fn len_25() -> f64 {
    25.0
}
fn i_16() -> f64 {
    16.0
}
fn phase_1() -> String {
    "1".into()
}
fn volt_230() -> f64 {
    230.0
}
fn loaded_2() -> String {
    "2".into()
}
fn cos_09() -> f64 {
    0.9
}
fn drop_3() -> f64 {
    3.0
}
fn cu() -> String {
    "cu".into()
}
fn nkt() -> String {
    "nkt".into()
}
fn b2() -> String {
    "B2".into()
}
fn temp_30() -> f64 {
    30.0
}
fn one_s() -> String {
    "1".into()
}
fn bundt() -> String {
    "bundt".into()
}
fn auto_mode() -> String {
    "auto".into()
}
fn sec_25() -> f64 {
    2.5
}
fn in_16() -> String {
    "16".into()
}
fn breaker_c() -> String {
    "C".into()
}
fn tn() -> String {
    "TN".into()
}

impl Cable {
    pub fn new(n: usize) -> Self {
        Cable { name: format!("Kabel {n}"), ..Cable::default() }
    }

    /// Apply a Kabeltype string. The number after G or X is the minimum section.
    /// Core count does not matter: 3G1,5, 5G1,5 and 7G1,5 all floor at 1,5 mm².
    /// Auto sizing stays on, so a longer run can pick 2,5, 4, 6 mm² or more.
    /// Phase, voltage and loaded conductors are left as they are.
    /// Returns an error string when the text is not a usable designation.
    pub fn apply_designation(&mut self, text: &str) -> Result<(), String> {
        match parse_cable_designation(text) {
            Some(Designation::Empty) => {
                self.designation.clear();
                self.cores = None;
                Ok(())
            }
            Some(Designation::Parsed { text, cores, section }) => {
                let Some(section) = section_in_list(section, &self.material) else {
                    return Err(format!("{section} mm² findes ikke for det valgte materiale"));
                };
                self.designation = text;
                self.cores = Some(cores);
                self.section = section;
                self.section_mode = "auto".into();
                Ok(())
            }
            None => Err("Kabeltype skal ligne 3G1,5, 7G1,5, 4G2,5 eller 5x1,5".into()),
        }
    }

    /// Phase writes the matching voltage (230 V or 400 V) unless `keep_voltage`.
    pub fn set_phase(&mut self, phase: &str, keep_voltage: bool) {
        self.phase = if phase.trim() == "3" { "3" } else { "1" }.into();
        if !keep_voltage {
            self.voltage = if self.phase == "3" { 400.0 } else { 230.0 };
        }
    }
}

impl Default for Cable {
    fn default() -> Self {
        Cable {
            name: "Kabel 1".into(),
            parent: None,
            length_mode: draw_mode(),
            length_m: 25.0,
            current: 16.0,
            phase: phase_1(),
            voltage: 230.0,
            loaded: loaded_2(),
            cosphi: 0.9,
            drop_pct: 3.0,
            material: cu(),
            brand: nkt(),
            method: b2(),
            temp: 30.0,
            sidelob: one_s(),
            sidelob_type: bundt(),
            section_mode: auto_mode(),
            section: 2.5,
            designation: String::new(),
            cores: None,
            in_amp: in_16(),
            breaker_type: breaker_c(),
            system_type: tn(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolRec {
    pub kind: String,
    pub name: String,
}

pub fn metres_per_unit(insunits: i64) -> f64 {
    match insunits {
        1 => 0.0254,
        2 => 0.3048,
        4 => 0.001,
        5 => 0.01,
        6 => 1.0,
        14 => 0.001, // decimetres? not used; millimetres is the Danish default below
        _ => 0.001,
    }
}

/// World size of a symbol, in drawing units, so it reads on both the inch floor plan and a mm house.
pub fn symbol_world(insunits: i64) -> f64 {
    match insunits {
        1 => 24.0,
        2 => 2.0,
        4 => 500.0,
        5 => 50.0,
        6 => 0.5,
        _ => 500.0,
    }
}

fn chunk(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    if rest.is_empty() {
        out.push(String::new());
        return out;
    }
    while !rest.is_empty() {
        let mut end = rest.len().min(200);
        while !rest.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        if end == 0 {
            break;
        }
        let (a, b) = rest.split_at(end);
        out.push(a.to_string());
        rest = b;
    }
    out
}

pub fn pack(role: &str, body: &str) -> Vec<RawTag> {
    let mut tags = vec![RawTag { code: 1001, value: APP.into() }, RawTag { code: 1000, value: role.into() }];
    for part in chunk(body) {
        tags.push(RawTag { code: 1000, value: part });
    }
    tags
}

pub fn unpack(tags: &[RawTag]) -> Option<(String, String)> {
    let start = tags.iter().position(|t| t.code == 1001 && t.value.trim().eq_ignore_ascii_case(APP))?;
    let rest = tags.get(start + 1..)?;
    let end = rest.iter().position(|t| t.code == 1001).unwrap_or(rest.len());
    let group = rest.get(..end)?;
    let mut strings = group.iter().filter(|t| t.code == 1000);
    let role = strings.next()?.value.clone();
    let mut body = String::new();
    for t in strings {
        body.push_str(&t.value);
    }
    Some((role, body))
}

pub fn pack_cable(c: &Cable) -> Vec<RawTag> {
    pack("cable", &serde_json::to_string(c).unwrap_or_default())
}

pub fn pack_symbol(s: &SymbolRec) -> Vec<RawTag> {
    pack("symbol", &serde_json::to_string(s).unwrap_or_default())
}

pub fn pack_supply(s: &Supply) -> Vec<RawTag> {
    pack("supply", &serde_json::to_string(s).unwrap_or_default())
}

pub fn read_cable(tags: &[RawTag]) -> Option<Cable> {
    let (role, body) = unpack(tags)?;
    if role != "cable" {
        return None;
    }
    serde_json::from_str(&body).ok()
}

pub fn read_symbol(tags: &[RawTag]) -> Option<SymbolRec> {
    let (role, body) = unpack(tags)?;
    if role != "symbol" {
        return None;
    }
    serde_json::from_str(&body).ok()
}

pub fn read_supply(tags: &[RawTag]) -> Option<Supply> {
    let (role, body) = unpack(tags)?;
    if role != "supply" {
        return None;
    }
    serde_json::from_str(&body).ok()
}

pub fn length_m(cable: &Cable, drawn_units: Option<f64>, insunits: i64) -> f64 {
    if cable.length_mode == "draw"
        && let Some(units) = drawn_units.filter(|u| u.is_finite() && *u > 0.0)
    {
        return units * metres_per_unit(insunits);
    }
    if cable.length_m.is_finite() && cable.length_m > 0.0 { cable.length_m } else { 0.0 }
}

pub fn to_input(cable: &Cable, length: f64, up: Z) -> Input {
    let loaded = cable.loaded.parse::<i32>().unwrap_or(2);
    let sidelob = cable.sidelob.parse::<i32>().unwrap_or(1);
    let lock = if cable.section_mode == "manual" { Some(cable.section) } else { None };
    Input {
        i: cable.current,
        l: length,
        u: if cable.voltage.is_finite() && cable.voltage > 0.0 {
            cable.voltage
        } else if cable.phase == "3" {
            400.0
        } else {
            230.0
        },
        drop_pct: cable.drop_pct,
        phase: cable.phase.clone(),
        loaded,
        material: cable.material.clone(),
        brand: cable.brand.clone(),
        method: cable.method.clone(),
        temp: cable.temp,
        sidelob,
        sidelob_type: cable.sidelob_type.clone(),
        cosphi: cable.cosphi,
        in_amp: cable.in_amp.parse().ok(),
        breaker_type: cable.breaker_type.clone(),
        system_type: cable.system_type.clone(),
        section_lock: lock,
        min_section: minimum_section(cable),
        up,
    }
}

pub fn used_section(cable: &Cable, length: f64) -> f64 {
    if cable.section_mode == "manual" && cable.section > 0.0 {
        return cable.section;
    }
    let preview = calc::analyze(&to_input(cable, length, Z::ZERO));
    if preview.ok { preview.section } else { cable.section }
}

pub fn analyze_cable(cable: &Cable, length: f64, up: Z) -> Output {
    calc::analyze(&to_input(cable, length, up))
}

/// The handle hex stored as a parent, if it parses.
pub fn parent_handle(cable: &Cable) -> Option<Handle> {
    cable.parent.as_deref().and_then(Handle::parse_hex)
}
