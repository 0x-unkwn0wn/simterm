//! Definición del nodo objetivo, sus servicios y vulnerabilidades ocultas.
//!
//! Solo tipos de **definición**: se cargan desde la campaña (RON). El motor no
//! incrusta ningún host concreto; los hosts viven en los ficheros de campaña.

use serde::{Deserialize, Serialize};

use crate::model::filesystem::FsNode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub port: u16,
    pub name: String,
    pub version: String,
    /// Token de credencial requerido para *enumerar* este servicio. Si está
    /// presente y el operador aún no lo ha recogido (no está en
    /// `foothold_tokens`), el servicio aparece en el escaneo pero rechaza la
    /// enumeración: hay que conseguir antes la credencial en otro host. `None`
    /// = servicio abierto (comportamiento clásico).
    #[serde(default)]
    pub requires: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExploitReliability {
    Reliable,
    Unstable,
}

impl Default for ExploitReliability {
    fn default() -> Self {
        Self::Unstable
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    /// Identificador interno (no visible directamente por el jugador).
    pub id: String,
    pub name: String,
    /// Puerto del servicio afectado.
    pub affected_service: u16,
    /// Dificultad de explotación 1..=10.
    pub difficulty: u8,
    /// Ruido añadido al intentar explotarla.
    pub stealth_cost: u8,
    /// Si el vector es determinista una vez identificado o depende de timing/estado.
    #[serde(default)]
    pub reliability: ExploitReliability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetNode {
    pub hostname: String,
    pub ip: String,
    pub os: String,
    pub services: Vec<Service>,
    /// Vulnerabilidades reales ocultas. El jugador NUNCA las lee directamente.
    pub vulnerabilities: Vec<Vulnerability>,
    /// Sistema de archivos del host (se explora en la fase POST). Opcional.
    #[serde(default)]
    pub filesystem: Vec<FsNode>,
    /// Token de credencial que este host acepta para un foothold determinista
    /// vía `login` (si el jugador lo recogió en un nivel anterior).
    #[serde(default)]
    pub accepts_token: Option<String>,
    /// Vector de escalada local descubrible por enumeración (`linpeas`/`sudo -l`/
    /// `suid`/`sysinfo`). Si está, ejecutar el comando adecuado en POST revela el
    /// vector y habilita `privesc` determinista (como una `privesc_key`).
    #[serde(default)]
    pub local_privesc: Option<LocalPrivesc>,
}

/// Vía de escalada local que la enumeración del host puede revelar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPrivesc {
    /// Tipo de vector: determina qué comando de enumeración lo descubre.
    pub kind: LocalKind,
    /// Pista mostrada al descubrirlo (cómo se aprovecha).
    pub note: String,
}

/// Categoría de un vector de escalada local. Cada una la revela su comando afín
/// (y también `linpeas`, que los cubre todos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalKind {
    /// Regla `sudo` abusable (GTFOBin). La revela `sudo -l`.
    Sudo,
    /// Binario SUID abusable. La revela `suid`.
    Suid,
    /// Kernel/SO vulnerable. La revela `sysinfo`.
    Kernel,
    /// Cron de root con script escribible. La revela `linpeas`.
    Cron,
}

impl TargetNode {
    pub fn vuln_by_id(&self, id: &str) -> Option<&Vulnerability> {
        self.vulnerabilities.iter().find(|v| v.id == id)
    }

    /// Nodo vacío (placeholder). Lo usan las misiones multi-host, que definen sus
    /// hosts en `network` en vez de en `target`.
    pub fn empty() -> Self {
        TargetNode {
            hostname: String::new(),
            ip: String::new(),
            os: String::new(),
            services: Vec::new(),
            vulnerabilities: Vec::new(),
            filesystem: Vec::new(),
            accepts_token: None,
            local_privesc: None,
        }
    }

    /// Nombre corto del host (primer segmento del FQDN).
    pub fn short_name(&self) -> &str {
        self.hostname.split('.').next().unwrap_or(&self.hostname)
    }
}
