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
    /// Nombres de las etapas de progresión, en orden. Por defecto, la kill chain
    /// del pentesting (RECON/ENUM/EXPLOIT/POST). Un dominio distinto (satélite,
    /// forense...) declara aquí las suyas y el motor las usa como etiquetas y para
    /// las condiciones de etapa de los comandos declarativos.
    #[serde(default = "default_stages")]
    pub stages: Vec<String>,

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
    /// Dominio del escenario: el conjunto de mecánicas de dominio que gobiernan
    /// la campaña. Si se omite, se infiere de las `stages` (ver
    /// [`Campaign::domain_kind`]). Ver [`DomainKind`].
    #[serde(default)]
    pub domain: Option<DomainKind>,
    /// Interruptores de mecánicas de dominio (kill chain, traza, VFS...). Cada uno
    /// es opcional: si se omite, cae al default del dominio. Ver [`Features`].
    #[serde(default)]
    pub features: Features,
}

/// Dominio de una campaña: el "tipo de escenario" que da semántica al núcleo
/// del motor. Es un conjunto **cerrado** de dominios in-tree (sin carga dinámica
/// de código): cada variante se corresponde con un módulo bajo `crate::domains`.
///
/// - [`DomainKind::Pentest`]: la kill chain de intrusión (RECON → ENUM →
///   EXPLOIT → POST) con traza, servicios, vulnerabilidades y VFS con shell.
/// - [`DomainKind::Bare`]: dominio **solo-datos**, sin mecánica de dominio en
///   Rust. Progresa con las `stages`, gana/pierde por `meters` y se conduce con
///   comandos declarativos (p. ej. `examples/demo_orbita`).
///
/// Si la campaña **omite** `domain`, se infiere heurísticamente (ver
/// [`Campaign::domain_kind`]): pentest si usa las etapas por defecto, `Bare` si
/// declara las suyas. Fijarlo explícitamente manda sobre la heurística.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum DomainKind {
    Pentest,
    Bare,
}

impl DomainKind {
    /// ¿Este dominio activa por defecto las mecánicas de intrusión (kill chain,
    /// traza, shell para el VFS)? Es la base de los defaults de [`Features`]; cada
    /// toggle puede seguir sobreescribiéndose por separado.
    pub fn defaults_pentest_mechanics(self) -> bool {
        matches!(self, DomainKind::Pentest)
    }
}

/// Interruptores por-campaña para activar/desactivar mecánicas del dominio de
/// pentesting en el motor y el frontend.
///
/// Cada toggle es `Option<bool>`: si se **omite**, cae al *default heurístico*
/// (pentest si la campaña usa las etapas por defecto; dominio propio si declara
/// las suyas). Si se **fija**, manda sobre la heurística. Así una campaña puede
/// mezclar: p. ej. un dominio forense con etapas propias PERO con el VFS libre
/// (`shell_for_vfs: Some(false)`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Features {
    /// Mostrar la ayuda de la kill chain (secciones por fase, pistas de
    /// intrusión) y los hints de arranque (vector de entrada + traza).
    #[serde(default)]
    pub kill_chain: Option<bool>,
    /// Usar el medidor de detección/"traza": barra de la UI, hints y resumen.
    #[serde(default)]
    pub trace: Option<bool>,
    /// Exigir "shell" (foothold, etapa POST) para explorar el VFS
    /// (`ls`/`cat`/`cd`/...). Ponlo en `Some(false)` para un dominio que quiera
    /// ficheros explorables sin la mecánica de intrusión.
    #[serde(default)]
    pub shell_for_vfs: Option<bool>,
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

/// Etapas por defecto: la kill chain del pentesting. Es el default pragmático
/// (la mayoría de campañas son de intrusión); un dominio distinto declara las
/// suyas en `Campaign.stages`.
pub(crate) fn default_stages() -> Vec<String> {
    vec![
        String::from("RECON"),
        String::from("ENUM"),
        String::from("EXPLOIT"),
        String::from("POST"),
    ]
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
    /// ¿La campaña usa las etapas por defecto (la kill chain del pentesting)? Es
    /// la base de la *inferencia heurística* del dominio cuando `domain` se omite.
    pub fn uses_default_stages(&self) -> bool {
        self.stages == default_stages()
    }

    /// Dominio efectivo de la campaña. Si `domain` está fijado, manda; si se
    /// omite, se infiere: pentest cuando usa las etapas por defecto, `Bare`
    /// cuando declara las suyas.
    pub fn domain_kind(&self) -> DomainKind {
        self.domain.unwrap_or_else(|| {
            if self.uses_default_stages() {
                DomainKind::Pentest
            } else {
                DomainKind::Bare
            }
        })
    }

    /// Default de las mecánicas de intrusión según el dominio activo. Cada toggle
    /// de [`Features`] puede seguir sobreescribiéndolo por separado.
    fn domain_pentest_default(&self) -> bool {
        self.domain_kind().defaults_pentest_mechanics()
    }

    /// ¿Mostrar la ayuda/hints de la kill chain? (toggle `features.kill_chain`,
    /// default según el dominio).
    pub fn kill_chain(&self) -> bool {
        self.features
            .kill_chain
            .unwrap_or_else(|| self.domain_pentest_default())
    }

    /// ¿Usar el medidor de detección/"traza"? (toggle `features.trace`, default
    /// según el dominio).
    pub fn uses_trace(&self) -> bool {
        self.features
            .trace
            .unwrap_or_else(|| self.domain_pentest_default())
    }

    /// ¿Exigir shell (foothold) para explorar el VFS? (toggle
    /// `features.shell_for_vfs`, default según el dominio).
    pub fn shell_for_vfs(&self) -> bool {
        self.features
            .shell_for_vfs
            .unwrap_or_else(|| self.domain_pentest_default())
    }

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
