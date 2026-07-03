//! Capa de **runtime**: el estado mutable de una partida y las acciones que lo
//! transforman. Opera sobre una `Campaign` (definición) que recibe ya cargada.

pub mod balance;
pub mod core;
pub mod meter;
pub mod probability;
pub mod state;
pub mod sysemu;

// Reubicado en el dominio de pentesting (Fase 1 de la generalización). Se
// re-exporta aquí para que `crate::runtime::actions` siga resolviendo sin tocar
// a los consumidores.
pub use crate::domains::pentest::actions;
