//! Electrical drawings: symbols, cables and the El-beregner model.
//!
//! The number after G or X is the minimum section for every core count.
//! 3G1,5, 5G1,5 and 7G1,5 all floor at 1,5 mm². None of them lock the section
//! or change phase, voltage or the number of loaded conductors.
//! 1-fase stays at 230 V. 3-fase stays at 400 V.

use cadcraft_engine::Session;
use cadcraft_engine::el::calc::{self, Input, Z};
use cadcraft_engine::el::model::{self, Cable, Designation};
use serde_json::json;

fn parsed(text: &str) -> (u32, f64) {
    match model::parse_cable_designation(text) {
        Some(Designation::Parsed { cores, section, .. }) => (cores, section),
        other => panic!("{text} should parse, got {other:?}"),
    }
}

fn assert_circuit(cable: &Cable, phase: &str, voltage: f64, loaded: &str, cores: u32, minimum: f64) {
    assert_eq!(cable.phase, phase);
    assert!((cable.voltage - voltage).abs() < 1e-9, "voltage {}", cable.voltage);
    assert_eq!(cable.loaded, loaded);
    assert_eq!(cable.cores, Some(cores));
    assert_eq!(cable.section_mode, "auto");
    assert!((cable.section - minimum).abs() < 1e-9, "stored minimum {}", cable.section);
    assert_eq!(model::minimum_section(cable), Some(minimum));
}

#[test]
fn cable_type_sets_a_minimum_and_leaves_the_circuit() {
    for text in ["3G1,5", "3G1.5", "3g1,5", "3 G 1,5"] {
        assert_eq!(parsed(text), (3, 1.5), "{text}");
    }
    for text in ["5G1,5", "5G1.5", "5x1,5", "5X1.5", "5 x 1.5"] {
        assert_eq!(parsed(text), (5, 1.5), "{text}");
    }
    for text in ["7G1,5", "7G1.5", "7g1,5", "7 x 1,5"] {
        assert_eq!(parsed(text), (7, 1.5), "{text}");
    }
    assert_eq!(parsed("4G2,5"), (4, 2.5));
    assert_eq!(parsed("4G2.5"), (4, 2.5));
    assert_eq!(parsed("4G6"), (4, 6.0));
    assert!(matches!(model::parse_cable_designation(""), Some(Designation::Empty)));
    assert!(matches!(model::parse_cable_designation("   "), Some(Designation::Empty)));
    assert!(model::parse_cable_designation("kabel").is_none());
    assert!(model::parse_cable_designation("5G").is_none());
    assert!(model::parse_cable_designation("G1,5").is_none());

    let mut cable = Cable::new(1);
    cable.set_phase("1", false);
    cable.loaded = "2".into();
    for (text, cores) in [("3G1,5", 3), ("7G1,5", 7), ("5G1,5", 5), ("3G1.5", 3), ("7G1.5", 7)] {
        cable.apply_designation(text).unwrap();
        assert_circuit(&cable, "1", 230.0, "2", cores, 1.5);
    }

    cable.set_phase("3", false);
    cable.apply_designation("7G1,5").unwrap();
    assert_circuit(&cable, "3", 400.0, "2", 7, 1.5);
    cable.apply_designation("3G1,5").unwrap();
    assert_circuit(&cable, "3", 400.0, "2", 3, 1.5);

    cable.loaded = "4".into();
    cable.apply_designation("3G1.5").unwrap();
    assert_circuit(&cable, "3", 400.0, "4", 3, 1.5);
    cable.apply_designation("7G1.5").unwrap();
    assert_circuit(&cable, "3", 400.0, "4", 7, 1.5);

    cable.apply_designation("4G2,5").unwrap();
    assert_circuit(&cable, "3", 400.0, "4", 4, 2.5);

    cable.apply_designation("4G6").unwrap();
    assert_eq!((cable.section, cable.cores, cable.section_mode.as_str()), (6.0, Some(4), "auto"));
    assert!(cable.apply_designation("3G7").is_err(), "7 mm² is not a copper section");
    assert!(cable.apply_designation("7G7").is_err(), "7 mm² is not a copper section");
    assert_eq!(
        (cable.designation.as_str(), cable.section, cable.cores, cable.phase.as_str(), cable.voltage, cable.section_mode.as_str()),
        ("4G6", 6.0, Some(4), "3", 400.0, "auto")
    );

    cable.material = "al".into();
    assert!(cable.apply_designation("3G1,5").is_err(), "1.5 mm² is not in the aluminium list");
    assert!(cable.apply_designation("7G1,5").is_err(), "1.5 mm² is not in the aluminium list");
    assert_eq!(cable.section, 6.0);
    assert_eq!(cable.designation, "4G6");

    cable.apply_designation("").unwrap();
    assert!(cable.designation.is_empty());
    assert_eq!(cable.cores, None);
    assert_eq!(cable.section_mode, "auto");
    assert_eq!(model::minimum_section(&cable), None);
    assert_eq!(cable.section, 6.0);
    assert_eq!((cable.phase.as_str(), cable.voltage, cable.loaded.as_str()), ("3", 400.0, "4"));
}

#[test]
fn analyze_matches_the_el_beregner() {
    let out = calc::analyze(&Input {
        i: 16.0,
        l: 25.0,
        u: 230.0,
        drop_pct: 3.0,
        phase: "1".into(),
        loaded: 2,
        material: "cu".into(),
        brand: "nkt".into(),
        method: "B2".into(),
        temp: 30.0,
        sidelob: 1,
        sidelob_type: "bundt".into(),
        cosphi: 0.9,
        in_amp: Some(16),
        breaker_type: "C".into(),
        system_type: "TN".into(),
        section_lock: Some(1.5),
        min_section: None,
        up: Z::ZERO,
    });
    assert!(out.ok, "{}", out.error);
    assert!((out.section - 1.5).abs() < 1e-9);
    assert!(out.locked);
    assert!(out.min_section.is_none());
    assert!((out.drop_v - 8.750).abs() < 0.01, "ΔU {} V", out.drop_v);
    assert!((out.drop_pct - 3.804).abs() < 0.01, "ΔU {} %", out.drop_pct);
    assert_eq!(out.corrected_capacity, Some(15.0));
    assert!(!out.current_ok, "15 A Iz does not cover 16 A");
    assert!((out.ikmax - 760.3).abs() < 0.5, "Ikmax {}", out.ikmax);
    assert!((out.ikmin - 288.9).abs() < 0.5, "Ikmin {}", out.ikmin);
    assert_eq!(out.u, 230.0);
    assert_eq!(out.n_loaded, 2);
    assert_eq!(out.phase, "1");
}

fn section_of(input: Input) -> f64 {
    let out = calc::analyze(&input);
    assert!(out.ok, "{}", out.error);
    out.section
}

#[test]
fn every_designation_is_a_floor_and_a_long_run_may_rise() {
    let base = Input::default();
    let open = section_of(Input { i: 10.0, l: 10.0, min_section: None, ..base.clone() });
    assert!((open - 1.5).abs() < 1e-9, "no designation picks 1.5, got {open}");

    let held = calc::analyze(&Input { i: 10.0, l: 10.0, min_section: Some(2.5), ..base.clone() });
    assert!((held.section - 2.5).abs() < 1e-9, "4G2,5 stays at 2.5, got {}", held.section);
    assert!(!held.locked);
    assert_eq!(held.min_section, Some(2.5));
    assert!(held.drop_ok && held.current_ok);

    // 16 A, 230 V, 3 %: 1.5 mm² fails both ΔU and Iz, so the floor of 1.5 is not the result.
    let at = |metres: f64| section_of(Input { l: metres, min_section: Some(1.5), ..base.clone() });
    assert!((at(25.0) - 2.5).abs() < 1e-9, "25 m → {}", at(25.0));
    assert!((at(40.0) - 4.0).abs() < 1e-9, "40 m → {}", at(40.0));
    assert!((at(60.0) - 6.0).abs() < 1e-9, "60 m → {}", at(60.0));
    assert!((at(80.0) - 10.0).abs() < 1e-9, "80 m → {}", at(80.0));

    let mut cable = Cable::default();
    for text in ["3G1,5", "3G1.5", "7G1,5", "7G1.5", "5G1,5"] {
        cable.apply_designation(text).unwrap();
        let out = model::analyze_cable(&cable, 80.0, Z::ZERO);
        assert!((out.section - 10.0).abs() < 1e-9, "{text} at 80 m chose {}", out.section);
        assert!(!out.locked, "{text} must not lock");
        assert_eq!(out.min_section, Some(1.5), "{text}");
        assert_eq!(out.phase, "1");
        assert!((out.u - 230.0).abs() < 1e-9);
        assert_eq!(out.n_loaded, 2);
        assert!(out.drop_ok && out.current_ok, "{text}");
    }

    cable.apply_designation("4G2,5").unwrap();
    cable.current = 10.0;
    let held_cable = model::analyze_cable(&cable, 10.0, Z::ZERO);
    assert!((held_cable.section - 2.5).abs() < 1e-9, "{}", held_cable.section);
    assert_eq!(cable.cores, Some(4));
    assert!((cable.voltage - 230.0).abs() < 1e-9);

    cable.apply_designation("").unwrap();
    let cleared = model::analyze_cable(&cable, 10.0, Z::ZERO);
    assert!((cleared.section - 1.5).abs() < 1e-9, "cleared designation has no floor, got {}", cleared.section);
    assert!(cleared.min_section.is_none());
    assert_eq!(cable.phase, "1");
    assert!((cable.voltage - 230.0).abs() < 1e-9);
}

#[test]
fn house_symbol_cable_and_designation_stay_at_the_circuit_voltage() {
    let mut s = Session::new();
    let house = s.execute("el.house", &json!({})).unwrap();
    assert!(house["entities"].as_u64().unwrap_or(0) > 0, "{house}");

    let symbol = s.execute("el.symbol", &json!({"kind": "stik", "at": [100.0, 100.0]})).unwrap();
    assert!(symbol["handle"].as_str().is_some(), "{symbol}");
    for kind in ["afbryder", "lampe", "tavle", "stik2", "korr", "kryds", "udtag", "ude", "daase", "kraft", "komfur", "tekst"] {
        s.execute("el.symbol", &json!({"kind": kind, "at": [120.0, 80.0]})).unwrap_or_else(|e| panic!("{kind}: {e}"));
    }

    let cable = s.execute("el.cable", &json!({"points": [[100.0, 100.0], [100.0, 220.0]]})).unwrap();
    let handle = cable["handle"].as_str().unwrap();
    for (text, cores) in [("3G1,5", 3), ("7G1,5", 7), ("3G1.5", 3), ("7G1.5", 7)] {
        let set = s.execute("el.set", &json!({"handle": handle, "designation": text})).unwrap();
        assert_eq!(set["phase"], json!("1"), "{text} {set}");
        assert_eq!(set["voltage"], json!(230.0), "{text} {set}");
        assert_eq!(set["loaded"], json!("2"), "{text} {set}");
        assert_eq!(set["section"], json!(1.5), "{text} {set}");
        assert_eq!(set["sectionMode"], json!("auto"), "{text} {set}");
        assert_eq!(set["cores"], json!(cores), "{text} {set}");
    }

    s.execute("el.set", &json!({"handle": handle, "designation": "3G1,5", "lengthMode": "manual", "lengthM": 80.0})).unwrap();
    let row = cable_row(&mut s, handle);
    let result = &row["result"];
    assert_eq!(result["ok"], json!(true), "{result}");
    assert_eq!(result["U"], json!(230.0), "{result}");
    assert_eq!(result["phase"], json!("1"), "{result}");
    assert_eq!(result["nLoaded"], json!(2), "{result}");
    assert_eq!(result["section"], json!(10.0), "{result}");
    assert_eq!(result["minSection"], json!(1.5), "{result}");
    assert_eq!(result["locked"], json!(false), "{result}");
    assert_eq!(result["currentOk"], json!(true), "{result}");
    assert_eq!(result["dropOk"], json!(true), "{result}");
    assert_eq!(row["cores"], json!(3));
    assert_eq!(row["minSection"], json!(1.5));
    assert_eq!(row["phase"], json!("1"));
    assert_eq!(row["voltage"], json!(230.0));

    s.execute("el.set", &json!({"handle": handle, "designation": "7G1,5"})).unwrap();
    let row = cable_row(&mut s, handle);
    assert_eq!(row["cores"], json!(7));
    assert_eq!(row["result"]["section"], json!(10.0), "{}", row["result"]);
    assert_eq!(row["phase"], json!("1"));
    assert_eq!(row["voltage"], json!(230.0));
    assert_eq!(row["sectionMode"], json!("auto"));

    s.execute("el.set", &json!({"handle": handle, "lengthM": 25.0})).unwrap();
    let row = cable_row(&mut s, handle);
    assert_eq!(row["result"]["section"], json!(2.5), "25 m rises above 1.5: {}", row["result"]);
    assert_eq!(row["result"]["locked"], json!(false));

    s.execute("el.set", &json!({"handle": handle, "designation": "4G2,5", "I": 10.0, "lengthM": 10.0})).unwrap();
    let row = cable_row(&mut s, handle);
    assert_eq!(row["cores"], json!(4));
    assert_eq!(row["result"]["section"], json!(2.5), "4G2,5 does not drop to 1.5: {}", row["result"]);
    assert_eq!(row["voltage"], json!(230.0));

    s.execute("el.set", &json!({"handle": handle, "designation": ""})).unwrap();
    let row = cable_row(&mut s, handle);
    assert_eq!(row["designation"], json!(""));
    assert_eq!(row["cores"], json!(null));
    assert_eq!(row["minSection"], json!(null));
    assert_eq!(row["result"]["section"], json!(1.5), "no designation has no extra floor: {}", row["result"]);
    assert_eq!(row["voltage"], json!(230.0));

    let three = s.execute("el.set", &json!({"handle": handle, "phase": "3"})).unwrap();
    assert_eq!((three["phase"].as_str(), three["voltage"].as_f64()), (Some("3"), Some(400.0)));
    let still = s.execute("el.set", &json!({"handle": handle, "designation": "7G1,5"})).unwrap();
    assert_eq!(still["phase"], json!("3"));
    assert_eq!(still["voltage"], json!(400.0));
    assert_eq!(still["loaded"], json!("2"));
    assert_eq!(still["sectionMode"], json!("auto"));
    assert_eq!(still["cores"], json!(7));

    s.execute("el.set", &json!({"handle": handle, "loaded": "4"})).unwrap();
    let kept = s.execute("el.set", &json!({"handle": handle, "designation": "3G1.5"})).unwrap();
    assert_eq!(kept["loaded"], json!("4"));
    assert_eq!(kept["voltage"], json!(400.0));
    assert_eq!(kept["phase"], json!("3"));
    assert_eq!(kept["cores"], json!(3));
    assert_eq!(kept["sectionMode"], json!("auto"));

    s.execute("el.set", &json!({"handle": handle, "designation": "4G6"})).unwrap();
    let err = s.execute("el.set", &json!({"handle": handle, "designation": "3G7"})).unwrap_err();
    assert!(err.to_string().contains("mm"), "{err}");
    let err = s.execute("el.set", &json!({"handle": handle, "designation": "7G7"})).unwrap_err();
    assert!(err.to_string().contains("mm"), "{err}");
    let row = cable_row(&mut s, handle);
    assert_eq!(row["designation"], json!("4G6"));
    assert_eq!(row["section"], json!(6.0));
    assert_eq!(row["cores"], json!(4));
    assert_eq!(row["phase"], json!("3"));
    assert_eq!(row["voltage"], json!(400.0));
    assert_eq!(row["loaded"], json!("4"));
    assert_eq!(row["sectionMode"], json!("auto"));
    let chosen = row["result"]["section"].as_f64().unwrap();
    assert!(chosen + 1e-9 >= 6.0, "4G6 never goes below 6, got {chosen}");
}

#[test]
fn typed_ib_13_is_what_the_calculation_uses() {
    let mut s = Session::new();
    let cable = s.execute("el.cable", &json!({"points": [[0.0, 0.0], [10.0, 0.0]]})).unwrap();
    let handle = cable["handle"].as_str().unwrap().to_string();
    let before = cable_row(&mut s, &handle);
    assert!((before["I"].as_f64().unwrap() - 16.0).abs() < 1e-9, "a new cable starts at 16 A");

    s.execute(
        "el.set",
        &json!({
            "handle": handle,
            "I": 13.0,
            "lengthMode": "manual",
            "lengthM": 24.88,
            "phase": "1",
            "voltage": 230.0,
            "loaded": "2",
            "cosphi": 0.9,
            "dropPct": 3.0,
            "sectionMode": "manual",
            "section": 2.5,
        }),
    )
    .unwrap();

    let row = cable_row(&mut s, &handle);
    assert!((row["I"].as_f64().unwrap() - 13.0).abs() < 1e-9, "Ib did not stay at 13 A: {row}");
    assert!((row["lengthM"].as_f64().unwrap() - 24.88).abs() < 1e-9, "{row}");
    assert_eq!(row["phase"], json!("1"));
    assert!((row["voltage"].as_f64().unwrap() - 230.0).abs() < 1e-9);
    assert!((row["cosphi"].as_f64().unwrap() - 0.9).abs() < 1e-9);
    assert!((row["dropPctLimit"].as_f64().unwrap() - 3.0).abs() < 1e-9);
    let result = &row["result"];
    assert!((result["I"].as_f64().unwrap() - 13.0).abs() < 1e-9, "analyze did not use 13 A: {result}");
    assert!((result["L"].as_f64().unwrap() - 24.88).abs() < 1e-9, "{result}");
    assert!((result["section"].as_f64().unwrap() - 2.5).abs() < 1e-9, "{result}");
    assert_eq!(result["phase"], json!("1"));
    assert!((result["U"].as_f64().unwrap() - 230.0).abs() < 1e-9);
    let drop_v = result["dropV"].as_f64().unwrap();
    let drop_pct = result["dropPct"].as_f64().unwrap();
    assert!((drop_v - 4.34).abs() < 0.02, "13 A at 24.88 m, 2.5 mm² should be about 4.34 V, got {drop_v}");
    assert!((drop_pct - 1.89).abs() < 0.02, "13 A should be about 1.89 %, got {drop_pct}");

    let sixteen = calc::analyze(&Input { i: 16.0, l: 24.88, section_lock: Some(2.5), ..Input::default() });
    assert!((sixteen.drop_v - 5.34).abs() < 0.02, "16 A reference drop {}", sixteen.drop_v);
    assert!((drop_v - sixteen.drop_v).abs() > 0.5, "13 A drop {drop_v} V matched the 16 A drop {} V", sixteen.drop_v);

    s.execute("el.set", &json!({"handle": handle, "designation": "3G1,5"})).unwrap();
    let again = cable_row(&mut s, &handle);
    assert!((again["I"].as_f64().unwrap() - 13.0).abs() < 1e-9, "a later edit put Ib back to 16 A");
    assert!((again["result"]["I"].as_f64().unwrap() - 13.0).abs() < 1e-9, "{again}");
    assert!((again["voltage"].as_f64().unwrap() - 230.0).abs() < 1e-9);
}

fn cable_row(s: &mut Session, handle: &str) -> serde_json::Value {
    let state = s.execute("el.state", &json!({})).unwrap();
    state["cables"].as_array().unwrap().iter().find(|c| c["handle"] == handle).unwrap().clone()
}
