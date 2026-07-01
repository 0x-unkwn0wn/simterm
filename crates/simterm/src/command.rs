//! Parseo de la línea de comandos introducida por el jugador.
//!
//! Los comandos "builtin" (mecánica del juego) viven aquí. Los easter eggs de
//! puro sabor NO: los define la campaña (`theme`/`easter_eggs`) y se resuelven
//! por nombre en el dispatcher (ver `App::cmd_easter`). Así el parser no conoce
//! ninguna historia concreta.

use simterm_engine::toolbox;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help,
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
    Cd(Option<String>),
    Pwd,
    Find(Option<String>),
    /// Identidad de la sesión actual.
    Whoami,
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
    /// Puerto no numérico en un comando de enumeración.
    BadPort(String),
    /// Id no numérico en research/exploit.
    BadId(String),
    /// Verbo no reconocido: el dispatcher comprueba si es un easter egg de la
    /// campaña antes de declararlo desconocido.
    Unknown(String),
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

pub fn parse(input: &str) -> Command {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Command::Empty;
    }

    let mut parts = trimmed.split_whitespace();
    let verb = parts.next().unwrap_or("").to_lowercase();
    let arg = parts.next();
    // Todo lo que sigue al primer token (para echo / decode multipalabra).
    let rest = trimmed
        .splitn(2, char::is_whitespace)
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_string();

    match verb.as_str() {
        "help" | "h" | "?" => Command::Help,
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
        "cd" => Command::Cd(arg.map(str::to_string)),
        "pwd" => Command::Pwd,
        "find" => Command::Find(arg.map(str::to_string)),
        "whoami" | "id" => Command::Whoami,
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
            } else {
                // Puede ser un easter egg de la campaña; lo decide el dispatcher.
                Command::Unknown(other.to_string())
            }
        }
    }
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
