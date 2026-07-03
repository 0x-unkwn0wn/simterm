//! Registro único de metadatos de comandos del frontend.
//!
//! Antes, la información de cada comando (nombre, alias, categoría, ayuda) estaba
//! repartida entre el parser (`command.rs`), la ayuda (`app/info.rs`), el
//! autocompletado (`completion.rs`) y la documentación. Este módulo la centraliza
//! en una única fuente de verdad para el FRONTEND.
//!
//! El parser sigue siendo la autoridad sobre el mapeo verbo → [`crate::command::Command`]
//! (para no cambiar el comportamiento), pero el catálogo de nombres/alias y su
//! metadata se leen desde aquí: el autocompletado consume [`all_verbs`], la ayuda
//! se apoya en las categorías, y el validador del motor (`--doctor`) recibe
//! [`reserved_verbs`] para detectar colisiones de easter eggs y comandos
//! declarativos con la mecánica del juego.
//!
//! El motor NO depende de este módulo: los comandos "solo presentación" viven en
//! el frontend, así que la lista de verbos reservados se le pasa como datos
//! neutrales al validador.

use simterm_engine::toolbox;

/// Naturaleza de un comando, para documentación y validación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// Cambia el estado de juego; implementado en el runtime del motor.
    EngineBuiltin,
    /// Solo presentación (ayuda, estado, historial...); vive en el frontend.
    FrontendOnly,
    /// Mecánica genérica de minijuego del frontend.
    Minigame,
    /// Verbo reservado que la campaña PUEDE reutilizar como sabor (p. ej. `sudo`,
    /// cuya forma `sudo -l` es built-in pero cuya forma desnuda queda libre).
    FlavorReserved,
}

/// Categoría temática de un comando (agrupa la ayuda y la documentación).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    General,
    Recon,
    Enum,
    Findings,
    MultiHost,
    Vfs,
    System,
    Offline,
    LocalPrivesc,
    Endings,
    Minigame,
}

/// Metadatos de un comando del frontend.
pub struct CommandSpec {
    /// Nombre canónico (el que documenta la ayuda).
    pub name: &'static str,
    /// Alias equivalentes.
    pub aliases: &'static [&'static str],
    /// Categoría temática.
    pub category: Category,
    /// Descripción corta.
    pub summary: &'static str,
    /// Uso, si toma argumentos.
    pub usage: Option<&'static str>,
    /// ¿Requiere/acepta argumentos?
    pub takes_args: bool,
    /// Naturaleza del comando.
    pub kind: CommandKind,
}

impl CommandSpec {
    /// ¿Puede una campaña reutilizar este verbo como easter egg / comando
    /// declarativo sin que quede oculto por la mecánica del juego?
    pub fn allows_flavor(&self) -> bool {
        matches!(self.kind, CommandKind::FlavorReserved)
    }
}

/// Catálogo de comandos del frontend (los verbos de enumeración con afinidad de
/// servicio se añaden aparte, desde `toolbox::TOOLS`).
pub const COMMANDS: &[CommandSpec] = &[
    // -------------------------------- General --------------------------------
    CommandSpec {
        name: "help",
        aliases: &["h", "?"],
        category: Category::General,
        summary: "Ayuda de la fase actual (usa 'help all' para verlo todo).",
        usage: Some("help [all]"),
        takes_args: true,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "status",
        aliases: &[],
        category: Category::General,
        summary: "Resumen de nivel, fase, shell y detección.",
        usage: None,
        takes_args: false,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "logs",
        aliases: &[],
        category: Category::General,
        summary: "Salta al final del registro.",
        usage: None,
        takes_args: false,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "logros",
        aliases: &["logro", "achievements", "achievement"],
        category: Category::General,
        summary: "Lista logros desbloqueados y pendientes.",
        usage: None,
        takes_args: false,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "history",
        aliases: &[],
        category: Category::General,
        summary: "Muestra el historial de comandos.",
        usage: None,
        takes_args: false,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "echo",
        aliases: &[],
        category: Category::General,
        summary: "Imprime el texto introducido.",
        usage: Some("echo <texto>"),
        takes_args: true,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "clear",
        aliases: &["cls"],
        category: Category::General,
        summary: "Limpia la consola visible.",
        usage: None,
        takes_args: false,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "reset",
        aliases: &["newgame"],
        category: Category::General,
        summary: "Reinicia la campaña y borra el progreso.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "quit",
        aliases: &["exit", "q"],
        category: Category::General,
        summary: "Sale del juego.",
        usage: None,
        takes_args: false,
        kind: CommandKind::FrontendOnly,
    },
    // ---------------------------- Recon y descubrimiento ----------------------------
    CommandSpec {
        name: "target",
        aliases: &["host"],
        category: Category::Recon,
        summary: "Muestra el host actual y sus servicios.",
        usage: None,
        takes_args: false,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "nmap",
        aliases: &["scan", "recon"],
        category: Category::Recon,
        summary: "Reconocimiento activo de servicios.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "sniff",
        aliases: &["intercept", "listen"],
        category: Category::Recon,
        summary: "Descubrimiento pasivo, un servicio por uso.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "connect",
        aliases: &[],
        category: Category::Recon,
        summary: "Pivota a través de un bastión de entrada.",
        usage: Some("connect [host]"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    // --------------------------------- Enumeración ---------------------------------
    // (probe/nikto/gobuster/enum4linux/hydra/sqlmap se añaden desde toolbox::TOOLS)

    // ------------------------------ Hallazgos y acciones ------------------------------
    CommandSpec {
        name: "intel",
        aliases: &[],
        category: Category::Findings,
        summary: "Lista los hallazgos y su confianza estimada.",
        usage: None,
        takes_args: false,
        kind: CommandKind::FrontendOnly,
    },
    CommandSpec {
        name: "searchsploit",
        aliases: &["verify", "research"],
        category: Category::Findings,
        summary: "Investiga un hallazgo.",
        usage: Some("searchsploit <id>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "exploit",
        aliases: &["run"],
        category: Category::Findings,
        summary: "Intenta explotar un hallazgo.",
        usage: Some("exploit <id>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "login",
        aliases: &["ssh"],
        category: Category::Findings,
        summary: "Foothold determinista con credencial reutilizada.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "cleanup",
        aliases: &["covertracks", "cleanlogs"],
        category: Category::Findings,
        summary: "Reduce la traza con coste y riesgo.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    // --------------------------------- Multi-host ---------------------------------
    CommandSpec {
        name: "netmap",
        aliases: &["lan", "neighbors"],
        category: Category::MultiHost,
        summary: "Descubre hosts internos alcanzables.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "pivot",
        aliases: &["jump"],
        category: Category::MultiHost,
        summary: "Cambia el contexto a un host alcanzable.",
        usage: Some("pivot <host>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    // ------------------------------------ VFS ------------------------------------
    CommandSpec {
        name: "ls",
        aliases: &["dir"],
        category: Category::Vfs,
        summary: "Lista un directorio.",
        usage: Some("ls [ruta]"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "cd",
        aliases: &[],
        category: Category::Vfs,
        summary: "Cambia de directorio.",
        usage: Some("cd [ruta]"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "pwd",
        aliases: &[],
        category: Category::Vfs,
        summary: "Muestra el directorio actual.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "cat",
        aliases: &["read", "type"],
        category: Category::Vfs,
        summary: "Lee un fichero y recoge botín local.",
        usage: Some("cat <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "exfil",
        aliases: &[],
        category: Category::Vfs,
        summary: "Extrae el fichero objetivo y completa el nivel.",
        usage: Some("exfil <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "find",
        aliases: &[],
        category: Category::Vfs,
        summary: "Busca ficheros por nombre.",
        usage: Some("find [texto]"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "loot",
        aliases: &["creds"],
        category: Category::Vfs,
        summary: "Muestra el botín recogido.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    // ------------------------ Sistema (shell POSIX emulada) ------------------------
    CommandSpec {
        name: "whoami",
        aliases: &[],
        category: Category::System,
        summary: "Nombre del usuario actual.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "id",
        aliases: &[],
        category: Category::System,
        summary: "uid/gid del usuario actual.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "uname",
        aliases: &[],
        category: Category::System,
        summary: "Información del kernel/sistema.",
        usage: Some("uname [-a]"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "hostname",
        aliases: &[],
        category: Category::System,
        summary: "Nombre del host.",
        usage: Some("hostname [-f]"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "ps",
        aliases: &[],
        category: Category::System,
        summary: "Lista de procesos (sintetizada de los servicios).",
        usage: Some("ps [aux]"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "netstat",
        aliases: &["ss"],
        category: Category::System,
        summary: "Puertos a la escucha (de los servicios del host).",
        usage: Some("netstat -tlnp"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "ifconfig",
        aliases: &["ip"],
        category: Category::System,
        summary: "Interfaces de red (de la IP del host).",
        usage: None,
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "env",
        aliases: &[],
        category: Category::System,
        summary: "Variables de entorno.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "export",
        aliases: &[],
        category: Category::System,
        summary: "Define una variable de entorno de sesión.",
        usage: Some("export VAR=valor"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "grep",
        aliases: &[],
        category: Category::System,
        summary: "Filtra líneas de un fichero por patrón.",
        usage: Some("grep PATRÓN <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "head",
        aliases: &[],
        category: Category::System,
        summary: "Primeras líneas de un fichero.",
        usage: Some("head [-n N] <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "tail",
        aliases: &[],
        category: Category::System,
        summary: "Últimas líneas de un fichero.",
        usage: Some("tail [-n N] <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "wc",
        aliases: &[],
        category: Category::System,
        summary: "Cuenta líneas/palabras/bytes de un fichero.",
        usage: Some("wc <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "file",
        aliases: &[],
        category: Category::System,
        summary: "Tipo de un fichero.",
        usage: Some("file <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    // --------------------------- Análisis / reversing offline ---------------------------
    CommandSpec {
        name: "john",
        aliases: &["hashcat"],
        category: Category::Offline,
        summary: "Rompe un hash saqueado offline.",
        usage: Some("john <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "strings",
        aliases: &[],
        category: Category::Offline,
        summary: "Cadenas imprimibles de un binario.",
        usage: Some("strings <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "disasm",
        aliases: &["objdump", "r2"],
        category: Category::Offline,
        summary: "Pseudo-desensamblado de un binario.",
        usage: Some("disasm <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "solve",
        aliases: &[],
        category: Category::Offline,
        summary: "Entrega el secreto extraído de un binario.",
        usage: Some("solve <ruta> <secreto>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "base64",
        aliases: &[],
        category: Category::Offline,
        summary: "Decodifica un fichero Base64.",
        usage: Some("base64 <ruta>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "xor",
        aliases: &[],
        category: Category::Offline,
        summary: "Decodifica un fichero XOR.",
        usage: Some("xor <ruta> <clave>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    // ----------------------------- Escalada local (POST) -----------------------------
    CommandSpec {
        name: "privesc",
        aliases: &["escalate", "root"],
        category: Category::LocalPrivesc,
        summary: "Escala privilegios a root.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "linpeas",
        aliases: &[],
        category: Category::LocalPrivesc,
        summary: "Enumera escalada local (cubre todos los tipos).",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "suid",
        aliases: &[],
        category: Category::LocalPrivesc,
        summary: "Revela vectores SUID.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    CommandSpec {
        name: "sysinfo",
        aliases: &[],
        category: Category::LocalPrivesc,
        summary: "Revela vectores de kernel/SO.",
        usage: None,
        takes_args: false,
        kind: CommandKind::EngineBuiltin,
    },
    // `sudo -l` es built-in, pero `sudo` a secas queda libre para sabor de campaña.
    CommandSpec {
        name: "sudo",
        aliases: &[],
        category: Category::LocalPrivesc,
        summary: "Revela reglas sudo abusables (sudo -l).",
        usage: Some("sudo -l"),
        takes_args: true,
        kind: CommandKind::FlavorReserved,
    },
    // --------------------------------- Finales ---------------------------------
    CommandSpec {
        name: "choose",
        aliases: &["deliver"],
        category: Category::Endings,
        summary: "Elige un desenlace con elección.",
        usage: Some("choose <n>"),
        takes_args: true,
        kind: CommandKind::EngineBuiltin,
    },
    // -------------------------------- Minijuegos --------------------------------
    CommandSpec {
        name: "fortune",
        aliases: &[],
        category: Category::Minigame,
        summary: "Imprime un aforismo de campaña.",
        usage: None,
        takes_args: false,
        kind: CommandKind::Minigame,
    },
    CommandSpec {
        name: "signal",
        aliases: &[],
        category: Category::Minigame,
        summary: "Intercepta una señal cifrada.",
        usage: None,
        takes_args: false,
        kind: CommandKind::Minigame,
    },
    CommandSpec {
        name: "decode",
        aliases: &["decrypt"],
        category: Category::Minigame,
        summary: "Descifra la última señal interceptada.",
        usage: Some("decode <texto>"),
        takes_args: true,
        kind: CommandKind::Minigame,
    },
    CommandSpec {
        name: "crack",
        aliases: &[],
        category: Category::Minigame,
        summary: "Fuerza un teclado numérico.",
        usage: Some("crack <0000-9999>"),
        takes_args: true,
        kind: CommandKind::Minigame,
    },
    CommandSpec {
        name: "mastermind",
        aliases: &["bulls", "mm"],
        category: Category::Minigame,
        summary: "Minijuego mastermind (picos y toques).",
        usage: Some("mastermind [NNNN]"),
        takes_args: true,
        kind: CommandKind::Minigame,
    },
];

impl Category {
    /// ¿Categoría específica del dominio pentest (kill chain)? Las demás
    /// (General, VFS, Sistema, Finales, Minijuegos) son genéricas y valen a
    /// cualquier dominio.
    fn is_pentest(self) -> bool {
        matches!(
            self,
            Category::Recon
                | Category::Enum
                | Category::Findings
                | Category::MultiHost
                | Category::Offline
                | Category::LocalPrivesc
        )
    }

    /// Etiqueta de sección para la referencia de ayuda.
    fn label(self) -> &'static str {
        match self {
            Category::General => "GENERAL",
            Category::Recon => "RECON",
            Category::Enum => "ENUM",
            Category::Findings => "HALLAZGOS",
            Category::MultiHost => "MULTI-HOST",
            Category::Vfs => "VFS",
            Category::System => "SISTEMA",
            Category::Offline => "OFFLINE",
            Category::LocalPrivesc => "PRIVESC LOCAL",
            Category::Endings => "FINALES",
            Category::Minigame => "MINIJUEGOS",
        }
    }
}

/// Orden de presentación de las categorías en la referencia de ayuda.
const CATEGORY_ORDER: &[Category] = &[
    Category::General,
    Category::Recon,
    Category::Enum,
    Category::Findings,
    Category::MultiHost,
    Category::Vfs,
    Category::System,
    Category::Offline,
    Category::LocalPrivesc,
    Category::Endings,
    Category::Minigame,
];

/// Naturaleza del comando, en una etiqueta corta para la referencia.
fn kind_tag(kind: CommandKind) -> &'static str {
    match kind {
        CommandKind::EngineBuiltin => "motor",
        CommandKind::FrontendOnly => "frontend",
        CommandKind::Minigame => "minijuego",
        CommandKind::FlavorReserved => "reservado/sabor",
    }
}

/// Referencia compacta de comandos generada DESDE el registro (una línea por
/// comando, agrupada por categoría). La consume la ayuda (`help`), de modo que la
/// lista de built-ins, sus alias y su uso tienen una única fuente de verdad.
pub fn reference_lines(kill_chain: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for cat in CATEGORY_ORDER {
        // En dominios no-pentest, se omiten las categorías de la kill chain.
        if !kill_chain && cat.is_pentest() {
            continue;
        }
        let specs: Vec<&CommandSpec> = COMMANDS.iter().filter(|s| s.category == *cat).collect();
        // La categoría ENUM se cubre con las herramientas de toolbox (ver ayuda).
        if specs.is_empty() {
            continue;
        }
        lines.push(format!("[{}]", cat.label()));
        for s in specs {
            let head = s.usage.unwrap_or(s.name);
            let aliases = if s.aliases.is_empty() {
                String::new()
            } else {
                format!(" (alias: {})", s.aliases.join(", "))
            };
            lines.push(format!(
                "  {:<24} - {} [{}{}]{}",
                head,
                s.summary,
                kind_tag(s.kind),
                if s.takes_args { ", args" } else { "" },
                aliases
            ));
        }
    }
    lines
}

/// Todos los verbos completables (nombres + alias del catálogo + herramientas de
/// enumeración). Lo consume el autocompletado.
/// Verbos completables según el dominio: todos si `kill_chain`, o solo los
/// genéricos (sin verbos de la kill chain ni herramientas de enumeración) para
/// un dominio propio.
pub fn all_verbs_for(kill_chain: bool) -> Vec<&'static str> {
    let mut v = Vec::new();
    for spec in COMMANDS {
        if !kill_chain && spec.category.is_pentest() {
            continue;
        }
        v.push(spec.name);
        v.extend_from_slice(spec.aliases);
    }
    if kill_chain {
        for t in toolbox::TOOLS {
            v.push(t.name);
        }
    }
    v
}

/// Verbos "reservados" por la mecánica del juego que una campaña NO debería
/// reutilizar como easter egg / comando declarativo (quedarían ocultos). Excluye
/// los verbos marcados como reutilizables para sabor (p. ej. `sudo`). Se pasa al
/// validador del motor para detectar colisiones.
pub fn reserved_verbs() -> Vec<&'static str> {
    let mut v = Vec::new();
    for spec in COMMANDS {
        if spec.allows_flavor() {
            continue;
        }
        v.push(spec.name);
        v.extend_from_slice(spec.aliases);
    }
    for t in toolbox::TOOLS {
        v.push(t.name);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_excluye_flavor_pero_incluye_builtins() {
        let reserved = reserved_verbs();
        assert!(reserved.contains(&"nmap"));
        assert!(reserved.contains(&"quit"));
        assert!(reserved.contains(&"nikto")); // herramienta de enumeración
                                              // `sudo` es flavor-reserved: no debe estar en la lista de reservados.
        assert!(!reserved.contains(&"sudo"));
    }

    #[test]
    fn dominio_propio_excluye_la_kill_chain() {
        let generic = all_verbs_for(false);
        assert!(generic.contains(&"help")); // genérico
        assert!(generic.contains(&"ls")); // VFS genérico
        assert!(!generic.contains(&"nmap")); // pentest fuera
        assert!(!generic.contains(&"exploit"));
        assert!(!generic.contains(&"nikto")); // herramienta de enumeración fuera
    }

    #[test]
    fn all_verbs_incluye_alias_y_herramientas() {
        let verbs = all_verbs_for(true);
        assert!(verbs.contains(&"logros"));
        assert!(verbs.contains(&"achievements")); // alias
        assert!(verbs.contains(&"sqlmap")); // herramienta
        assert!(verbs.contains(&"sudo")); // completable aunque sea flavor
    }
}
