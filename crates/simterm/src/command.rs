//! Parseo de la línea de comandos introducida por el jugador.
//!
//! Los comandos "builtin" (mecánica del juego) viven aquí. Los easter eggs de
//! puro sabor NO: los define la campaña (`theme`/`easter_eggs`) y se resuelven
//! por nombre en el dispatcher (ver `App::cmd_easter`). Así el parser no conoce
//! ninguna historia concreta.

use simterm_engine::toolbox;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Ayuda. `all` = referencia completa; si es `false`, solo la fase actual.
    Help {
        all: bool,
    },
    Target,
    /// Reconocimiento activo del host (nmap).
    Recon,
    /// Reconocimiento pasivo: interceptación de tráfico (sniff).
    Sniff,
    /// Pivote a través de un bastión de entrada: (host opcional).
    Connect(Option<String>),
    /// Descubrimiento de hosts internos desde un host comprometido (netmap).
    Netmap,
    /// Pivote entre hosts de la red interna: pivot <host>.
    Pivot(Option<String>),
    /// Herramienta de enumeración sobre un puerto: (nombre, puerto).
    Enumerate(String, Option<u16>),
    /// Investigación de un hallazgo (searchsploit / verify).
    Research(usize),
    Intel,
    Exploit(usize),
    /// Foothold determinista con credencial reutilizada.
    Login,
    Privesc,
    Loot,
    /// Cracking offline de hashes saqueados.
    John(Option<String>),
    /// Reversing: cadenas imprimibles de un binario.
    Strings(Option<String>),
    /// Reversing: pseudo-desensamblado de un binario.
    Disasm(Option<String>),
    /// Reversing: entrega de secreto extraído de un binario.
    Solve(Option<String>, Option<String>),
    /// Decodificación de ficheros codificados en el VFS.
    DecodeFile {
        tool: String,
        path: Option<String>,
        key: Option<String>,
    },
    /// Enumeración local de privesc en POST.
    LocalEnum(String),
    // --- VFS (fase POST) ---
    Ls(Option<String>),
    Cat(Option<String>),
    Exfil(Option<String>),
    Cd(Option<String>),
    Pwd,
    Find(Option<String>),
    /// Encubrimiento activo: baja la traza con coste y riesgo.
    Cleanup,
    /// Reinicia la campaña (borra el guardado).
    Reset,
    /// Elige un desenlace en el final con elección (número 1..=N).
    Choose(Option<usize>),
    Status,
    Logs,
    Achievements,
    Clear,
    Quit,
    /// Comando de sistema emulado (uname, ps, netstat, env, grep...). El motor lo
    /// interpreta a partir del estado del host; `args` son los tokens tras el verbo.
    Shell {
        verb: String,
        args: Vec<String>,
    },
    /// Puerto no numérico en un comando de enumeración.
    BadPort(String),
    /// Id no numérico en research/exploit.
    BadId(String),
    /// Verbo no reconocido: el dispatcher comprueba si es un comando declarativo,
    /// un comando de terminal autorado o un easter egg antes de declararlo
    /// desconocido (`command not found`). `args` permite resolver comandos con
    /// argumentos (p. ej. `systemctl status nginx`).
    Unknown {
        verb: String,
        args: Vec<String>,
    },
    Empty,

    // --- Minijuegos (mecánica genérica del motor; el CONTENIDO es de campaña) ---
    /// `echo`: devuelve el texto introducido.
    Echo(String),
    /// Aforismo aleatorio (de `campaign.fortunes`).
    Fortune,
    /// Interceptar una señal cifrada (palabras de `campaign.signals`).
    Signal,
    /// Descifrar la señal interceptada.
    Decode(String),
    /// Forzar un teclado numérico (0000..9999).
    Crack(Option<u16>),
    /// Historial de comandos introducidos.
    History,
    /// Mastermind / picos y toques (sin arg = inicia; con arg = tirada).
    Mastermind(Option<String>),
}

impl Command {
    /// ¿Es un verbo de la kill chain (pentesting)? En dominios propios (no
    /// pentest) estos verbos no existen: el parser los degrada a `Unknown`.
    pub fn is_pentest(&self) -> bool {
        matches!(
            self,
            Command::Target
                | Command::Recon
                | Command::Sniff
                | Command::Connect(_)
                | Command::Netmap
                | Command::Pivot(_)
                | Command::Enumerate(..)
                | Command::Research(_)
                | Command::Intel
                | Command::Exploit(_)
                | Command::Login
                | Command::Privesc
                | Command::Loot
                | Command::John(_)
                | Command::Strings(_)
                | Command::Disasm(_)
                | Command::Solve(..)
                | Command::DecodeFile { .. }
                | Command::LocalEnum(_)
                | Command::Cleanup
                | Command::BadPort(_)
                | Command::BadId(_)
        )
    }
}

/// Parsea la línea. `pentest` indica si el dominio activo usa la kill chain; si
/// no, los verbos pentest se degradan a `Unknown` (caen a comando de campaña /
/// easter egg / "command not found").
pub fn parse(input: &str, pentest: bool) -> Command {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Command::Empty;
    }

    let mut parts = trimmed.split_whitespace();
    let verb = parts.next().unwrap_or("").to_lowercase();
    let arg = parts.next();
    // Todos los tokens tras el verbo (para los comandos de sistema/terminal).
    let all_args: Vec<String> = trimmed
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect();
    // Copias reservadas para degradar un verbo pentest a `Unknown` (ver el gate
    // al final): el `match` consume `verb`/`all_args` en algunas ramas.
    let gated_verb = verb.clone();
    let gated_args = all_args.clone();
    // Todo lo que sigue al primer token (para echo / decode multipalabra).
    let rest = trimmed
        .splitn(2, char::is_whitespace)
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_string();

    let cmd = match verb.as_str() {
        "help" | "h" | "?" => Command::Help {
            all: matches!(arg, Some("all") | Some("todo") | Some("full") | Some("-a")),
        },
        "target" | "host" => Command::Target,
        "nmap" | "scan" | "recon" => Command::Recon,
        "sniff" | "intercept" | "listen" => Command::Sniff,
        "connect" => Command::Connect(arg.map(str::to_string)),
        "netmap" | "lan" | "neighbors" => Command::Netmap,
        "pivot" | "jump" => Command::Pivot(arg.map(str::to_string)),
        "searchsploit" | "verify" | "research" => parse_id(arg, Command::Research),
        "exploit" | "run" => parse_id(arg, Command::Exploit),
        "login" | "ssh" => Command::Login,
        "privesc" | "escalate" | "root" => Command::Privesc,
        "loot" | "creds" => Command::Loot,
        "john" | "hashcat" => Command::John(arg.map(str::to_string)),
        "strings" => Command::Strings(arg.map(str::to_string)),
        "disasm" | "objdump" | "r2" => Command::Disasm(arg.map(str::to_string)),
        "solve" => {
            let mut args = rest.splitn(2, char::is_whitespace);
            Command::Solve(
                args.next().filter(|s| !s.is_empty()).map(str::to_string),
                args.next()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            )
        }
        "base64" => Command::DecodeFile {
            tool: String::from("base64"),
            path: arg.map(str::to_string),
            key: None,
        },
        "xor" => {
            let mut args = rest.splitn(2, char::is_whitespace);
            Command::DecodeFile {
                tool: String::from("xor"),
                path: args.next().filter(|s| !s.is_empty()).map(str::to_string),
                key: args
                    .next()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            }
        }
        "linpeas" | "suid" | "sysinfo" => Command::LocalEnum(verb),
        "sudo" if arg == Some("-l") => Command::LocalEnum(String::from("sudo")),
        "ls" | "dir" => Command::Ls(arg.map(str::to_string)),
        "cat" | "read" | "type" => Command::Cat(arg.map(str::to_string)),
        "exfil" => Command::Exfil(arg.map(str::to_string)),
        "cd" => Command::Cd(arg.map(str::to_string)),
        "pwd" => Command::Pwd,
        "find" => Command::Find(arg.map(str::to_string)),
        "cleanup" | "covertracks" | "cleanlogs" => Command::Cleanup,
        "reset" | "newgame" => Command::Reset,
        "choose" | "deliver" => Command::Choose(arg.and_then(|s| s.parse::<usize>().ok())),
        "intel" => Command::Intel,
        "status" => Command::Status,
        "logs" => Command::Logs,
        "achievements" | "achievement" | "logros" | "logro" => Command::Achievements,
        "clear" | "cls" => Command::Clear,
        "quit" | "exit" | "q" => Command::Quit,

        // --- Minijuegos (mecánica del motor) ---
        "echo" => Command::Echo(rest),
        "fortune" => Command::Fortune,
        "signal" => Command::Signal,
        "decode" | "decrypt" => Command::Decode(rest),
        "crack" => Command::Crack(arg.and_then(|s| s.parse::<u16>().ok())),
        "mastermind" | "bulls" | "mm" => Command::Mastermind(arg.map(str::to_string)),
        "history" => Command::History,

        other => {
            if toolbox::tool_by_name(other).is_some() {
                parse_enum(other, arg)
            } else if is_system_verb(other) {
                // Comando de sistema emulado (uname, ps, netstat, grep...).
                Command::Shell {
                    verb: other.to_string(),
                    args: all_args,
                }
            } else {
                // Puede ser un comando declarativo/terminal o un easter egg; lo
                // decide el dispatcher. Si nada casa: command not found.
                Command::Unknown {
                    verb: other.to_string(),
                    args: all_args,
                }
            }
        }
    };

    // Gate de dominio: en una campaña no-pentest, los verbos de la kill chain se
    // degradan a `Unknown` (el dispatcher probará comandos de campaña / easter
    // eggs y, si nada casa, dirá "command not found").
    if !pentest && cmd.is_pentest() {
        return Command::Unknown {
            verb: gated_verb,
            args: gated_args,
        };
    }
    cmd
}

/// Verbos de sistema emulados por el motor (`runtime::sysemu`). El parser los
/// enruta a `Command::Shell`; el motor sintetiza su salida desde el estado.
fn is_system_verb(verb: &str) -> bool {
    matches!(
        verb,
        "uname"
            | "hostname"
            | "id"
            | "whoami"
            | "ps"
            | "netstat"
            | "ss"
            | "ifconfig"
            | "ip"
            | "env"
            | "export"
            | "grep"
            | "head"
            | "tail"
            | "wc"
            | "file"
    )
}

fn parse_enum(tool: &str, arg: Option<&str>) -> Command {
    match arg {
        None => Command::Enumerate(tool.to_string(), None),
        Some(raw) => match raw.parse::<u16>() {
            Ok(port) => Command::Enumerate(tool.to_string(), Some(port)),
            Err(_) => Command::BadPort(raw.to_string()),
        },
    }
}

fn parse_id(arg: Option<&str>, f: impl Fn(usize) -> Command) -> Command {
    match arg {
        Some(raw) => match raw.parse::<usize>() {
            Ok(id) => f(id),
            Err(_) => Command::BadId(raw.to_string()),
        },
        None => Command::BadId(String::from("(vacío)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbos_pentest_se_degradan_en_dominio_propio() {
        // En dominio pentest, la kill chain existe.
        assert_eq!(parse("nmap", true), Command::Recon);
        assert!(matches!(parse("exploit 1", true), Command::Exploit(1)));

        // En un dominio propio, se degradan a Unknown ("command not found").
        match parse("nmap", false) {
            Command::Unknown { verb, .. } => assert_eq!(verb, "nmap"),
            other => panic!("esperaba Unknown, fue {other:?}"),
        }
        assert!(matches!(parse("exploit 1", false), Command::Unknown { .. }));
        // Una herramienta de enumeración (toolbox) también.
        assert!(matches!(parse("nikto 80", false), Command::Unknown { .. }));

        // Los verbos genéricos NO se degradan nunca.
        assert!(matches!(parse("help", false), Command::Help { .. }));
        assert!(matches!(parse("ls /x", false), Command::Ls(_)));
        assert!(matches!(parse("echo hola", false), Command::Echo(_)));
    }
}
