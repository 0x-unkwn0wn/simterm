//! Localized text for generic engine mechanics.
//!
//! Campaign-authored story, host names, file contents, briefings, and endings
//! remain campaign data. This module only owns reusable engine UI strings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Es,
    En,
}

impl Default for Language {
    fn default() -> Self {
        Language::Es
    }
}

impl Language {
    pub fn text(self) -> EngineText {
        EngineText { language: self }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EngineText {
    language: Language,
}

impl EngineText {
    pub fn language(self) -> Language {
        self.language
    }

    pub fn help_hint(self) -> &'static str {
        match self.language {
            Language::Es => "Escribe 'help' para ver los comandos.",
            Language::En => "Type 'help' to see the commands.",
        }
    }

    pub fn resumed(self, level: usize, total: usize) -> String {
        match self.language {
            Language::Es => format!("Partida reanudada: misión {level}/{total}."),
            Language::En => format!("Game resumed: mission {level}/{total}."),
        }
    }

    pub fn reset_done(self) -> &'static str {
        match self.language {
            Language::Es => "Progreso borrado. Campaña reiniciada.",
            Language::En => "Progress deleted. Campaign restarted.",
        }
    }

    pub fn reset_hint(self) -> &'static str {
        match self.language {
            Language::Es => "(usa 'reset' para empezar una campaña nueva)",
            Language::En => "(use 'reset' to start a new campaign)",
        }
    }

    pub fn mission_header(self, n: usize, total: usize, name: &str) -> String {
        match self.language {
            Language::Es => format!("--- MISIÓN {n}/{total}: {name} ---"),
            Language::En => format!("--- MISSION {n}/{total}: {name} ---"),
        }
    }

    pub fn target_header(self, hostname: &str, ip: &str, os: &str, multi: bool) -> String {
        match self.language {
            Language::Es => format!(
                "objetivo {hostname}  ({ip})  os: {os}{}",
                if multi { "  · red interna" } else { "" }
            ),
            Language::En => format!(
                "target {hostname}  ({ip})  os: {os}{}",
                if multi { "  · internal network" } else { "" }
            ),
        }
    }

    pub fn entry_hint_active(self) -> &'static str {
        match self.language {
            Language::Es => "Fase RECON — escaneo activo: ejecuta 'nmap'.",
            Language::En => "RECON phase - active scan: run 'nmap'.",
        }
    }

    pub fn entry_hint_cold(self) -> &'static str {
        match self.language {
            Language::Es => {
                "El cliente ya señaló el servicio: empiezas en ENUM. Mira 'target' y enuméralo."
            }
            Language::En => {
                "The client already flagged the service: you start in ENUM. Check 'target' and enumerate it."
            }
        }
    }

    pub fn entry_hint_passive(self) -> &'static str {
        match self.language {
            Language::Es => {
                "Operación sigilosa: intercepta el tráfico con 'sniff' ('nmap' deja rastro extra)."
            }
            Language::En => {
                "Stealth operation: intercept traffic with 'sniff' ('nmap' leaves extra traces)."
            }
        }
    }

    pub fn entry_hint_pivot(self) -> &'static str {
        match self.language {
            Language::Es => "El objetivo está tras un bastión: usa 'connect' antes de escanear.",
            Language::En => "The target is behind a bastion: use 'connect' before scanning.",
        }
    }

    pub fn trace_hint(self, limit: f32, hint: &str) -> String {
        match self.language {
            Language::Es => format!("Traza máxima {:.0}. {hint}", limit),
            Language::En => format!("Max trace {:.0}. {hint}", limit),
        }
    }

    pub fn phase_reached(self, phase: &str) -> String {
        match self.language {
            Language::Es => format!("[fase] Has alcanzado la fase {phase}."),
            Language::En => format!("[phase] You reached the {phase} phase."),
        }
    }

    pub fn time_window_closed(self) -> &'static str {
        match self.language {
            Language::Es => {
                "!! VENTANA CERRADA — Se agotó el tiempo de la operación. OPERACIÓN ABORTADA."
            }
            Language::En => "!! WINDOW CLOSED - The operation ran out of time. OPERATION ABORTED.",
        }
    }

    pub fn achievement_unlocked(self, title: &str) -> String {
        match self.language {
            Language::Es => format!("[logro] DESBLOQUEADO: {title}"),
            Language::En => format!("[achievement] UNLOCKED: {title}"),
        }
    }

    pub fn level_completed(self) -> &'static str {
        match self.language {
            Language::Es => "## MISIÓN COMPLETADA — objetivo exfiltrado.",
            Language::En => "## MISSION COMPLETE - objective exfiltrated.",
        }
    }

    pub fn level_summary(
        self,
        level: usize,
        grade: &str,
        trace: f32,
        limit: f32,
        time: u32,
    ) -> String {
        match self.language {
            Language::Es => {
                format!("Cierre M{level} · grado {grade} · traza {trace:.0}/{limit:.0} · t={time}")
            }
            Language::En => format!(
                "Closeout M{level} · grade {grade} · trace {trace:.0}/{limit:.0} · t={time}"
            ),
        }
    }

    pub fn final_choice_prompt(self) -> &'static str {
        match self.language {
            Language::Es => "Tienes el objetivo final en tus manos. ¿Qué decides?",
            Language::En => "You have the final objective in hand. What do you do?",
        }
    }

    pub fn choose_hint(self) -> &'static str {
        match self.language {
            Language::Es => "Decide con 'choose <n>'.",
            Language::En => "Decide with 'choose <n>'.",
        }
    }

    pub fn no_pending_choice(self) -> &'static str {
        match self.language {
            Language::Es => "No hay ninguna decisión pendiente.",
            Language::En => "There is no pending decision.",
        }
    }

    pub fn invalid_choice(self, total: usize) -> String {
        match self.language {
            Language::Es => format!("Opción inválida. Elige entre 1 y {total} con 'choose <n>'."),
            Language::En => {
                format!("Invalid choice. Choose between 1 and {total} with 'choose <n>'.")
            }
        }
    }

    pub fn campaign_completed(self) -> &'static str {
        match self.language {
            Language::Es => "## CAMPAÑA COMPLETADA.",
            Language::En => "## CAMPAIGN COMPLETE.",
        }
    }

    pub fn campaign_summary(self, missions: usize, time: u32) -> String {
        match self.language {
            Language::Es => format!("Resumen · {missions} misiones · tiempo total t={time}"),
            Language::En => format!("Summary · {missions} missions · total time t={time}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn ron_accepts_language_codes() {
        assert_eq!(ron::de::from_str::<Language>("es").unwrap(), Language::Es);
        assert_eq!(ron::de::from_str::<Language>("en").unwrap(), Language::En);
    }
}
