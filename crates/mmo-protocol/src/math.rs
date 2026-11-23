#![allow(clippy::all)]
use super::*;
use ::lang::{CatalogEntry, LangCatalog};
pub fn ground_height_at(x: f32, z: f32) -> f32 {
    let dist_sq = x * x + z * z;
    if dist_sq < 225.0 {
        return 0.0;
    }
    let dist = sqrt_f32(dist_sq);
    let blend = clamp_f32((dist - 15.0) / 14.0, 0.0, 1.0);
    let hill =
        sin_approx(x * 0.055) * cos_approx(z * 0.055) * 2.4 + sin_approx((x + z) * 0.09) * 0.8;
    let rim = if dist > 82.0 {
        let t = (dist - 82.0) / 16.0;
        t * t * 9.0
    } else {
        0.0
    };
    hill * blend + rim
}

pub fn forest_tree_position(index: usize) -> Option<(f32, f32)> {
    if index >= FOREST_TREE_COUNT {
        return None;
    }

    let angle = index as f32 * 2.39996;
    let radius = 22.0 + ((index * 73) % 58) as f32;
    let x = cos_approx(angle) * radius;
    let z = sin_approx(angle) * radius;
    if sqrt_f32(x * x + (z + 58.0) * (z + 58.0)) < 14.0
        || (abs_f32(x) < 5.5 && z < -10.0 && z > -55.0)
    {
        None
    } else {
        Some((x, z))
    }
}

pub fn move_with_tree_collision(from_x: f32, from_z: f32, to_x: f32, to_z: f32) -> (f32, f32) {
    if !from_x.is_finite() || !from_z.is_finite() || !to_x.is_finite() || !to_z.is_finite() {
        return (from_x, from_z);
    }

    let (x, _) = sweep_tree_collision(from_x, from_z, to_x, from_z);
    let (_, z) = sweep_tree_collision(x, from_z, x, to_z);
    (x, z)
}

fn sweep_tree_collision(from_x: f32, from_z: f32, to_x: f32, to_z: f32) -> (f32, f32) {
    let dx = to_x - from_x;
    let dz = to_z - from_z;
    let steps = ceil_f32(abs_f32(dx).max(abs_f32(dz)) * 4.0).max(1);
    let mut last_safe = (from_x, from_z);

    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let x = from_x + dx * t;
        let z = from_z + dz * t;
        if overlaps_tree(x, z) {
            break;
        }
        last_safe = (x, z);
    }

    last_safe
}

fn overlaps_tree(x: f32, z: f32) -> bool {
    let radius_sq = TREE_COLLISION_RADIUS * TREE_COLLISION_RADIUS;
    for index in 0..FOREST_TREE_COUNT {
        let Some((tree_x, tree_z)) = forest_tree_position(index) else {
            continue;
        };
        let dx = x - tree_x;
        let dz = z - tree_z;
        if dx * dx + dz * dz < radius_sq {
            return true;
        }
    }
    false
}

#[inline]
pub fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn abs_f32(v: f32) -> f32 {
    if v < 0.0 {
        -v
    } else {
        v
    }
}

#[inline]
pub fn floor_f32(x: f32) -> i32 {
    let i = x as i32;
    if (i as f32) > x {
        i - 1
    } else {
        i
    }
}

#[inline]
pub fn ceil_f32(x: f32) -> i32 {
    let i = x as i32;
    if (i as f32) < x {
        i + 1
    } else {
        i
    }
}

#[inline]
pub fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut guess = x * 0.5 + 0.5;
    let mut i = 0;
    while i < 7 {
        guess = 0.5 * (guess + x / guess);
        i += 1;
    }
    guess
}

#[inline]
pub fn sin_approx(mut x: f32) -> f32 {
    const PI: f32 = 3.14159265;
    const TWO_PI: f32 = 6.2831853;
    while x > PI {
        x -= TWO_PI;
    }
    while x < -PI {
        x += TWO_PI;
    }
    let b = 4.0 / PI;
    let c = -4.0 / (PI * PI);
    let y = b * x + c * x * abs_f32(x);
    0.225 * (y * abs_f32(y) - y) + y
}

#[inline]
pub fn cos_approx(x: f32) -> f32 {
    sin_approx(x + 1.5707963)
}

pub fn atan2_approx(y: f32, x: f32) -> f32 {
    const PI: f32 = 3.14159265;
    if x == 0.0 {
        if y > 0.0 {
            return PI * 0.5;
        }
        if y < 0.0 {
            return -PI * 0.5;
        }
        return 0.0;
    }
    let z = y / x;
    let az = abs_f32(z);
    let atan = if az < 1.0 {
        z / (1.0 + 0.28 * z * z)
    } else {
        let sign = if z > 0.0 { 1.0 } else { -1.0 };
        sign * (PI * 0.5) - z / (z * z + 0.28)
    };
    if x < 0.0 {
        if y >= 0.0 {
            atan + PI
        } else {
            atan - PI
        }
    } else {
        atan
    }
}

// =========================================================================
// aethravox/Lang Integration (6-Language Offline MMO UI + LangButton Host)
// =========================================================================

pub static MMO_CATALOG: LangCatalog = LangCatalog::new(&[
    CatalogEntry::new(
        ::lang::fnv1a_64("WAYFARERS ONLINE (WFO)".as_bytes()),
        "WAYFARERS ONLINE (WFO)",
        [
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("ENTRAR AL MUNDO".as_bytes()),
        "ENTRAR AL MUNDO",
        [
            "ENTRAR AL MUNDO",
            "ENTER WORLD",
            "ENTRAR NO MUNDO",
            "ENTRER DANS LE MONDE",
            "WELT BETRETEN",
            "ENTRA NEL MONDO",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("NOMBRE".as_bytes()),
        "NOMBRE",
        ["NOMBRE", "NAME", "NOME", "NOM", "NAME", "NOME"],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("VANGUARDIA".as_bytes()),
        "VANGUARDIA",
        [
            "VANGUARDIA",
            "VANGUARD",
            "VANGUARDA",
            "AVANT-GARDE",
            "VORHUT",
            "AVANGUARDIA",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("ARCANISTA".as_bytes()),
        "ARCANISTA",
        [
            "ARCANISTA",
            "ARCANIST",
            "ARCANISTA",
            "ARCANISTE",
            "ARKANIST",
            "ARCANISTA",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("MONTARAZ".as_bytes()),
        "MONTARAZ",
        [
            "MONTARAZ",
            "RANGER",
            "PATRULHEIRO",
            "RODEUR",
            "WALDLAUFER",
            "GUARDABOSCHI",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("GOLPE".as_bytes()),
        "GOLPE",
        ["GOLPE", "STRIKE", "GOLPE", "FRAPPE", "SCHLAG", "COLPO"],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("NOVA".as_bytes()),
        "NOVA",
        ["NOVA", "SKILL", "SKILL", "SORT", "SKILL", "SKILL"],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("CURAR".as_bytes()),
        "CURAR",
        ["CURAR", "MEND", "CURAR", "SOIN", "HEILEN", "CURA"],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("MINAR".as_bytes()),
        "MINAR",
        ["MINAR", "GATHER", "COLETAR", "MINER", "SAMMELN", "MINARE"],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("PLAZA SANTUARIO".as_bytes()),
        "PLAZA SANTUARIO",
        [
            "PLAZA SANTUARIO",
            "SANCTUARY PLAZA",
            "PRACA SANTUARIO",
            "PLACE SANCTUAIRE",
            "ZUFLUCHTSPLATZ",
            "PIAZZA SANTUARIO",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("BOSQUE ESMERALDA".as_bytes()),
        "BOSQUE ESMERALDA",
        [
            "BOSQUE ESMERALDA",
            "EMERALD WOODS",
            "BOSQUE ESMERALDA",
            "BOIS EMERAUDE",
            "SMARAGDWALD",
            "BOSCO SMERALDO",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("CIUDADELA".as_bytes()),
        "CIUDADELA",
        [
            "CIUDADELA",
            "CITADEL",
            "CIDADELA",
            "CITADELLE",
            "ZITADELLE",
            "CITTADELLA",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("VETAS CRISTAL".as_bytes()),
        "VETAS CRISTAL",
        [
            "VETAS CRISTAL",
            "CRYSTAL VEINS",
            "VEIOS CRISTAL",
            "VEINES CRISTAL",
            "KRISTALLADERN",
            "VENE CRISTALLO",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("EN LINEA".as_bytes()),
        "EN LINEA",
        [
            "EN LINEA", "ONLINE", "ONLINE", "EN LIGNE", "ONLINE", "ONLINE",
        ],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("ORO".as_bytes()),
        "ORO",
        ["ORO", "GOLD", "OURO", "OR", "GOLD", "ORO"],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("CHAT".as_bytes()),
        "CHAT",
        ["CHAT", "CHAT", "CHAT", "CHAT", "CHAT", "CHAT"],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("SALTAR".as_bytes()),
        "SALTAR",
        ["SALTAR", "JUMP", "PULAR", "SAUTER", "SPRINGEN", "SALTA"],
    ),
    CatalogEntry::new(
        ::lang::fnv1a_64("TACTIL".as_bytes()),
        "TACTIL",
        ["TACTIL", "TOUCH", "TOQUE", "TACTILE", "TOUCH", "TOUCH"],
    ),
]);

#[::lang::app]
pub mod auto_labels {
    pub fn basics() -> [&'static str; 2] {
        ["Conectar", "Enviar"]
    }
}
