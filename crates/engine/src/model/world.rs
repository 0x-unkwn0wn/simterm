//! Nodo genérico del mundo: identidad + sistema de archivos explorable, sin
//! semántica de dominio.
//!
//! Es la parte **neutral** de un "host": cualquier dominio tiene nodos con un
//! nombre y ficheros que explorar (un host en pentest, una estación en forense,
//! un subsistema de una nave). Los datos específicos de intrusión —servicios y
//! vulnerabilidades— son *payload del dominio pentest* y viven en su
//! [`crate::domains::pentest::target::TargetNode`], que por ahora mantiene estos
//! campos genéricos junto al payload por compatibilidad de formato. El sub-paso 5
//! de la generalización completa la separación de almacenamiento (el runtime
//! guardará un `WorldNode` neutral + el payload del dominio activo).

use serde::{Deserialize, Serialize};

use crate::model::filesystem::FsNode;

/// Identidad y contenido explorable de un nodo, sin nada de dominio.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldNode {
    /// Nombre del nodo (FQDN en pentest).
    pub hostname: String,
    /// Dirección (IP en pentest; libre en otros dominios).
    pub ip: String,
    /// Sistema/plataforma (SO en pentest).
    pub os: String,
    /// Sistema de archivos explorable del nodo.
    #[serde(default)]
    pub filesystem: Vec<FsNode>,
}

impl WorldNode {
    /// Nombre corto del nodo (primer segmento del FQDN).
    pub fn short_name(&self) -> &str {
        self.hostname.split('.').next().unwrap_or(&self.hostname)
    }
}
