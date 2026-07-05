//! Paleta de la interfaz: preferencia del JUGADOR, no de la campaña.
//!
//! La estética por defecto es CRT de fósforo ámbar (ver `ui.rs`). Este módulo
//! extrae los cinco colores a datos para que el jugador pueda escogerlos —por
//! CLI (`--appearance <nombre>`) o en vivo (F2)— sin recompilar y sin tocar el
//! `theme` de la campaña, que sigue mandando en el branding textual.
//!
//! La disciplina de dos colores (un matiz + un acento crítico) se preserva a
//! propósito: se ofrecen PRESETS cerrados, no un editor RGB libre, para no
//! diluir la identidad visual. Cada preset es un monocromo coherente.

use ratatui::style::Color;

/// Los cinco colores que componen la interfaz, más el ritmo de parpadeo de las
/// alertas críticas. Es `Copy`: circula por valor sin coste.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Nombre corto del preset (para `--appearance` y depuración).
    pub name: &'static str,
    /// Fondo de toda la interfaz.
    pub bg: Color,
    /// Texto principal.
    pub amber: Color,
    /// Texto secundario / UI / bordes.
    pub amber_dim: Color,
    /// Brillo / foco / datos clave.
    pub amber_hi: Color,
    /// Acento reservado en exclusiva a las alertas críticas.
    pub brick: Color,
    /// Periodo (en frames) del parpadeo lento de las alertas críticas.
    /// Un valor de 0 desactiva el parpadeo (útil para fotosensibilidad).
    pub blink_period: u16,
}

/// Periodo de parpadeo estándar (heredado de la estética original).
const BLINK: u16 = 9;

/// CRT de fósforo ámbar: la estética por defecto, cálida sobre negro.
pub const AMBER: Palette = Palette {
    name: "amber",
    bg: Color::Rgb(0x0A, 0x08, 0x00),
    amber: Color::Rgb(0xFF, 0xB0, 0x00),
    amber_dim: Color::Rgb(0xA8, 0x70, 0x00),
    amber_hi: Color::Rgb(0xFF, 0xD0, 0x50),
    brick: Color::Rgb(0x8B, 0x25, 0x00),
    blink_period: BLINK,
};

/// Fósforo verde P1: el monocromo de terminal clásico.
pub const GREEN: Palette = Palette {
    name: "green",
    bg: Color::Rgb(0x00, 0x0A, 0x02),
    amber: Color::Rgb(0x2E, 0xE6, 0x3A),
    amber_dim: Color::Rgb(0x1C, 0x8A, 0x2A),
    amber_hi: Color::Rgb(0xB6, 0xFF, 0xB0),
    brick: Color::Rgb(0xC0, 0x30, 0x20),
    blink_period: BLINK,
};

/// Fósforo blanco-azulado ("hielo"): frío, tipo VT monocromo blanco.
pub const ICE: Palette = Palette {
    name: "ice",
    bg: Color::Rgb(0x00, 0x04, 0x0A),
    amber: Color::Rgb(0x9C, 0xD8, 0xFF),
    amber_dim: Color::Rgb(0x4A, 0x80, 0xB0),
    amber_hi: Color::Rgb(0xE8, 0xF6, 0xFF),
    brick: Color::Rgb(0xFF, 0x53, 0x63),
    blink_period: BLINK,
};

/// Ámbar de alto contraste: negro puro, tonos más luminosos y sin parpadeo.
/// Pensado para legibilidad y para reducir la fatiga por destellos.
pub const AMBER_HC: Palette = Palette {
    name: "amber-hc",
    bg: Color::Rgb(0x00, 0x00, 0x00),
    amber: Color::Rgb(0xFF, 0xC4, 0x2E),
    amber_dim: Color::Rgb(0xD6, 0x9E, 0x20),
    amber_hi: Color::Rgb(0xFF, 0xF0, 0xB0),
    brick: Color::Rgb(0xFF, 0x45, 0x2A),
    blink_period: 0,
};

/// Todos los presets disponibles, en el orden en que los cicla F2.
pub const PRESETS: [Palette; 4] = [AMBER, GREEN, ICE, AMBER_HC];

impl Default for Palette {
    fn default() -> Self {
        AMBER
    }
}

impl Palette {
    /// Busca un preset por nombre (sin distinguir mayúsculas). `None` si no hay
    /// ninguno con ese nombre.
    pub fn by_name(name: &str) -> Option<Palette> {
        PRESETS
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .copied()
    }

    /// El siguiente preset en el ciclo (envuelve al llegar al final). Se usa para
    /// el cambio en vivo con F2.
    pub fn next(self) -> Palette {
        let i = PRESETS
            .iter()
            .position(|p| p.name == self.name)
            .unwrap_or(0);
        PRESETS[(i + 1) % PRESETS.len()]
    }

    /// Lista de nombres disponibles, para la ayuda de la CLI.
    pub fn names() -> Vec<&'static str> {
        PRESETS.iter().map(|p| p.name).collect()
    }
}
