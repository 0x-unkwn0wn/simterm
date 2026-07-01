//! Toolbox de enumeración: herramientas con afinidad por tipo de servicio.
//!
//! Capa de *definición* (datos estáticos del motor). Cada herramienta es buena
//! contra cierta categoría de servicio: usada sobre el servicio adecuado revela
//! vulns reales con poco ruido; usada sobre el equivocado es ruidosa e ineficaz.
//!
//! Estas herramientas son mecánica de motor genérica (no contenido de campaña):
//! representan utilidades de pentesting del mundo real, no una historia concreta.

use crate::model::language::Language;

/// Categoría de un servicio, inferida de su nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceCat {
    Web,
    Smb,
    Ssh,
    Db,
    Other,
}

impl ServiceCat {
    pub fn label_in(&self, language: Language) -> &'static str {
        match (language, self) {
            (_, ServiceCat::Web) => "web",
            (_, ServiceCat::Smb) => "smb",
            (_, ServiceCat::Ssh) => "ssh/login",
            (Language::Es, ServiceCat::Db) => "base de datos",
            (Language::Es, ServiceCat::Other) => "genérico",
            (Language::En, ServiceCat::Db) => "database",
            (Language::En, ServiceCat::Other) => "generic",
        }
    }

    pub fn label(&self) -> &'static str {
        self.label_in(Language::Es)
    }
}

/// Deduce la categoría de un servicio por su nombre.
pub fn category(service_name: &str) -> ServiceCat {
    match service_name.to_lowercase().as_str() {
        "http" | "https" | "http-proxy" | "http-alt" => ServiceCat::Web,
        "smb" | "netbios" | "netbios-ssn" | "microsoft-ds" => ServiceCat::Smb,
        "ssh" => ServiceCat::Ssh,
        "mysql" | "pgsql" | "postgresql" | "redis" | "mongodb" | "mssql" | "oracle" => {
            ServiceCat::Db
        }
        _ => ServiceCat::Other,
    }
}

/// Definición de una herramienta de enumeración.
pub struct EnumTool {
    pub name: &'static str,
    pub desc: &'static str,
    /// Categorías para las que es buena. Vacío = herramienta genérica.
    pub affinities: &'static [ServiceCat],
    pub time: u32,
    pub noise: f32,
    /// Probabilidad de detectar una vuln real cuando la afinidad encaja.
    pub hit_match: f32,
    /// Probabilidad cuando NO encaja (herramienta inadecuada).
    pub hit_other: f32,
    /// Rango de confianza inicial de los hallazgos (lo, hi).
    pub conf: (f32, f32),
    /// Probabilidad de soltar un falso positivo si la afinidad encaja.
    pub fp_match: f32,
    /// Probabilidad de falso positivo si NO encaja (ruido inútil).
    pub fp_other: f32,
}

impl EnumTool {
    pub fn is_generic(&self) -> bool {
        self.affinities.is_empty()
    }

    /// ¿Es adecuada para esta categoría de servicio?
    pub fn matches(&self, cat: ServiceCat) -> bool {
        self.is_generic() || self.affinities.contains(&cat)
    }

    pub fn desc_in(&self, language: Language) -> &'static str {
        match (language, self.name) {
            (Language::Es, "probe") => {
                "sonda genérica de servicio (sirve para cualquiera, mediocre)"
            }
            (Language::Es, "nikto") => "escáner de vulnerabilidades web",
            (Language::Es, "gobuster") => "fuerza bruta de rutas/ficheros web",
            (Language::Es, "enum4linux") => "enumeración SMB / NetBIOS",
            (Language::Es, "hydra") => "fuerza bruta de credenciales (MUY ruidoso)",
            (Language::Es, "sqlmap") => "explotación de inyección SQL (web / base de datos)",
            (Language::En, "probe") => "generic service probe (works anywhere, mediocre)",
            (Language::En, "nikto") => "web vulnerability scanner",
            (Language::En, "gobuster") => "web path/file brute forcer",
            (Language::En, "enum4linux") => "SMB / NetBIOS enumeration",
            (Language::En, "hydra") => "credential brute forcing (VERY noisy)",
            (Language::En, "sqlmap") => "SQL injection exploitation (web / database)",
            _ => self.desc,
        }
    }
}

/// Catálogo de herramientas disponibles en la fase de enumeración.
pub const TOOLS: &[EnumTool] = &[
    EnumTool {
        name: "probe",
        desc: "sonda genérica de servicio (sirve para cualquiera, mediocre)",
        affinities: &[],
        time: 6,
        noise: 6.0,
        hit_match: 0.42,
        hit_other: 0.42,
        conf: (0.30, 0.55),
        fp_match: 0.70,
        fp_other: 0.70,
    },
    EnumTool {
        name: "nikto",
        desc: "escáner de vulnerabilidades web",
        affinities: &[ServiceCat::Web],
        time: 8,
        noise: 8.0,
        hit_match: 0.72,
        hit_other: 0.12,
        conf: (0.45, 0.70),
        fp_match: 0.35,
        fp_other: 0.75,
    },
    EnumTool {
        name: "gobuster",
        desc: "fuerza bruta de rutas/ficheros web",
        affinities: &[ServiceCat::Web],
        time: 7,
        noise: 7.0,
        hit_match: 0.60,
        hit_other: 0.10,
        conf: (0.40, 0.65),
        fp_match: 0.40,
        fp_other: 0.70,
    },
    EnumTool {
        name: "enum4linux",
        desc: "enumeración SMB / NetBIOS",
        affinities: &[ServiceCat::Smb],
        time: 8,
        noise: 7.0,
        hit_match: 0.72,
        hit_other: 0.12,
        conf: (0.50, 0.75),
        fp_match: 0.35,
        fp_other: 0.70,
    },
    EnumTool {
        name: "hydra",
        desc: "fuerza bruta de credenciales (MUY ruidoso)",
        affinities: &[ServiceCat::Ssh],
        time: 12,
        noise: 16.0,
        hit_match: 0.70,
        hit_other: 0.10,
        conf: (0.55, 0.80),
        fp_match: 0.30,
        fp_other: 0.60,
    },
    EnumTool {
        name: "sqlmap",
        desc: "explotación de inyección SQL (web / base de datos)",
        affinities: &[ServiceCat::Web, ServiceCat::Db],
        time: 10,
        noise: 10.0,
        hit_match: 0.70,
        hit_other: 0.12,
        conf: (0.50, 0.78),
        fp_match: 0.35,
        fp_other: 0.65,
    },
];

pub fn tool_by_name(name: &str) -> Option<&'static EnumTool> {
    TOOLS.iter().find(|t| t.name == name)
}
