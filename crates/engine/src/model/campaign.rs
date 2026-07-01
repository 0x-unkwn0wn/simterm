//! Definición de una campaña completa: la unidad que el motor interpreta.
//!
//! Una `Campaign` es 100% datos cargados desde disco (RON). El motor no contiene
//! ninguna campaña; solo sabe interpretar esta estructura.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::language::Language;
use crate::model::mission::Mission;
use crate::model::terminal::TerminalCommand;
use crate::model::theme::{EasterEgg, Theme};

pub use crate::model::command::{CampaignCommand, CommandCondition, CommandEffect};

/// Una campaña: secuencia de misiones + toda su tematización y contenido suelto.
#[derive(Debug, Clone, Deserialize)]
pub struct Campaign {
    /// Nombre de la campaña (se muestra al arrancar).
    pub name: String,
    /// Idioma de los textos genéricos del motor. El contenido narrativo sigue
    /// viviendo en la campaña.
    #[serde(default)]
    pub language: Language,
    /// Texto de introducción (lore) mostrado al arrancar la campaña.
    #[serde(default)]
    pub intro: Vec<String>,
    /// Niveles, en orden de progresión.
    pub missions: Vec<Mission>,

    /// Branding y textos cosméticos. Si se omite, el motor usa defaults neutrales.
    #[serde(default)]
    pub theme: Theme,
    /// Comandos ocultos temáticos (no afectan a la partida).
    #[serde(default)]
    pub easter_eggs: Vec<EasterEgg>,
    /// Aforismos para el comando oculto `fortune` (elección aleatoria).
    #[serde(default = "default_fortunes")]
    pub fortunes: Vec<String>,
    /// Palabras en claro para el minijuego `signal` (se interceptan cifradas).
    #[serde(default = "default_signals")]
    pub signals: Vec<String>,
    /// Logros específicos de esta campaña, definidos como datos.
    #[serde(default)]
    pub achievements: Vec<CampaignAchievement>,
    /// Comandos declarativos con efectos simples definidos por la campaña. A
    /// diferencia de los `easter_eggs` (solo sabor), estos SÍ pueden alterar el
    /// estado de la partida (flags, traza, logros...) sin tocar Rust.
    #[serde(default)]
    pub commands: Vec<CampaignCommand>,
    /// Variables de entorno base del host objetivo (para `env`, `export` y la
    /// expansión de `$VAR`). El motor deriva además `USER`, `HOME`, `PWD`,
    /// `HOSTNAME` y `SHELL` a partir del estado.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Procesos extra que muestra `ps`, además de los sintetizados a partir de los
    /// servicios del host. Formato libre (p. ej. `"www-data 812 nginx"`).
    #[serde(default)]
    pub processes: Vec<String>,
    /// Comandos de terminal autorados (salida realista, presentacional). Para CLIs
    /// ficticias que el motor no puede sintetizar (p. ej. `systemctl status`).
    #[serde(default)]
    pub terminal: Vec<TerminalCommand>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CampaignAchievement {
    /// Identificador estable para guardado y desbloqueo.
    pub id: String,
    /// Título visible del logro.
    pub title: String,
    /// Descripción visible del logro.
    #[serde(default)]
    pub description: String,
    /// Evento declarativo que desbloquea el logro.
    pub trigger: CampaignAchievementTrigger,
}

#[derive(Debug, Clone, Deserialize)]
pub enum CampaignAchievementTrigger {
    /// Se desbloquea al leer/decodificar un fichero concreto.
    ReadFile(String),
    /// Se desbloquea al completar la misión con este `Mission.id`.
    CompleteMission(String),
    /// Se desbloquea al elegir un final: `choice` es 1-based, como `choose <n>`.
    ChooseEnding { mission: String, choice: usize },
    /// Se desbloquea al completar la campaña.
    CampaignComplete,
}

/// Aforismos neutrales por defecto (no pertenecen a ninguna historia concreta).
fn default_fortunes() -> Vec<String> {
    vec![
        String::from("Quien controla la información, controla el miedo."),
        String::from("El silencio también es una señal. Apréndela."),
        String::from("La paciencia es el único exploit que nunca falla."),
    ]
}

/// Palabras por defecto para el minijuego de señales (neutrales).
fn default_signals() -> Vec<String> {
    vec![
        String::from("ALPHA"),
        String::from("BRAVO"),
        String::from("CIPHER"),
        String::from("ECHO"),
        String::from("SIGNAL"),
    ]
}

impl Campaign {
    /// Busca el easter egg cuyo conjunto de `triggers` contiene `verb`.
    pub fn easter_egg(&self, verb: &str) -> Option<&EasterEgg> {
        self.easter_eggs
            .iter()
            .find(|e| e.triggers.iter().any(|t| t == verb))
    }

    /// Busca el comando declarativo cuyo conjunto de `triggers` contiene `verb`.
    /// No comprueba condiciones: eso lo hace el runtime con el estado en curso.
    pub fn command(&self, verb: &str) -> Option<&CampaignCommand> {
        self.commands
            .iter()
            .find(|c| c.triggers.iter().any(|t| t == verb))
    }

    /// Busca el comando de terminal autorado cuyo `triggers` contiene `verb`.
    pub fn terminal_command(&self, verb: &str) -> Option<&TerminalCommand> {
        self.terminal
            .iter()
            .find(|c| c.triggers.iter().any(|t| t == verb))
    }
}
