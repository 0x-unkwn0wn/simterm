//! `CoreState`: el estado de runtime **domain-agnóstico**.
//!
//! Es el núcleo naciente de la separación `GameState` → núcleo + estado de
//! dominio (sub-paso 5 de la generalización). `GameState` lo embebe como
//! `core` y va migrando aquí, de forma incremental y sin romper nada, los campos
//! que no son de ningún dominio concreto. De momento alberga la **sesión de
//! shell** (overrides de `export`, `$?`) y los **medidores de campaña**: piezas
//! puramente genéricas que cualquier dominio (pentest, forense, satélite) usa
//! igual. El resto del estado sigue en `GameState` y migra en pasadas sucesivas;
//! los campos específicos de intrusión acabarán formando un `DomainState`.

use std::collections::BTreeMap;

use crate::model::meter::MeterDef;
use crate::runtime::meter::Meter;

#[derive(Debug, Clone, Default)]
pub struct CoreState {
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
}

impl CoreState {
    pub fn new() -> Self {
        Self::default()
    }
}
