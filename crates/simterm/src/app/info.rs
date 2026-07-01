//! Comandos de información/presentación de la consola: ayuda, datos del host,
//! hallazgos, identidad y estado. Solo construyen líneas de log; no tocan la
//! lógica del juego.

use simterm_engine::{FindingStatus, GameOutcome, Language};

use super::App;

impl App {
    pub(super) fn cmd_help(&mut self) {
        let lang = self.game.campaign.language;
        let phase = self.game.phase.label();
        let mut lines = match lang {
            Language::Es => vec![
                format!("--- AYUDA (fase actual: {phase}) ---"),
                String::from("Kill chain: RECON -> ENUM -> EXPLOIT -> POST. Cada fase habilita acciones."),
                String::from("[RECON] (la entrada varía según la operación)"),
                String::from("  nmap                 - escaneo activo: revela todos los servicios (t+5, ruido+4)"),
                String::from("  sniff                - interceptación pasiva: 1 servicio por uso (t+8, ruido+1)"),
                String::from("  connect [host]       - pivota tras un bastión (solo operaciones con bastión)"),
                String::from("  target               - datos del host y servicios descubiertos"),
                String::from("[ENUM] (enumera cada servicio con la herramienta ADECUADA a su tipo)"),
            ],
            Language::En => vec![
                format!("--- HELP (current phase: {phase}) ---"),
                String::from("Kill chain: RECON -> ENUM -> EXPLOIT -> POST. Each phase unlocks actions."),
                String::from("[RECON] (entry point depends on the operation)"),
                String::from("  nmap                 - active scan: reveals all services (t+5, noise+4)"),
                String::from("  sniff                - passive interception: 1 service per use (t+8, noise+1)"),
                String::from("  connect [host]       - pivot through a bastion (bastion operations only)"),
                String::from("  target               - host data and discovered services"),
                String::from("[ENUM] (enumerate each service with the RIGHT tool for its type)"),
            ],
        };
        for t in simterm_engine::toolbox::TOOLS {
            let affi = if t.affinities.is_empty() {
                match lang {
                    Language::Es => String::from("cualquiera"),
                    Language::En => String::from("any"),
                }
            } else {
                t.affinities
                    .iter()
                    .map(|c| c.label_in(lang))
                    .collect::<Vec<_>>()
                    .join("/")
            };
            let arg = match lang {
                Language::Es => "puerto",
                Language::En => "port",
            };
            let affinity = match lang {
                Language::Es => "afín",
                Language::En => "fit",
            };
            let noise = match lang {
                Language::Es => "ruido",
                Language::En => "noise",
            };
            lines.push(format!(
                "  {:<11}<{}> - {} [{}: {}, {} {:.0}]",
                t.name,
                arg,
                t.desc_in(lang),
                affinity,
                affi,
                noise,
                t.noise
            ));
        }
        let rest: &[&str] = match lang {
            Language::Es => &[
                "  searchsploit <id>    - investiga un hallazgo (poco ruido; precisión ~78%)",
                "  intel                - lista los hallazgos con su confianza estimada",
                "[EXPLOIT]",
                "  exploit <id>         - explota un hallazgo; éxito = shell de usuario (fase POST)",
                "  login                - foothold determinista si reutilizas una credencial válida",
                "  netmap               - (red interna) descubre hosts vecinos desde un host comprometido",
                "  pivot <host>         - (red interna) cambia el contexto a otro host alcanzable",
                "[POST] (tras conseguir shell — explora el sistema de archivos)",
                "  ls [ruta]            - lista un directorio",
                "  cd [ruta] / pwd      - cambia de directorio / muestra el actual",
                "  cat <ruta>           - lee un fichero (lore, credenciales, objetivo)",
                "  find [texto]         - busca ficheros por nombre",
                "  john <ruta>          - rompe un hash saqueado (alias: hashcat)",
                "  strings <ruta>       - extrae cadenas de un binario reversible",
                "  disasm <ruta>        - pseudo-desensambla un binario (alias: objdump/r2)",
                "  solve <ruta> <sec>   - entrega el secreto extraído por reversing",
                "  base64 <ruta>        - decodifica un fichero Base64",
                "  xor <ruta> <clave>   - decodifica un fichero XOR",
                "  linpeas              - enumera escalada local (cubre sudo/suid/kernel/cron)",
                "  sudo -l / suid / sysinfo - chequeos locales específicos de privesc",
                "  privesc              - escala a root (desbloquea ficheros protegidos)",
                "  cleanup              - encubre tu rastro: baja la traza (coste y riesgo crecientes)",
                "  loot                 - muestra el botín recogido",
                "  >> objetivo: exfiltra con 'cat' el fichero objetivo para completar el nivel",
                "[GENERAL]",
                "  whoami               - identidad de la sesión actual",
                "  status / logs        - resumen de estado · ir al final del registro",
                "  logros               - muestra logros desbloqueados y pendientes",
                "  clear / quit         - limpia la consola · salir del juego",
                "  reset                - reinicia la campaña (borra el progreso guardado)",
                "Pistas: la herramienta adecuada da hallazgos reales con poco ruido; la inadecuada,",
                "ruido y falsos positivos. La confianza es una ESTIMACIÓN: investiga antes de explotar.",
                "Teclas: Tab autocompleta · ↑/↓ historial · RePág/AvPág scroll · Esc limpia línea.",
                "Rumor de campo: el sistema responde a más señales de las que figuran aquí.",
            ],
            Language::En => &[
                "  searchsploit <id>    - research a finding (low noise; ~78% accuracy)",
                "  intel                - list findings with estimated confidence",
                "[EXPLOIT]",
                "  exploit <id>         - exploit a finding; success = user shell (POST phase)",
                "  login                - deterministic foothold with a valid reused credential",
                "  netmap               - (internal network) discover neighboring hosts from a compromised host",
                "  pivot <host>         - (internal network) move context to a reachable host",
                "[POST] (after getting a shell - explore the filesystem)",
                "  ls [path]            - list a directory",
                "  cd [path] / pwd      - change directory / show current directory",
                "  cat <path>           - read a file (lore, credentials, objective)",
                "  find [text]          - search files by name",
                "  john <path>          - crack a looted hash (alias: hashcat)",
                "  strings <path>       - extract strings from a reversible binary",
                "  disasm <path>        - pseudo-disassemble a binary (alias: objdump/r2)",
                "  solve <path> <sec>   - submit the secret extracted by reversing",
                "  base64 <path>        - decode a Base64 file",
                "  xor <path> <key>     - decode a XOR file",
                "  linpeas              - enumerate local privesc (covers sudo/suid/kernel/cron)",
                "  sudo -l / suid / sysinfo - specific local privesc checks",
                "  privesc              - escalate to root (unlocks protected files)",
                "  cleanup              - cover your tracks: lowers trace (increasing cost and risk)",
                "  loot                 - show collected loot",
                "  >> objective: exfiltrate the objective file with 'cat' to complete the level",
                "[GENERAL]",
                "  whoami               - current session identity",
                "  status / logs        - status summary / jump to the end of the log",
                "  logros               - show unlocked and pending achievements",
                "  clear / quit         - clear console / exit the game",
                "  reset                - restart the campaign (deletes saved progress)",
                "Tips: the right tool finds real issues with low noise; the wrong one creates",
                "noise and false positives. Confidence is an ESTIMATE: research before exploiting.",
                "Keys: Tab autocomplete · Up/Down history · PgUp/PgDn scroll · Esc clears line.",
                "Field rumor: the system responds to more signals than the manual lists.",
            ],
        };
        for line in rest {
            lines.push(line.to_string());
        }

        // Referencia rápida de comandos, generada desde el registro único
        // (`registry::reference_lines`): alias, uso y naturaleza de cada built-in.
        lines.push(match lang {
            Language::Es => {
                String::from("--- REFERENCIA RÁPIDA (desde el registro de comandos) ---")
            }
            Language::En => String::from("--- QUICK REFERENCE (from the command registry) ---"),
        });
        lines.extend(crate::registry::reference_lines());

        // Comandos declarativos definidos por la campaña (no ocultos). Su metadata
        // vive en los datos de la campaña, no en el registro de built-ins.
        let campaign_cmds: Vec<String> = self
            .game
            .campaign
            .commands
            .iter()
            .filter(|c| !c.hidden && !c.triggers.is_empty())
            .map(|c| {
                let verbs = c.triggers.join(", ");
                match c.lines.first() {
                    Some(first) => format!("  {verbs:<20} - {first}"),
                    None => format!("  {verbs}"),
                }
            })
            .collect();
        if !campaign_cmds.is_empty() {
            lines.push(match lang {
                Language::Es => String::from("[CAMPAÑA] (comandos definidos por esta campaña)"),
                Language::En => String::from("[CAMPAIGN] (commands defined by this campaign)"),
            });
            lines.extend(campaign_cmds);
        }

        // Comandos de terminal autorados (no ocultos): sabor de shell de la campaña.
        let terminal_cmds: Vec<String> = self
            .game
            .campaign
            .terminal
            .iter()
            .filter(|c| !c.hidden && !c.triggers.is_empty())
            .map(|c| format!("  {}", c.triggers.join(", ")))
            .collect();
        if !terminal_cmds.is_empty() {
            lines.push(match lang {
                Language::Es => String::from("[TERMINAL] (comandos de shell de esta campaña)"),
                Language::En => String::from("[TERMINAL] (shell commands of this campaign)"),
            });
            lines.extend(terminal_cmds);
        }

        for l in lines {
            self.game.log(l);
        }
    }

    pub(super) fn cmd_target(&mut self) {
        let lang = self.game.campaign.language;
        let t = &self.game.target;
        let mut lines = vec![
            String::from("--- NODO OBJETIVO ---"),
            format!("hostname : {}", t.hostname),
            format!("ip       : {}", t.ip),
            format!("os       : {}", t.os),
        ];

        let discovered: Vec<_> = t
            .services
            .iter()
            .filter(|s| self.game.discovered_ports.contains(&s.port))
            .cloned()
            .collect();

        if discovered.is_empty() {
            lines.push(String::from(
                "servicios: DESCONOCIDOS — ejecuta 'nmap' para descubrirlos.",
            ));
        } else {
            lines.push(String::from("servicios descubiertos:"));
            for s in &discovered {
                let cat = simterm_engine::toolbox::category(&s.name);
                lines.push(format!(
                    "  - {:>5}/tcp  {:<6} {:<16} [{}]",
                    s.port,
                    s.name,
                    s.version,
                    cat.label_in(lang)
                ));
            }
        }
        lines.push(String::from(
            "(las vulnerabilidades siguen ocultas: enumera cada servicio)",
        ));
        for l in lines {
            self.game.log(l);
        }
    }

    pub(super) fn cmd_intel(&mut self) {
        if self.game.intel.is_empty() {
            self.game.log(String::from(
                "Sin hallazgos todavía. Descubre servicios ('nmap') y enuméralos.",
            ));
            return;
        }
        self.game.log(String::from(
            "--- HALLAZGOS (la confianza es una estimación) ---",
        ));
        // Clonamos las líneas para no chocar con el préstamo mutable del log.
        let rows: Vec<String> = self
            .game
            .intel
            .iter()
            .map(|f| {
                let lang = self.game.campaign.language;
                format!(
                    "  #{:<3} [{:>3}%] {:<14} src:{:<9} {}",
                    f.public_id,
                    f.confidence_pct(),
                    f.status.label_in(lang),
                    f.source.label_in(lang),
                    f.title
                )
            })
            .collect();
        for r in rows {
            self.game.log(r);
        }
    }

    pub(super) fn cmd_status(&mut self) {
        let g = &self.game;
        let total = g.intel.len();
        let verified = g
            .intel
            .iter()
            .filter(|f| {
                matches!(
                    f.status,
                    FindingStatus::VerifiedTrue | FindingStatus::VerifiedFalse
                )
            })
            .count();
        let outcome = match g.outcome {
            Some(GameOutcome::Victory) => "VICTORIA",
            Some(GameOutcome::Defeat) => "DERROTA",
            None => "en curso",
        };
        let time_line = match g.time_remaining() {
            Some(rem) => format!(
                "tiempo op.    : t={}/{}  (quedan {})",
                g.clock,
                g.time_limit.unwrap_or(0),
                rem
            ),
            None => format!("tiempo op.    : t={}  (sin ventana)", g.clock),
        };
        let lines = vec![
            String::from("--- ESTADO ---"),
            format!(
                "nivel         : {}/{}  {}",
                g.level_number(),
                g.level_count(),
                g.level_name()
            ),
            format!(
                "fase          : {}   skill efectivo {:.2}",
                g.phase.label(),
                g.effective_skill()
            ),
            format!(
                "shell         : {}   cwd {}",
                if !g.has_foothold() {
                    "sin acceso"
                } else if g.is_root {
                    "root"
                } else {
                    "usuario"
                },
                g.cwd_display()
            ),
            time_line,
            format!(
                "detección     : {:.0}/{:.0}  (ruido total {:.0})",
                g.detection.detection, g.detection_limit, g.detection.total_noise
            ),
            format!("hallazgos     : {total} ({verified} con veredicto de verificación)"),
            format!("campaña       : {outcome}"),
        ];
        for l in lines {
            self.game.log(l);
        }
    }

    pub(super) fn cmd_logs(&mut self) {
        self.follow = true;
        self.game
            .log(String::from("--- registro al día (final del log) ---"));
    }

    pub(super) fn cmd_achievements(&mut self) {
        let lang = self.game.campaign.language;
        let unlocked = self.game.achievements.len() + self.game.campaign_achievements.len();
        let total = simterm_engine::ACHIEVEMENTS.len() + self.game.campaign.achievements.len();
        self.game
            .log(format!("--- LOGROS ({unlocked}/{total}) ---"));

        self.game.log(String::from("[motor]"));
        for id in simterm_engine::ACHIEVEMENTS {
            let mark = if self.game.achievements.contains(id) {
                "[x]"
            } else {
                "[ ]"
            };
            self.game.log(format!(
                "{mark} {} - {}",
                id.title_in(lang),
                id.description_in(lang)
            ));
        }

        if !self.game.campaign.achievements.is_empty() {
            self.game.log(String::from("[campaña]"));
            let rows: Vec<String> = self
                .game
                .campaign
                .achievements
                .iter()
                .map(|achievement| {
                    let mark = if self
                        .game
                        .campaign_achievements
                        .iter()
                        .any(|id| id == &achievement.id)
                    {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    if achievement.description.is_empty() {
                        format!("{mark} {}", achievement.title)
                    } else {
                        format!("{mark} {} - {}", achievement.title, achievement.description)
                    }
                })
                .collect();
            for row in rows {
                self.game.log(row);
            }
        }
    }
}
