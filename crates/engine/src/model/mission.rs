//! Definición de misiones (datos, NO estado de juego).
//!
//! Capa de *definición*: estructuras inmutables que describen qué niveles existen
//! y cómo es cada objetivo. El estado de la partida en curso (intel, detección,
//! reloj, logs...) vive aparte, en [`crate::runtime::state`].
//!
//! El motor NO incrusta ninguna misión: todas se cargan desde la campaña.

use serde::Deserialize;

use crate::model::meter::MeterDef;
use crate::model::target::TargetNode;

/// Umbral de detección por defecto si una misión no especifica el suyo. Es un
/// valor de balance neutral del motor, no contenido de campaña.
const DEFAULT_DETECTION_LIMIT: f32 = 100.0;

fn default_detection_limit() -> f32 {
    DEFAULT_DETECTION_LIMIT
}

fn default_skill() -> f32 {
    0.5
}

fn default_root_difficulty() -> u8 {
    5
}

fn default_entry() -> EntryVector {
    EntryVector::Active
}

/// Un host dentro de una red interna (misión multi-host). Si una misión define
/// `network`, se ignora su `target`.
#[derive(Debug, Clone, Deserialize)]
pub struct NetHost {
    /// Definición del host (servicios, vulnerabilidades, filesystem).
    pub target: TargetNode,
    /// Hosts internos alcanzables desde este (por nombre corto o FQDN).
    #[serde(default)]
    pub links: Vec<String>,
    /// ¿Es un punto de entrada de la red (alcanzable desde el principio)?
    #[serde(default)]
    pub entry: bool,
    /// Fichero objetivo a exfiltrar en este host (si lo hay).
    #[serde(default)]
    pub objective: Option<String>,
}

/// Un desenlace posible al cerrar una operación con decisión (final con
/// elección). Solo lo usan las misiones que definen `endings`.
#[derive(Debug, Clone, Deserialize)]
pub struct Ending {
    /// Texto de la opción que se muestra en la lista de elección.
    pub title: String,
    /// Epílogo (lore) que se muestra al elegir este desenlace.
    #[serde(default)]
    pub lines: Vec<String>,
}

/// Vector de entrada de una operación: cómo arranca el nivel (la "boca").
/// Mantiene intacta la kill chain; solo cambia el primer paso.
#[derive(Debug, Clone, Deserialize)]
pub enum EntryVector {
    /// Escaneo activo con `nmap`: ruidoso, revela todos los servicios de golpe.
    Active,
    /// Arranque "frío": el cliente ya señaló servicios. Se empieza en ENUM con
    /// esos puertos ya descubiertos (vacío = todos). Sin `nmap` obligatorio.
    Cold {
        #[serde(default)]
        ports: Vec<u16>,
    },
    /// Interceptación pasiva con `sniff`: muy sigiloso, revela los servicios de
    /// uno en uno. El `nmap` activo aquí deja rastro extra.
    Passive,
    /// El objetivo está tras un bastión: hay que `connect` antes de escanear.
    Pivot {
        #[serde(default)]
        gateway: String,
    },
}

/// Definición de un nivel: metadatos + ajustes + nodo objetivo.
#[derive(Debug, Clone, Deserialize)]
pub struct Mission {
    /// Identificador interno de la misión (metadato; no se usa en la lógica).
    pub id: String,
    pub name: String,
    /// Líneas de briefing que se muestran al empezar el nivel.
    #[serde(default)]
    pub briefing: Vec<String>,
    /// Detección a la que se pierde el nivel.
    #[serde(default = "default_detection_limit")]
    pub detection_limit: f32,
    /// Medidores genéricos del nivel (combustible, oxígeno, progreso...): cada
    /// uno con su umbral y qué pasa al alcanzarlo. Vacío = solo la traza clásica.
    #[serde(default)]
    pub meters: Vec<MeterDef>,
    /// Ventana de tiempo de la operación, en ticks de reloj. Si el reloj la
    /// supera, derrota por "ventana cerrada". `None` = sin límite (clásico).
    #[serde(default)]
    pub time_limit: Option<u32>,
    /// Defensa activa: si es `true`, el host tiene equipo azul que responde por
    /// etapas a la traza (endurece el sistema y acelera la detección). `false`
    /// = host pasivo (comportamiento clásico).
    #[serde(default)]
    pub reactive: bool,
    /// Habilidad del operador en este nivel (0.0 ..= 1.0).
    #[serde(default = "default_skill")]
    pub skill: f32,
    /// Dificultad de la escalada de privilegios local (privesc), 1..=10.
    #[serde(default = "default_root_difficulty")]
    pub root_difficulty: u8,
    /// Ruta del fichero objetivo a exfiltrar para completar el nivel.
    /// Si es `None`, el nivel se completa al lograr root con `privesc`.
    #[serde(default)]
    pub objective: Option<String>,
    /// Texto de cierre (lore) que se muestra al completar el nivel.
    #[serde(default)]
    pub debrief: Vec<String>,
    /// Cómo arranca el nivel (por defecto: escaneo activo con `nmap`).
    #[serde(default = "default_entry")]
    pub entry: EntryVector,
    /// Desenlaces a elegir al cerrar la operación (final con elección). Si está
    /// vacío, la operación se cierra directamente.
    #[serde(default)]
    pub endings: Vec<Ending>,
    /// Host único de la operación (modo clásico). Las misiones multi-host lo
    /// omiten y definen `network` en su lugar.
    #[serde(default = "TargetNode::empty")]
    pub target: TargetNode,
    /// Red interna de hosts (modo multi-host). Si está vacía, se usa `target`.
    #[serde(default)]
    pub network: Vec<NetHost>,
    /// Pista de música de la misión, como ruta RELATIVA al directorio de la
    /// campaña (p. ej. `"music/theme.wav"`). Si es `None`, el frontend recurre a
    /// la convención por nombre: `music/mission_{N}_theme.wav`. Es solo
    /// presentación: el motor no reproduce audio, solo transporta el dato.
    #[serde(default)]
    pub music: Option<String>,
    /// Guion opcional para el autoplayer genérico. Cada entrada es una línea de
    /// comando que el frontend inyecta como si la hubiera tecleado el jugador.
    /// Si está vacío, los dominios no-pentest pueden usar una heurística basada
    /// en comandos declarativos.
    #[serde(default)]
    pub autoplay: Vec<String>,
}
