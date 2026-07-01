//! Comandos de terminal autorados por la campaña (salida realista, presentacional).
//!
//! Son el nivel de "sabor de shell" por encima de los `easter_eggs`: además de
//! imprimir líneas, admiten **plantillas** (`{clock}`, `$VAR`, `{env:NOMBRE}`),
//! **respuestas por argumento** y un **código de salida** (`$?`). No cambian el
//! estado de juego (para efectos están los `commands` declarativos); solo emulan
//! una CLI que el motor no puede sintetizar (p. ej. `systemctl status nginx`).

use serde::Deserialize;

/// Un comando de terminal definido por la campaña.
#[derive(Debug, Clone, Deserialize)]
pub struct TerminalCommand {
    /// Verbos que lo disparan (p. ej. `["systemctl"]`).
    pub triggers: Vec<String>,
    /// Salida por defecto (cuando ningún caso de `args` coincide). Admite plantillas.
    #[serde(default)]
    pub output: Vec<String>,
    /// Respuestas por argumento: la clave es la cadena de argumentos exacta tras el
    /// verbo (p. ej. `"status nginx"`); el valor, las líneas de salida.
    #[serde(default)]
    pub args: Vec<(String, Vec<String>)>,
    /// Código de salida que deja en `$?` (0 = éxito).
    #[serde(default)]
    pub exit: i32,
    /// Si es `true`, no se muestra en la ayuda ni en el autocompletado.
    #[serde(default)]
    pub hidden: bool,
}

impl TerminalCommand {
    /// Devuelve las líneas de salida para la cadena de argumentos `arg_line`
    /// (vacía = sin argumentos). Prefiere una coincidencia exacta en `args`; si no,
    /// cae en `output`.
    pub fn resolve(&self, arg_line: &str) -> &[String] {
        self.args
            .iter()
            .find(|(k, _)| k == arg_line)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&self.output)
    }
}
