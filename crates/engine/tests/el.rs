//! Electrical drawings: symbols, cables and the El-beregner model.
//!
//! 5G1,5 locks the cross-section. It does not change phase, voltage or the
//! number of loaded conductors. A single-phase circuit stays at 230 V.

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

#[test]
fn cable_type_sets_section_and_leaves_the_circuit() {
    for text in ["3G1,5", "3G1.5", "3g1,5", "3 G 1,5"] {
        assert_eq!(parsed(text), (3, 1.5), "{text}");
    }
    for text in ["5G1,5", "5G1.5", "5x1,5", "5X1.5", "5 x 1.5"] {
        assert_eq!(parsed(text), (5, 1.5), "{text}");
    }
    assert_eq!(parsed("4G6"), (4, 6.0));
    assert!(matches!(model::parse_cable_designation(""), Some(Designation::Empty)));
    assert!(matches!(model::parse_cable_designation("   "), Some(Designation::Empty)));
    assert!(model::parse_cable_designation("kabel").is_none());
    assert!(model::parse_cable_designation("5G").is_none());
    assert!(model::parse_cable_designation("G1,5").is_none());

    let mut cable = Cable::new(1);
    cable.set_phase("1", false);
    cable.loaded = "2".into();
    cable.apply_designation("5G1,5").unwrap();
    assert_eq!(
        (cable.phase.as_str(), cable.voltage, cable.loaded.as_str(), cable.section, cable.section_mode.as_str(), cable.cores),
        ("1", 230.0, "2", 1.5, "manual", Some(5))
    );

    cable.set_phase("3", false);
    cable.apply_designation("5G1,5").unwrap();
    assert_eq!((cable.phase.as_str(), cable.voltage, cable.loaded.as_str(), cable.section), ("3", 400.0, "2", 1.5));

    cable.loaded = "4".into();
    cable.apply_designation("5G1.5").unwrap();
    assert_eq!(cable.loaded, "4");
    assert!((cable.voltage - 400.0).abs() < 1e-9);

    cable.apply_designation("4G6").unwrap();
    assert_eq!((cable.section, cable.cores), (6.0, Some(4)));
    assert!(cable.apply_designation("5G7").is_err(), "7 mm² is not a copper section");
    assert_eq!((cable.designation.as_str(), cable.section, cable.cores, cable.phase.as_str(), cable.voltage), ("4G6", 6.0, Some(4), "3", 400.0));

    cable.material = "al".into();
    assert!(cable.apply_designation("5G1,5").is_err(), "1.5 mm² is not in the aluminium list");
    assert_eq!(cable.section, 6.0);

    cable.apply_designation("").unwrap();
    assert!(cable.designation.is_empty());
    assert_eq!(cable.section_mode, "manual");
    assert_eq!(cable.section, 6.0);
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
        up: Z::ZERO,
    });
    assert!(out.ok, "{}", out.error);
    assert!((out.section - 1.5).abs() < 1e-9);
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

#[test]
fn house_symbol_cable_and_5g15_stay_at_230v() {
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
    let set = s.execute("el.set", &json!({"handle": handle, "designation": "5G1,5"})).unwrap();
    assert_eq!(set["phase"], json!("1"), "{set}");
    assert_eq!(set["voltage"], json!(230.0), "{set}");
    assert_eq!(set["loaded"], json!("2"), "{set}");
    assert_eq!(set["section"], json!(1.5), "{set}");
    assert_eq!(set["sectionMode"], json!("manual"), "{set}");
    assert_eq!(set["cores"], json!(5), "{set}");

    s.execute("el.set", &json!({"handle": handle, "lengthMode": "manual", "lengthM": 25.0})).unwrap();
    let state = s.execute("el.state", &json!({})).unwrap();
    let row = state["cables"].as_array().unwrap().iter().find(|c| c["handle"] == handle).unwrap();
    let result = &row["result"];
    assert_eq!(result["ok"], json!(true), "{result}");
    assert_eq!(result["U"], json!(230.0), "{result}");
    assert_eq!(result["phase"], json!("1"), "{result}");
    assert_eq!(result["nLoaded"], json!(2), "{result}");
    assert_eq!(result["section"], json!(1.5), "{result}");
    assert!(result["locked"].as_bool().unwrap());
    let ik = result["Ikmax"].as_f64().unwrap();
    let drop = result["dropPct"].as_f64().unwrap();
    let iz = result["Iz"].as_f64().unwrap();
    assert!(ik.is_finite() && (ik - 760.3).abs() < 0.5, "Ikmax {ik}");
    assert!(drop.is_finite() && (drop - 3.804).abs() < 0.02, "ΔU {drop}");
    assert!((iz - 15.0).abs() < 1e-9, "Iz {iz}");
    assert_eq!(row["cores"], json!(5));
    assert_eq!(row["phase"], json!("1"));
    assert_eq!(row["voltage"], json!(230.0));

    let three = s.execute("el.set", &json!({"handle": handle, "phase": "3"})).unwrap();
    assert_eq!((three["phase"].as_str(), three["voltage"].as_f64()), (Some("3"), Some(400.0)));
    let still = s.execute("el.set", &json!({"handle": handle, "designation": "5G1,5"})).unwrap();
    assert_eq!(still["phase"], json!("3"));
    assert_eq!(still["voltage"], json!(400.0));
    assert_eq!(still["loaded"], json!("2"));
    assert_eq!(still["section"], json!(1.5));

    s.execute("el.set", &json!({"handle": handle, "loaded": "4"})).unwrap();
    let kept = s.execute("el.set", &json!({"handle": handle, "designation": "5G1.5"})).unwrap();
    assert_eq!(kept["loaded"], json!("4"));
    assert_eq!(kept["voltage"], json!(400.0));
    assert_eq!(kept["phase"], json!("3"));

    s.execute("el.set", &json!({"handle": handle, "designation": "4G6"})).unwrap();
    let err = s.execute("el.set", &json!({"handle": handle, "designation": "5G7"})).unwrap_err();
    assert!(err.to_string().contains("mm"), "{err}");
    let state = s.execute("el.state", &json!({})).unwrap();
    let row = state["cables"].as_array().unwrap().iter().find(|c| c["handle"] == handle).unwrap();
    assert_eq!(row["designation"], json!("4G6"));
    assert_eq!(row["section"], json!(6.0));
    assert_eq!(row["cores"], json!(4));
    assert_eq!(row["phase"], json!("3"));
    assert_eq!(row["voltage"], json!(400.0));
    assert_eq!(row["loaded"], json!("4"));
}
