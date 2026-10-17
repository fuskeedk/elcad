//! Cable calculation ported from the El-beregner (`calc.js` on sydsjel.dk/el).
//!
//! Same NKT R1/X1/R0/X0, the same Iz tables and the same Ik formulas, so a cable
//! with the same input produces the same numbers.
//!
//! Ikmax = U0 / Z1
//! Ikmin = 3 · U0 · 0.95 / |1.5·(R1+R0) + j(X1+X0)|
//! with R and X = impedance ahead of the cable + the cable at 20 °C.

/// Positive-sequence and zero-sequence impedance, ohms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Z {
    pub r1: f64,
    pub x1: f64,
    pub r0: f64,
    pub x0: f64,
}

impl Z {
    pub const ZERO: Z = Z { r1: 0.0, x1: 0.0, r0: 0.0, x0: 0.0 };
}

impl std::ops::Add for Z {
    type Output = Self;

    fn add(self, o: Self) -> Self {
        Z { r1: self.r1 + o.r1, x1: self.x1 + o.x1, r0: self.r0 + o.r0, x0: self.x0 + o.x0 }
    }
}

#[derive(Clone, Debug)]
pub struct Input {
    pub i: f64,
    pub l: f64,
    pub u: f64,
    pub drop_pct: f64,
    /// `"3"` is three-phase (line voltage). Anything else is single-phase.
    pub phase: String,
    pub loaded: i32,
    pub material: String,
    pub brand: String,
    pub method: String,
    pub temp: f64,
    pub sidelob: i32,
    pub sidelob_type: String,
    pub cosphi: f64,
    pub in_amp: Option<i32>,
    pub breaker_type: String,
    pub system_type: String,
    /// Exact section from the lock checkbox. A cable designation does not set this.
    pub section_lock: Option<f64>,
    /// Smallest section the designation allows. `None` when there is no designation.
    /// Auto selection never goes below it. Any core count uses the same floor.
    pub min_section: Option<f64>,
    pub up: Z,
}

impl Default for Input {
    fn default() -> Self {
        Input {
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
            section_lock: None,
            min_section: None,
            up: Z::ZERO,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Output {
    pub ok: bool,
    pub error: String,
    pub i: f64,
    pub l: f64,
    pub u: f64,
    pub u0: f64,
    pub phase: String,
    pub material: String,
    pub material_label: String,
    pub brand: String,
    pub brand_label: String,
    pub method: String,
    pub cosphi: f64,
    pub min_drop_section: f64,
    pub recommended: f64,
    pub recommended_meets: bool,
    pub section: f64,
    pub locked: bool,
    /// Listed section the designation will not go below. `None` without a designation.
    pub min_section: Option<f64>,
    pub capacity: Option<f64>,
    pub corrected_capacity: Option<f64>,
    pub temp_used: f64,
    pub temp_factor: f64,
    pub temp_ref: String,
    pub group_factor: f64,
    pub loaded_factor: f64,
    pub n_loaded: i32,
    pub n_sidelob: i32,
    pub drop_v: f64,
    pub drop_pct: f64,
    pub drop_limit_pct: f64,
    pub drop_ok: bool,
    pub current_ok: bool,
    pub cable_z: Option<Z>,
    pub ikmax: f64,
    pub ikmin: f64,
    pub ikmax_start: f64,
    pub ikmin_start: f64,
    pub pe_section: f64,
    pub pe_required: f64,
    pub in_amp: Option<i32>,
    pub breaker_type: String,
    pub is_fuse: bool,
    pub system_type: String,
    pub t_max: f64,
    pub ia: Option<f64>,
    pub over_ok: Option<bool>,
    pub earth_ok: Option<bool>,
    pub short_ok: Option<bool>,
    pub t_adiabatic: Option<f64>,
    pub fuse_note: String,
}

impl Default for Output {
    fn default() -> Self {
        Output {
            ok: false,
            error: String::new(),
            i: 0.0,
            l: 0.0,
            u: 0.0,
            u0: 0.0,
            phase: String::new(),
            material: String::new(),
            material_label: String::new(),
            brand: String::new(),
            brand_label: String::new(),
            method: String::new(),
            cosphi: 0.0,
            min_drop_section: 0.0,
            recommended: 0.0,
            recommended_meets: false,
            section: 0.0,
            locked: false,
            min_section: None,
            capacity: None,
            corrected_capacity: None,
            temp_used: 0.0,
            temp_factor: 1.0,
            temp_ref: String::new(),
            group_factor: 1.0,
            loaded_factor: 1.0,
            n_loaded: 2,
            n_sidelob: 1,
            drop_v: 0.0,
            drop_pct: 0.0,
            drop_limit_pct: 0.0,
            drop_ok: false,
            current_ok: false,
            cable_z: None,
            ikmax: 0.0,
            ikmin: 0.0,
            ikmax_start: 0.0,
            ikmin_start: 0.0,
            pe_section: 0.0,
            pe_required: 0.0,
            in_amp: None,
            breaker_type: String::new(),
            is_fuse: false,
            system_type: String::new(),
            t_max: 0.4,
            ia: None,
            over_ok: None,
            earth_ok: None,
            short_ok: None,
            t_adiabatic: None,
            fuse_note: String::new(),
        }
    }
}

pub const SECTIONS_CU: &[f64] = &[1.5, 2.5, 4.0, 6.0, 10.0, 16.0, 25.0, 35.0, 50.0, 70.0, 95.0, 120.0, 150.0, 185.0, 240.0, 300.0];
pub const SECTIONS_AL: &[f64] = &[16.0, 25.0, 35.0, 50.0, 70.0, 95.0, 120.0, 150.0, 185.0, 240.0, 300.0];
const PE_SECTIONS: &[f64] = SECTIONS_CU;
pub const STANDARD_IN: &[i32] = &[6, 10, 16, 20, 25, 32, 35, 40, 50, 63, 80, 100, 125, 160, 200, 250];

pub fn sections_for(material: &str) -> &'static [f64] {
    if material == "al" { SECTIONS_AL } else { SECTIONS_CU }
}

fn lookup(table: &[(f64, f64)], section: f64) -> Option<f64> {
    table.iter().find(|(s, _)| (*s - section).abs() < 1e-6).map(|(_, v)| *v)
}

fn r1_table(al: bool) -> &'static [(f64, f64)] {
    if al {
        &[
            (16.0, 1.91),
            (25.0, 1.2),
            (35.0, 0.868),
            (50.0, 0.641),
            (70.0, 0.444),
            (95.0, 0.321),
            (120.0, 0.254),
            (150.0, 0.207),
            (185.0, 0.166),
            (240.0, 0.127),
            (300.0, 0.103),
        ]
    } else {
        &[
            (1.5, 12.1),
            (2.5, 7.41),
            (4.0, 4.61),
            (6.0, 3.08),
            (10.0, 1.83),
            (16.0, 1.15),
            (25.0, 0.727),
            (35.0, 0.525),
            (50.0, 0.388),
            (70.0, 0.269),
            (95.0, 0.194),
            (120.0, 0.155),
            (150.0, 0.126),
            (185.0, 0.1017),
            (240.0, 0.0787),
            (300.0, 0.067),
        ]
    }
}

fn x1_table(al: bool) -> &'static [(f64, f64)] {
    if al {
        &[
            (16.0, 0.089),
            (25.0, 0.084),
            (35.0, 0.082),
            (50.0, 0.081),
            (70.0, 0.081),
            (95.0, 0.078),
            (120.0, 0.077),
            (150.0, 0.077),
            (185.0, 0.077),
            (240.0, 0.077),
            (300.0, 0.077),
        ]
    } else {
        &[
            (1.5, 0.11),
            (2.5, 0.102),
            (4.0, 0.096),
            (6.0, 0.094),
            (10.0, 0.089),
            (16.0, 0.085),
            (25.0, 0.086),
            (35.0, 0.082),
            (50.0, 0.084),
            (70.0, 0.081),
            (95.0, 0.082),
            (120.0, 0.082),
            (150.0, 0.084),
            (185.0, 0.082),
            (240.0, 0.083),
            (300.0, 0.077),
        ]
    }
}

fn r0x0_table(al: bool) -> &'static [(f64, f64, f64)] {
    if al {
        &[
            (16.0, 7.64, 0.334),
            (25.0, 4.8, 0.338),
            (35.0, 3.473, 0.326),
            (50.0, 2.565, 0.324),
            (70.0, 1.774, 0.319),
            (95.0, 1.283, 0.312),
            (120.0, 1.016, 0.31),
            (150.0, 0.829, 0.312),
            (185.0, 0.662, 0.313),
            (240.0, 0.508, 0.31),
            (300.0, 0.41, 0.307),
        ]
    } else {
        &[
            (1.5, 48.4, 0.44),
            (2.5, 29.64, 0.41),
            (4.0, 18.44, 0.386),
            (6.0, 12.32, 0.376),
            (10.0, 7.32, 0.357),
            (16.0, 4.6, 0.341),
            (25.0, 2.91, 0.346),
            (35.0, 2.1, 0.327),
            (50.0, 1.554, 0.335),
            (70.0, 1.088, 0.293),
            (95.0, 0.824, 0.33),
            (120.0, 0.644, 0.33),
            (150.0, 0.516, 0.335),
            (185.0, 0.424, 0.329),
            (240.0, 0.32, 0.33),
            (300.0, 0.26, 0.33),
        ]
    }
}

fn nkt_cu(method: &str) -> &'static [(f64, f64)] {
    match method {
        "A1" => &[
            (1.5, 13.5),
            (2.5, 18.0),
            (4.0, 24.0),
            (6.0, 31.0),
            (10.0, 42.0),
            (16.0, 56.0),
            (25.0, 73.0),
            (35.0, 89.0),
            (50.0, 108.0),
            (70.0, 136.0),
            (95.0, 164.0),
            (120.0, 188.0),
            (150.0, 216.0),
            (185.0, 245.0),
            (240.0, 286.0),
            (300.0, 328.0),
        ],
        "A2" => &[
            (1.5, 13.0),
            (2.5, 17.5),
            (4.0, 23.0),
            (6.0, 29.0),
            (10.0, 39.0),
            (16.0, 52.0),
            (25.0, 68.0),
            (35.0, 83.0),
            (50.0, 99.0),
            (70.0, 125.0),
            (95.0, 150.0),
            (120.0, 172.0),
            (150.0, 196.0),
            (185.0, 223.0),
            (240.0, 261.0),
            (300.0, 298.0),
        ],
        "B1" => &[
            (1.5, 15.5),
            (2.5, 21.0),
            (4.0, 28.0),
            (6.0, 36.0),
            (10.0, 50.0),
            (16.0, 68.0),
            (25.0, 89.0),
            (35.0, 110.0),
            (50.0, 134.0),
            (70.0, 171.0),
            (95.0, 207.0),
            (120.0, 239.0),
            (150.0, 275.0),
            (185.0, 314.0),
            (240.0, 370.0),
            (300.0, 426.0),
        ],
        "B2" => &[
            (1.5, 15.0),
            (2.5, 20.0),
            (4.0, 27.0),
            (6.0, 34.0),
            (10.0, 46.0),
            (16.0, 62.0),
            (25.0, 80.0),
            (35.0, 99.0),
            (50.0, 118.0),
            (70.0, 149.0),
            (95.0, 179.0),
            (120.0, 206.0),
            (150.0, 236.0),
            (185.0, 268.0),
            (240.0, 313.0),
            (300.0, 358.0),
        ],
        "C" => &[
            (1.5, 17.5),
            (2.5, 24.0),
            (4.0, 32.0),
            (6.0, 41.0),
            (10.0, 57.0),
            (16.0, 76.0),
            (25.0, 96.0),
            (35.0, 119.0),
            (50.0, 144.0),
            (70.0, 184.0),
            (95.0, 223.0),
            (120.0, 259.0),
            (150.0, 299.0),
            (185.0, 341.0),
            (240.0, 403.0),
            (300.0, 464.0),
        ],
        "D" => &[
            (1.5, 18.0),
            (2.5, 24.0),
            (4.0, 31.0),
            (6.0, 39.0),
            (10.0, 52.0),
            (16.0, 67.0),
            (25.0, 86.0),
            (35.0, 103.0),
            (50.0, 122.0),
            (70.0, 151.0),
            (95.0, 179.0),
            (120.0, 203.0),
            (150.0, 230.0),
            (185.0, 258.0),
            (240.0, 297.0),
            (300.0, 336.0),
        ],
        _ => &[],
    }
}

fn nkt_al(method: &str) -> &'static [(f64, f64)] {
    match method {
        "A1" => &[
            (16.0, 43.0),
            (25.0, 57.0),
            (35.0, 70.0),
            (50.0, 84.0),
            (70.0, 107.0),
            (95.0, 129.0),
            (120.0, 149.0),
            (150.0, 170.0),
            (185.0, 194.0),
            (240.0, 227.0),
            (300.0, 261.0),
        ],
        "A2" => &[
            (16.0, 41.0),
            (25.0, 53.0),
            (35.0, 65.0),
            (50.0, 78.0),
            (70.0, 98.0),
            (95.0, 118.0),
            (120.0, 135.0),
            (150.0, 155.0),
            (185.0, 176.0),
            (240.0, 207.0),
            (300.0, 237.0),
        ],
        "B1" => &[
            (16.0, 53.0),
            (25.0, 70.0),
            (35.0, 86.0),
            (50.0, 104.0),
            (70.0, 133.0),
            (95.0, 161.0),
            (120.0, 186.0),
            (150.0, 214.0),
            (185.0, 245.0),
            (240.0, 288.0),
            (300.0, 331.0),
        ],
        "B2" => &[
            (16.0, 48.0),
            (25.0, 62.0),
            (35.0, 77.0),
            (50.0, 92.0),
            (70.0, 116.0),
            (95.0, 139.0),
            (120.0, 160.0),
            (150.0, 183.0),
            (185.0, 208.0),
            (240.0, 243.0),
            (300.0, 278.0),
        ],
        "C" => &[
            (16.0, 59.0),
            (25.0, 73.0),
            (35.0, 90.0),
            (50.0, 110.0),
            (70.0, 140.0),
            (95.0, 170.0),
            (120.0, 197.0),
            (150.0, 227.0),
            (185.0, 259.0),
            (240.0, 305.0),
            (300.0, 351.0),
        ],
        "D" => &[
            (16.0, 52.0),
            (25.0, 66.0),
            (35.0, 80.0),
            (50.0, 94.0),
            (70.0, 117.0),
            (95.0, 138.0),
            (120.0, 157.0),
            (150.0, 178.0),
            (185.0, 200.0),
            (240.0, 230.0),
            (300.0, 260.0),
        ],
        _ => &[],
    }
}

fn waskoenig_cu_air() -> &'static [(f64, f64)] {
    &[
        (1.5, 24.0),
        (2.5, 32.0),
        (4.0, 42.0),
        (6.0, 53.0),
        (10.0, 74.0),
        (16.0, 98.0),
        (25.0, 133.0),
        (35.0, 162.0),
        (50.0, 197.0),
        (70.0, 250.0),
        (95.0, 308.0),
        (120.0, 360.0),
        (150.0, 412.0),
        (185.0, 475.0),
        (240.0, 564.0),
        (300.0, 649.0),
    ]
}

fn waskoenig_cu_ground() -> &'static [(f64, f64)] {
    &[
        (1.5, 31.0),
        (2.5, 40.0),
        (4.0, 52.0),
        (6.0, 64.0),
        (10.0, 86.0),
        (16.0, 112.0),
        (25.0, 145.0),
        (35.0, 174.0),
        (50.0, 206.0),
        (70.0, 254.0),
        (95.0, 305.0),
        (120.0, 349.0),
        (150.0, 392.0),
        (185.0, 444.0),
        (240.0, 517.0),
        (300.0, 585.0),
    ]
}

fn waskoenig_al_air() -> &'static [(f64, f64)] {
    &[
        (16.0, 77.0),
        (25.0, 102.0),
        (35.0, 126.0),
        (50.0, 149.0),
        (70.0, 192.0),
        (95.0, 234.0),
        (120.0, 273.0),
        (150.0, 311.0),
        (185.0, 369.0),
        (240.0, 427.0),
        (300.0, 504.0),
    ]
}

fn waskoenig_al_ground() -> &'static [(f64, f64)] {
    &[
        (16.0, 88.0),
        (25.0, 112.0),
        (35.0, 135.0),
        (50.0, 158.0),
        (70.0, 196.0),
        (95.0, 234.0),
        (120.0, 267.0),
        (150.0, 300.0),
        (185.0, 349.0),
        (240.0, 398.0),
        (300.0, 457.0),
    ]
}

fn prysmian_cu_air() -> &'static [(f64, f64)] {
    &[
        (1.5, 26.0),
        (2.5, 36.0),
        (4.0, 42.0),
        (6.0, 54.0),
        (10.0, 75.0),
        (16.0, 100.0),
        (25.0, 127.0),
        (35.0, 158.0),
        (50.0, 192.0),
        (70.0, 246.0),
        (95.0, 298.0),
        (120.0, 383.0),
        (150.0, 399.0),
        (185.0, 456.0),
        (240.0, 538.0),
        (300.0, 703.0),
    ]
}

fn prysmian_cu_ground() -> &'static [(f64, f64)] {
    &[
        (1.5, 34.0),
        (2.5, 45.0),
        (4.0, 52.0),
        (6.0, 65.0),
        (10.0, 87.0),
        (16.0, 114.0),
        (25.0, 138.0),
        (35.0, 170.0),
        (50.0, 201.0),
        (70.0, 250.0),
        (95.0, 295.0),
        (120.0, 371.0),
        (150.0, 380.0),
        (185.0, 426.0),
        (240.0, 493.0),
        (300.0, 634.0),
    ]
}

fn capacity_row(method: &str, material: &str, brand: &str) -> &'static [(f64, f64)] {
    let al = material == "al";
    let ground = method == "D";
    match brand {
        "waskoenig" => {
            if al {
                if ground { waskoenig_al_ground() } else { waskoenig_al_air() }
            } else if ground {
                waskoenig_cu_ground()
            } else {
                waskoenig_cu_air()
            }
        }
        "prysmian" => {
            if al {
                if ground { waskoenig_al_ground() } else { waskoenig_al_air() }
            } else if ground {
                prysmian_cu_ground()
            } else {
                prysmian_cu_air()
            }
        }
        _ => {
            if al {
                nkt_al(method)
            } else {
                nkt_cu(method)
            }
        }
    }
}

pub fn get_capacity(method: &str, section: f64, material: &str, brand: &str) -> Option<f64> {
    lookup(capacity_row(method, material, brand), section)
}

fn group_row(kind: &str) -> &'static [(i32, f64)] {
    match kind {
        "vaeg" => &[(1, 1.0), (2, 0.85), (3, 0.79), (4, 0.75), (5, 0.73), (6, 0.72), (7, 0.72), (8, 0.71), (9, 0.7)],
        "loft" => &[(1, 0.95), (2, 0.81), (3, 0.72), (4, 0.68), (5, 0.66), (6, 0.64), (7, 0.63), (8, 0.62), (9, 0.61)],
        "perf" => &[(1, 1.0), (2, 0.88), (3, 0.82), (4, 0.77), (5, 0.75), (6, 0.73), (7, 0.73), (8, 0.72), (9, 0.72)],
        "stige" => &[(1, 1.0), (2, 0.87), (3, 0.82), (4, 0.8), (5, 0.8), (6, 0.79), (7, 0.79), (8, 0.78), (9, 0.78)],
        _ => &[(1, 1.0), (2, 0.8), (3, 0.7), (4, 0.65), (5, 0.6), (6, 0.57), (7, 0.54), (8, 0.52), (9, 0.5), (12, 0.45), (16, 0.41), (20, 0.38)],
    }
}

fn row_get(row: &[(i32, f64)], n: i32) -> Option<f64> {
    row.iter().find(|(k, _)| *k == n).map(|(_, v)| *v)
}

pub fn get_group_factor(n: i32, kind: &str) -> f64 {
    let bundt = group_row("bundt");
    let row = group_row(kind);
    if let Some(v) = row_get(row, n) {
        return v;
    }
    if n >= 12 && kind == "bundt" {
        return row_get(row, 20).or_else(|| row_get(row, 16)).or_else(|| row_get(row, 12)).unwrap_or(1.0);
    }
    if n > 9 {
        return row_get(bundt, n).unwrap_or_else(|| row_get(bundt, 20).unwrap_or(1.0));
    }
    row_get(row, 9).unwrap_or(1.0)
}

const TEMP_AIR: &[(f64, f64)] = &[
    (10.0, 1.22),
    (15.0, 1.17),
    (20.0, 1.12),
    (25.0, 1.06),
    (30.0, 1.0),
    (35.0, 0.94),
    (40.0, 0.87),
    (45.0, 0.79),
    (50.0, 0.71),
    (55.0, 0.61),
    (60.0, 0.5),
];
const TEMP_GROUND: &[(f64, f64)] = &[(10.0, 1.1), (15.0, 1.05), (20.0, 1.0), (25.0, 0.95), (30.0, 0.89), (35.0, 0.84), (40.0, 0.77)];

pub fn get_temp_factor(temp: f64, ground: bool) -> f64 {
    let table = if ground { TEMP_GROUND } else { TEMP_AIR };
    let Some((t0, f0)) = table.first().copied() else { return 1.0 };
    let Some((tn, fn_)) = table.last().copied() else { return 1.0 };
    if temp <= t0 {
        return f0;
    }
    if temp >= tn {
        return fn_;
    }
    for w in table.windows(2) {
        let (a, b) = (w[0], w[1]);
        if temp >= a.0 && temp <= b.0 {
            let span = b.0 - a.0;
            if span.abs() < 1e-12 {
                return a.1;
            }
            return a.1 + (b.1 - a.1) * (temp - a.0) / span;
        }
    }
    1.0
}

fn loaded_factor(n: i32) -> f64 {
    match n {
        3 => 0.84,
        4 => 0.8,
        _ => 1.0,
    }
}

fn pe_from_table(phase_section: f64) -> f64 {
    if phase_section <= 16.0 {
        phase_section
    } else if phase_section <= 35.0 {
        16.0
    } else {
        phase_section / 2.0
    }
}

fn round_up(val: f64, list: &[f64]) -> f64 {
    list.iter().copied().find(|s| *s >= val - 1e-9).unwrap_or_else(|| list.last().copied().unwrap_or(val))
}

/// Cable impedance for `length_m` metres. `None` when the section is not in the table.
pub fn impedance(section: f64, length_m: f64, material: &str) -> Option<Z> {
    let al = material == "al";
    let r1 = lookup(r1_table(al), section)?;
    let x1 = lookup(x1_table(al), section)?;
    let (_, r0, x0) = r0x0_table(al).iter().find(|(s, _, _)| (*s - section).abs() < 1e-6).copied()?;
    let lkm = length_m / 1000.0;
    Some(Z { r1: r1 * lkm, x1: x1 * lkm, r0: r0 * lkm, x0: x0 * lkm })
}

pub fn drop_voltage(section: f64, i: f64, lkm: f64, phase: &str, material: &str, cos_phi: f64) -> f64 {
    let al = material == "al";
    let Some(r1) = lookup(r1_table(al), section) else { return f64::INFINITY };
    let Some(x1) = lookup(x1_table(al), section) else { return f64::INFINITY };
    let sin_phi = (1.0 - cos_phi * cos_phi).max(0.0).sqrt();
    let k = if phase == "3" { 3.0_f64.sqrt() } else { 2.0 };
    k * i * lkm * (r1 * cos_phi + x1 * sin_phi)
}

/// First listed section at or above `min_section`. `None` when there is no floor.
fn listed_floor(material: &str, min_section: Option<f64>) -> Option<f64> {
    let min = min_section.filter(|s| s.is_finite() && *s > 0.0)?;
    let list = sections_for(material);
    list.iter().copied().find(|s| *s + 1e-9 >= min).or_else(|| list.last().copied())
}

fn sections_from(material: &str, floor: Option<f64>) -> Vec<f64> {
    let list = sections_for(material);
    let kept: Vec<f64> = match floor {
        Some(floor) => list.iter().copied().filter(|s| *s + 1e-9 >= floor).collect(),
        None => list.to_vec(),
    };
    if kept.is_empty() { list.to_vec() } else { kept }
}

fn find_min_section_for_drop(drop_v: f64, i: f64, lkm: f64, phase: &str, material: &str, cos_phi: f64, floor: Option<f64>) -> f64 {
    let list = sections_from(material, floor);
    for s in &list {
        if drop_voltage(*s, i, lkm, phase, material, cos_phi) <= drop_v {
            return *s;
        }
    }
    list.last().copied().unwrap_or(0.0)
}

struct Suggested {
    section: f64,
    meets: bool,
}

fn find_suggested(
    drop_v: f64,
    i: f64,
    lkm: f64,
    phase: &str,
    method: &str,
    material: &str,
    temp_factor: f64,
    group_factor: f64,
    loaded_factor: f64,
    cos_phi: f64,
    brand: &str,
    floor: Option<f64>,
) -> Suggested {
    let list = sections_from(material, floor);
    for s in &list {
        if drop_voltage(*s, i, lkm, phase, material, cos_phi) > drop_v {
            continue;
        }
        let Some(base) = get_capacity(method, *s, material, brand) else { continue };
        if base * temp_factor * group_factor * loaded_factor >= i {
            return Suggested { section: *s, meets: true };
        }
    }
    let last = list.last().copied().unwrap_or(0.0);
    Suggested { section: last, meets: false }
}

pub fn fault_currents(total: Z, u0: f64) -> (f64, f64, f64) {
    let z1 = (total.r1 * total.r1 + total.x1 * total.x1).sqrt();
    let ikmax = if z1 > 1e-9 { u0 / z1 } else { f64::INFINITY };
    let r1w = 1.5 * total.r1;
    let r0w = 1.5 * total.r0;
    let den2 = (r1w + r0w) * (r1w + r0w) + (total.x1 + total.x0) * (total.x1 + total.x0);
    let ikmin = if den2 > 1e-12 { (3.0 * u0 * 0.95) / den2.sqrt() } else { f64::INFINITY };
    (ikmax, ikmin, z1)
}

fn gg_note(in_amp: i32) -> String {
    if in_amp <= 16 {
        "1 t (In ≤ 16 A)".into()
    } else if in_amp <= 63 {
        "1 t (16 A < In ≤ 63 A)".into()
    } else if in_amp <= 160 {
        "2 t (63 A < In ≤ 160 A)".into()
    } else if in_amp <= 400 {
        "3 t (160 A < In ≤ 400 A)".into()
    } else {
        "4 t (In > 400 A)".into()
    }
}

fn brand_label(brand: &str) -> &'static str {
    match brand {
        "waskoenig" => "Waskönig",
        "prysmian" => "Prysmian Afumex",
        _ => "NKT",
    }
}

fn is_fuse(breaker: &str) -> bool {
    matches!(breaker, "NH00" | "NH1" | "NH2" | "Diazed" | "Neozed")
}

/// Run the same calculation as `ElCalc.analyze`.
pub fn analyze(input: &Input) -> Output {
    let mut out = Output::default();
    let i = input.i;
    let l = input.l;
    let u = input.u;
    let drop_pct = input.drop_pct;
    if !i.is_finite() || !l.is_finite() || !u.is_finite() || !drop_pct.is_finite() || i <= 0.0 || l <= 0.0 || u <= 0.0 || drop_pct <= 0.0 {
        out.error = "Udfyld strøm, længde, spænding og tilladt spændingsfald.".into();
        return out;
    }
    let phase = if input.phase == "3" { "3" } else { "1" };
    let material = if input.material == "al" { "al" } else { "cu" };
    let method = if input.method.is_empty() { "B2" } else { input.method.as_str() };
    let brand = if input.brand.is_empty() { "nkt" } else { input.brand.as_str() };
    let ground = method == "D";
    let temp_used = if input.temp.is_finite() {
        input.temp
    } else if ground {
        20.0
    } else {
        30.0
    };
    let temp_factor = get_temp_factor(temp_used, ground);
    let n_loaded = input.loaded;
    let lf = loaded_factor(n_loaded);
    let sidelob_type = if input.sidelob_type.is_empty() { "bundt" } else { input.sidelob_type.as_str() };
    let n_sidelob = if input.sidelob > 0 { input.sidelob } else { 1 };
    let gf = get_group_factor(n_sidelob, sidelob_type);
    let cos_phi = if (0.5..=1.0).contains(&input.cosphi) { input.cosphi } else { 0.9 };
    let drop_v_limit = (u * drop_pct) / 100.0;
    let lkm = l / 1000.0;
    let floor = listed_floor(material, input.min_section);
    let min_drop = find_min_section_for_drop(drop_v_limit, i, lkm, phase, material, cos_phi, floor);
    let suggested = find_suggested(drop_v_limit, i, lkm, phase, method, material, temp_factor, gf, lf, cos_phi, brand, floor);
    let mut section = input.section_lock.unwrap_or(suggested.section);
    if !sections_for(material).iter().any(|s| (*s - section).abs() < 1e-6) {
        section = suggested.section;
    }
    // A designation is a floor, never a lock. An explicit lock below that floor is raised.
    if let Some(floor) = floor
        && section + 1e-9 < floor
    {
        section = suggested.section;
    }
    let locked = input.section_lock.is_some_and(|s| (s - section).abs() < 1e-6);
    let actual_drop_v = drop_voltage(section, i, lkm, phase, material, cos_phi);
    let actual_drop_pct = if u.abs() > 1e-12 { (actual_drop_v / u) * 100.0 } else { f64::INFINITY };
    let base_cap = get_capacity(method, section, material, brand);
    let corrected = base_cap.map(|c| c * temp_factor * gf * lf);
    let cable_z = impedance(section, l, material);
    let u0 = if phase == "3" { u / 3.0_f64.sqrt() } else { u };
    let (ikmax_start, ikmin_start, _) = fault_currents(input.up, u0);
    let end_z = cable_z.map(|z| input.up + z);
    let (ikmax, ikmin, _) = end_z.map(|z| fault_currents(z, u0)).unwrap_or((0.0, 0.0, 0.0));
    let pe_table = pe_from_table(section);
    let mut pe_required = pe_table;
    let t_max = if input.system_type == "TT" { 0.2 } else { 0.4 };
    if section > 16.0 && ikmax.is_finite() && ikmax > 0.0 {
        let k_pe = if material == "al" { 76.0 } else { 115.0 };
        let pe_adiabatic = (ikmax * ikmax * t_max).sqrt() / k_pe;
        pe_required = pe_table.max(pe_adiabatic);
    }
    let pe_section = round_up(pe_required, PE_SECTIONS);
    let fuse = is_fuse(&input.breaker_type);
    let mut over_ok = None;
    let mut earth_ok = None;
    let mut short_ok = None;
    let mut ia = None;
    let mut t_adiabatic = None;
    if let Some(in_val) = input.in_amp.filter(|n| *n > 0) {
        let iz = corrected.unwrap_or(0.0);
        over_ok = Some(i <= f64::from(in_val) && (iz <= 0.0 || f64::from(in_val) <= iz));
        let mult = match input.breaker_type.as_str() {
            "C" => 10.0,
            "D" => 20.0,
            _ => 5.0,
        };
        let ia_v = f64::from(in_val) * mult;
        ia = Some(ia_v);
        earth_ok = Some(cable_z.is_some() && ikmin.is_finite() && ikmin >= ia_v);
        let k_ad = if material == "al" { 76.0 } else { 115.0 };
        let i2t = (k_ad * section) * (k_ad * section);
        let ik_use = if cable_z.is_some() && ikmax.is_finite() { ikmax } else { 0.0 };
        let t_ad = if ik_use > 0.0 { i2t / (ik_use * ik_use) } else { f64::INFINITY };
        t_adiabatic = Some(t_ad);
        short_ok = Some(cable_z.is_none() || t_ad >= 0.01);
    }
    let is90 = brand == "waskoenig" || brand == "prysmian";
    let temp_ref = if ground {
        if is90 { "20 °C i jord, 90 °C XLPE" } else { "20 °C i jord, NKT" }
    } else if is90 {
        "30 °C i luft, 90 °C XLPE"
    } else {
        "30 °C i luft, NKT"
    };
    Output {
        ok: true,
        error: String::new(),
        i,
        l,
        u,
        u0,
        phase: phase.into(),
        material: material.into(),
        material_label: if material == "al" { "Aluminium" } else { "Kobber" }.into(),
        brand: brand.into(),
        brand_label: brand_label(brand).into(),
        method: method.into(),
        cosphi: cos_phi,
        min_drop_section: min_drop,
        recommended: suggested.section,
        recommended_meets: suggested.meets,
        section,
        locked,
        min_section: floor,
        capacity: base_cap,
        corrected_capacity: corrected,
        temp_used,
        temp_factor,
        temp_ref: temp_ref.into(),
        group_factor: gf,
        loaded_factor: lf,
        n_loaded,
        n_sidelob,
        drop_v: actual_drop_v,
        drop_pct: actual_drop_pct,
        drop_limit_pct: drop_pct,
        drop_ok: actual_drop_v.is_finite() && actual_drop_v <= drop_v_limit + 1e-9,
        current_ok: corrected.is_some_and(|c| c + 1e-9 >= i),
        cable_z,
        ikmax,
        ikmin,
        ikmax_start,
        ikmin_start,
        pe_section,
        pe_required,
        in_amp: input.in_amp.filter(|n| *n > 0),
        breaker_type: input.breaker_type.clone(),
        is_fuse: fuse,
        system_type: if input.system_type.is_empty() { "TN" } else { input.system_type.as_str() }.into(),
        t_max,
        ia,
        over_ok,
        earth_ok,
        short_ok,
        t_adiabatic,
        fuse_note: if fuse { gg_note(input.in_amp.unwrap_or(0)) } else { String::new() },
    }
}

/// Z1 = U0 / Ikmax, X = 0, R0 = R1. Same conversion as the El-beregner.
pub fn impedance_from_ik(ik_a: f64, u0: f64) -> Option<Z> {
    if !ik_a.is_finite() || !u0.is_finite() || ik_a <= 0.0 || u0 <= 0.0 {
        return None;
    }
    let z = u0 / ik_a;
    Some(Z { r1: z, x1: 0.0, r0: z, x0: 0.0 })
}
