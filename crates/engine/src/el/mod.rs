//! Electrical drawings: installation symbols, cables and the El-beregner model.
//!
//! Symbols and cables are ordinary CADCraft entities (blocks and polylines) on
//! the current drawing. The calculation matches sydsjel.dk/el.

pub mod calc;
pub mod house;
pub mod model;
pub mod place;
pub mod symbols;

use std::sync::Mutex;

static ARMED: Mutex<Option<String>> = Mutex::new(None);

/// Remember which symbol the next `el.symbol` prompt should insert.
pub fn arm(kind: &str) {
    if let Ok(mut slot) = ARMED.lock() {
        *slot = Some(kind.to_string());
    }
}

pub fn take_armed() -> Option<String> {
    ARMED.lock().ok().and_then(|mut slot| slot.take())
}
