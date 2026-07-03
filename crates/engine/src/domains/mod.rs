//! Módulos de **dominio**: cada uno da semántica a un tipo de escenario.
//!
//! El núcleo del motor (terminal emulada, progresión de campaña, VFS, comandos
//! declarativos, medidores...) es agnóstico al tema. Un *dominio* es la capa que
//! convierte ese núcleo en una experiencia concreta: aporta sus verbos, su
//! estado y sus mecánicas. El pentesting/intrusión es el dominio de referencia;
//! forense, operación de un satélite o pilotaje de una nave serían hermanos.
//!
//! Esta es la primera fase de la generalización: por ahora el dominio de pentest
//! solo está *reubicado* aquí (mismo comportamiento, frontera visible). Las fases
//! siguientes generalizan etapas y medidores y extraen `GameState` a un núcleo
//! neutral + estado de dominio.

/// El identificador del dominio (dato de campaña) vive en la capa de modelo para
/// mantener la dependencia `domains → model`. Se re-exporta aquí para que el
/// resto del motor lo consuma como `crate::domains::DomainKind`.
pub use crate::model::campaign::DomainKind;

use crate::model::campaign::Campaign;
use crate::runtime::core::CoreState;

/// Contrato entre el **núcleo neutral** y el **dominio activo**. Cada dominio
/// in-tree (hoy solo pentest; mañana sysadmin, forense...) lo implementa sobre su
/// estado, y el núcleo dispara estos enganches sin conocer el dominio concreto.
///
/// El núcleo pasa `core` (estado neutral: cursor de etapa, `cwd`, reloj...) y la
/// `Campaign` por referencia; el dominio decide qué significan para él. Así el
/// mismo `GameState`/frontend sirve a cualquier dominio. Ver [`DomainKind`] para
/// el conjunto cerrado de dominios y [`crate::runtime::state::DomainState`] para
/// su almacenamiento.
pub trait Domain {
    /// Prompt de la shell. El pentest muestra la consola del operador antes del
    /// foothold y `user@host:cwd$` después; otro dominio puede devolver el suyo.
    fn prompt(&self, campaign: &Campaign, core: &CoreState) -> String;

    /// Efectos del dominio por cada tick de reloj (además de la deriva neutral de
    /// medidores, que la lleva el núcleo). El pentest sube la traza por *dwell*
    /// mientras no haya foothold; un dominio sin traza no hace nada.
    fn on_tick(&mut self, core: &CoreState, ticks: u32);
}

pub mod pentest;
