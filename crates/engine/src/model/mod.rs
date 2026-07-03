//! Capa de **definición**: tipos de datos inmutables que describen una campaña.
//! Se cargan desde RON. No contienen estado de partida ni contenido incrustado.

pub mod campaign;
pub mod command;
pub mod filesystem;
pub mod intel;
pub mod language;
pub mod meter;
pub mod mission;
/// Reubicado en el dominio de pentesting (Fase 1 de la generalización). Se
/// re-exporta aquí para que `crate::model::target` siga resolviendo.
pub use crate::domains::pentest::target;
pub mod terminal;
pub mod theme;
pub mod toolbox;
pub mod world;
