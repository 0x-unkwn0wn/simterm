//! Capa de **runtime**: el estado mutable de una partida y las acciones que lo
//! transforman. Opera sobre una `Campaign` (definición) que recibe ya cargada.

pub mod actions;
pub mod balance;
pub mod detection;
pub mod probability;
pub mod state;
pub mod sysemu;
