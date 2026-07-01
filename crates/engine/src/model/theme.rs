//! Tematización y textos de presentación de una campaña.
//!
//! TODO el texto con identidad (branding, narrativa cosmética, easter eggs) vive
//! aquí, como DATOS de la campaña. El motor solo provee *defaults neutrales y
//! genéricos* para que arranque aunque la campaña no defina nada: ninguno de
//! esos defaults pertenece a una historia concreta.
//!
//! Una campaña puede sobreescribir cualquier campo en su `campaign.ron`.

use serde::Deserialize;

/// Textos de marca y cosméticos que la interfaz muestra fuera de las misiones.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Theme {
    /// Título corto de la barra superior (chip): el nombre corto del juego.
    pub app_title: String,
    /// Cabecera del panel de arranque (overlay de boot).
    pub boot_header: String,
    /// Líneas del overlay de arranque (typewriter al iniciar sesión).
    pub boot_lines: Vec<String>,
    /// Título del panel de overlays (briefing/debrief).
    pub overlay_title: String,
    /// Título del overlay de alerta crítica.
    pub alert_title: String,
    /// Prompt de la consola antes de tener foothold (p. ej. "user@host:~$ ").
    pub operator_prompt: String,
    /// Cuatro grados de sigilo, de mejor (menos traza) a peor.
    pub stealth_grades: Vec<String>,
    /// Mensajes de la defensa activa, uno por etapa (ver runtime::balance).
    pub defense_messages: Vec<String>,
    /// Líneas del overlay de operación abortada (derrota por traza).
    pub aborted_lines: Vec<String>,
    /// Rollo de créditos de fin de campaña.
    pub credits: Vec<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            app_title: String::from("TERMINAL"),
            boot_header: String::from("T E R M I N A L"),
            boot_lines: vec![
                String::from("Enlace establecido."),
                String::from("Sesión iniciada."),
            ],
            overlay_title: String::from(" TERMINAL "),
            alert_title: String::from(" ALERTA "),
            operator_prompt: String::from("operator@console:~$ "),
            stealth_grades: vec![
                String::from("FANTASMA"),
                String::from("LIMPIO"),
                String::from("OPERATIVO"),
                String::from("DESCUIDADO"),
            ],
            defense_messages: vec![
                String::from("[DEFENSA] Rastreo activo: han correlacionado tu actividad. Exploits y escaladas más difíciles a partir de ahora."),
                String::from("[DEFENSA] Contramedidas desplegadas: rotación de credenciales y endurecimiento. La traza se acelera."),
                String::from("[DEFENSA] Purga en curso: el equipo azul cierra el cerco. Termina y sal, o te pierden."),
            ],
            aborted_lines: vec![
                String::from("La traza ha alcanzado el umbral."),
                String::from("Han localizado el origen de la conexión."),
                String::from("Enlace cortado. Operación abortada."),
            ],
            credits: vec![
                String::from("FIN"),
                String::new(),
                String::from("Campaña completada."),
            ],
        }
    }
}

impl Theme {
    /// Grado de sigilo según la fracción de traza dejada (0.0..=1.0).
    pub fn grade(&self, ratio: f32) -> &str {
        let n = self.stealth_grades.len().max(1);
        // Reparte el ratio en `n` tramos iguales.
        let idx = ((ratio.clamp(0.0, 0.999) * n as f32) as usize).min(n - 1);
        self.stealth_grades
            .get(idx)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Mensaje de la etapa de defensa `stage` (1..). Si la campaña no define
    /// suficientes, repite el último.
    pub fn defense_message(&self, stage: u8) -> &str {
        if self.defense_messages.is_empty() {
            return "";
        }
        let i = (stage.max(1) as usize - 1).min(self.defense_messages.len() - 1);
        &self.defense_messages[i]
    }
}

/// Un easter egg: comandos ocultos que responden con texto temático. No afectan
/// a la partida. Las líneas admiten el marcador `{clock}`, sustituido por el
/// reloj interno de la operación al mostrarse.
#[derive(Debug, Clone, Deserialize)]
pub struct EasterEgg {
    /// Verbos que lo disparan (p. ej. ["sudo"], ["top", "ps", "htop"]).
    pub triggers: Vec<String>,
    /// Respuesta (una línea por entrada).
    pub lines: Vec<String>,
}
