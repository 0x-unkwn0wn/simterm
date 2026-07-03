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

pub mod pentest;
