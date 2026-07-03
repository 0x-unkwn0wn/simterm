//! `CoreState`: el estado de runtime **domain-agnóstico**.
//!
//! Es el núcleo de la separación `GameState` → núcleo + estado de dominio (Fase 2
//! de la generalización). `GameState` lo embebe como `core` y va migrando aquí,
//! de forma incremental y sin romper nada, los campos que no son de ningún
//! dominio concreto. Ya alberga: la **sesión** (logs, `running`, `clock`,
//! `outcome`, el cursor de etapa `stage`, el `cwd` del VFS), la **sesión de
//! shell** (overrides de `export`, `$?`), los **medidores de campaña** y el
//! **bookkeeping persistente**. Lo que queda en `GameState` es específico de
//! intrusión y acabará formando un `PentestState` (Fase C).

use std::collections::BTreeMap;

use crate::model::meter::MeterDef;
use crate::runtime::meter::Meter;

/// Desenlace de la campaña. Neutro: cualquier dominio gana o pierde igual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOutcome {
    Victory,
    Defeat,
}

#[derive(Debug, Clone, Default)]
pub struct CoreState {
    // ----- Sesión (neutra, común a todos los dominios) -----
    /// Registro de líneas de la consola/telemetría del nivel. Lo alimenta
    /// `GameState::log`. Se limpia en las transiciones de nivel.
    pub logs: Vec<String>,
    /// ¿Sigue viva la sesión? El frontend sale de su bucle cuando pasa a `false`.
    /// Arranca en `true` (ver [`CoreState::new`]).
    pub running: bool,
    /// Reloj del nivel activo (ticks). Se reinicia en cada nivel.
    pub clock: u32,
    /// Desenlace de la *campaña*: solo se fija al perder o al completarla. `None`
    /// mientras la partida sigue en curso.
    pub outcome: Option<GameOutcome>,
    /// Cursor de etapa de progresión: índice en `Campaign.stages`. Neutro; cada
    /// dominio le da su lectura (el pentest, como `Phase`). Se reinicia por nivel.
    pub stage: usize,
    /// Directorio de trabajo actual en el VFS (componentes de ruta; vacío = "/").
    /// Es navegación genérica del terminal, común a cualquier dominio con VFS.
    pub cwd: Vec<String>,

    /// Definición de los medidores de campaña del nivel activo (de la misión).
    pub meter_defs: Vec<MeterDef>,
    /// Valores vivos de esos medidores, por id. Vacío si el nivel no declara
    /// ninguno (la inmensa mayoría de campañas).
    pub meters: BTreeMap<String, Meter>,
    /// Overrides de entorno de la sesión (`export VAR=valor`). No se persisten y
    /// se reinician al cambiar de nivel (dejas la caja anterior).
    pub env_session: Vec<(String, String)>,
    /// Código de salida del último comando de shell (`$?`). No se persiste.
    pub last_exit: i32,

    // ----- Bookkeeping persistente de campaña (se guarda entre sesiones) -----
    /// Reloj acumulado de toda la campaña (para el resumen final).
    pub campaign_clock: u32,
    /// Logros data-driven de campaña desbloqueados (por id).
    pub campaign_achievements: Vec<String>,
    /// Flags persistentes de campaña, activadas por comandos declarativos
    /// (`SetFlag`). Persisten entre niveles y en el guardado.
    pub flags: Vec<String>,

    // ----- Flujo de cierre de nivel / final con elección -----
    /// Resumen del último nivel cerrado (se muestra en el debrief).
    pub last_summary: Option<String>,
    /// Epílogo elegido en el final con elección (se muestra en el cierre).
    pub epilogue: Option<Vec<String>>,
    /// Operación con final con elección: ¿esperando la decisión del jugador?
    pub awaiting_choice: bool,
}

impl CoreState {
    pub fn new() -> Self {
        CoreState {
            // La sesión arranca viva; `Default` daría `false` (bool por defecto).
            running: true,
            ..Self::default()
        }
    }
}
