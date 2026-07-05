//! Comandos declarativos de campaña: verbos definidos en datos (RON) con efectos
//! simples sobre la partida, sin escribir Rust.
//!
//! A diferencia de los `easter_eggs` (puro sabor, no tocan el estado), un
//! [`CampaignCommand`] puede imprimir líneas, poner/quitar flags persistentes,
//! sumar o restar traza, desbloquear un logro o completar la misión actual. Su
//! disponibilidad se puede condicionar a flags, a una misión concreta o a la
//! fase de la kill chain.
//!
//! Los EFECTOS se aplican en el runtime (`crate::runtime::actions::campaign_command`),
//! nunca en el frontend: el motor sigue siendo el dueño del estado de juego.

use serde::Deserialize;

/// Un comando declarativo definido por la campaña.
#[derive(Debug, Clone, Deserialize)]
pub struct CampaignCommand {
    /// Verbos que lo disparan (p. ej. `["inspect", "look"]`).
    pub triggers: Vec<String>,
    /// Líneas impresas en el log al ejecutarlo. Admiten el marcador `{clock}`.
    #[serde(default)]
    pub lines: Vec<String>,
    /// Efectos declarativos que se aplican, en orden, sobre el estado de juego.
    #[serde(default)]
    pub effects: Vec<CommandEffect>,
    /// Condiciones que deben cumplirse TODAS para que el comando esté disponible.
    /// Si alguna falla, el verbo se trata como no reconocido (cae a easter egg /
    /// desconocido), de modo que el comando puede aparecer/desaparecer por estado.
    #[serde(default)]
    pub conditions: Vec<CommandCondition>,
    /// Líneas mostradas cuando el comando PERTENECE al momento actual (sus
    /// condiciones de *alcance* —`Mission`, `Phase`, flags— se cumplen) pero
    /// fallan sus condiciones de **trabajo real** (`FileRead`, `RanCommand`). En
    /// vez de un críptico "command not found", el jugador ve por qué aún no puede
    /// entregar (p. ej. "primero lee la pista y ejecuta grep"). Si está vacío, un
    /// comando bloqueado cae a "desconocido" como antes. `{clock}` se sustituye.
    #[serde(default)]
    pub locked: Vec<String>,
    /// Si es `true`, el comando no se muestra en la ayuda ni en el autocompletado
    /// (verbo secreto). Sigue siendo ejecutable.
    #[serde(default)]
    pub hidden: bool,
}

/// Efecto declarativo simple aplicable por un [`CampaignCommand`].
#[derive(Debug, Clone, Deserialize)]
pub enum CommandEffect {
    /// Imprime una línea adicional en el log.
    AddLog(String),
    /// Activa una flag persistente de campaña.
    SetFlag(String),
    /// Desactiva una flag persistente de campaña.
    ClearFlag(String),
    /// Suma traza (valor positivo) o la reduce (valor negativo).
    AddTrace(f32),
    /// Modifica un medidor de campaña por su `id` (positivo suma, negativo resta)
    /// y evalúa su `on_limit`.
    AddMeter(String, f32),
    /// Avanza a la etapa nombrada (por su nombre en `Campaign.stages`, sin
    /// distinguir mayúsculas). Nunca retrocede. Permite que un dominio guíe su
    /// progresión de etapas solo con datos.
    ReachStage(String),
    /// Desbloquea un logro de campaña por su `id`.
    UnlockAchievement(String),
    /// Completa la misión actual (equivale a lograr el objetivo). Combínalo con
    /// `conditions` para exigir una flag u otra condición antes de cerrarla.
    CompleteMission,
}

/// Condición de disponibilidad de un [`CampaignCommand`].
#[derive(Debug, Clone, Deserialize)]
pub enum CommandCondition {
    /// La flag indicada está activa.
    FlagSet(String),
    /// La flag indicada NO está activa.
    FlagNotSet(String),
    /// Solo disponible dentro de la misión con este `Mission.id`.
    Mission(String),
    /// Solo disponible al haber alcanzado la etapa nombrada (por su nombre en
    /// `Campaign.stages`, sin distinguir mayúsculas). Por defecto (kill chain):
    /// `"recon"`, `"enum"`, `"exploit"` o `"post"`.
    Phase(String),
    /// El jugador ha leído esta ruta del VFS en el nivel actual (con `cat` o vía
    /// un pipeline). Verificación de trabajo real: exige haber CONSULTADO un
    /// fichero (una pista, un dataset) antes de avanzar. La ruta se compara ya
    /// normalizada (absoluta, p. ej. `"/pistas/globs.md"`).
    FileRead(String),
    /// El jugador ha ejecutado este verbo en el nivel actual (p. ej. `"grep"`,
    /// `"find"`). Verificación de trabajo real: exige haber USADO de verdad una
    /// herramienta antes de reclamar el nivel. Se compara sin distinguir
    /// mayúsculas.
    RanCommand(String),
}

impl CommandCondition {
    /// Nombre de fase canónico (minúsculas) si la condición es de fase.
    pub fn phase_name(&self) -> Option<&str> {
        match self {
            CommandCondition::Phase(p) => Some(p.as_str()),
            _ => None,
        }
    }
}
