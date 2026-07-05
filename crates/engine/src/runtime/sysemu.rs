//! Emulación de comandos de sistema tipo POSIX (`uname`, `ps`, `netstat`, `env`…).
//!
//! Es mecánica **neutral** del motor, como `ls`/`cat`: sintetiza salida realista a
//! partir del estado que ya existe (`TargetNode`: hostname, ip, os, servicios; el
//! VFS; `is_root`; el reloj). No contiene historia. Así una campaña obtiene una
//! caja creíble sin autorar cada salida ni tocar Rust.
//!
//! La salida es POSIX auténtica (en inglés), independientemente del idioma
//! narrativo de la campaña: un operador sigue viendo `command not found` en inglés.

use std::collections::BTreeMap;

use crate::model::filesystem::{self, ReadOutcome};
use crate::runtime::state::GameState;

/// Resultado de un comando de shell: líneas a imprimir y código de salida (`$?`).
pub struct ShellOutput {
    pub lines: Vec<String>,
    pub exit: i32,
}

impl ShellOutput {
    pub(crate) fn ok(lines: Vec<String>) -> Self {
        ShellOutput { lines, exit: 0 }
    }

    pub(crate) fn code(lines: Vec<String>, exit: i32) -> Self {
        ShellOutput { lines, exit }
    }

    /// Salida de "orden no encontrada" (código 127), como bash.
    fn not_found(verb: &str) -> Self {
        ShellOutput::code(vec![format!("bash: {verb}: command not found")], 127)
    }
}

// ------------------------------- Identidad -------------------------------

fn username(state: &GameState) -> &'static str {
    if !state.has_foothold() {
        "operator"
    } else if state.pentest().is_root {
        "root"
    } else {
        "user"
    }
}

fn uid(state: &GameState) -> u32 {
    if state.has_foothold() && state.pentest().is_root {
        0
    } else {
        1000
    }
}

fn home(state: &GameState) -> String {
    if state.has_foothold() && state.pentest().is_root {
        String::from("/root")
    } else {
        format!("/home/{}", username(state))
    }
}

// ------------------------------- Entorno -------------------------------

/// Vista combinada del entorno: derivadas del motor < `campaign.env` < overrides
/// de sesión (`export`). Ordenada por clave para una salida determinista.
pub fn env_pairs(state: &GameState) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    // Derivadas.
    env.insert(String::from("USER"), username(state).to_string());
    env.insert(String::from("LOGNAME"), username(state).to_string());
    env.insert(String::from("HOME"), home(state));
    env.insert(String::from("PWD"), state.cwd_display());
    env.insert(String::from("SHELL"), String::from("/bin/bash"));
    if state.has_foothold() {
        env.insert(
            String::from("HOSTNAME"),
            state.pentest().target.short_name().to_string(),
        );
    }
    // Definidas por la campaña (pueden sobreescribir derivadas como SHELL/PATH).
    for (k, v) in &state.campaign.env {
        env.insert(k.clone(), v.clone());
    }
    // Overrides de sesión (`export`).
    for (k, v) in &state.core.env_session {
        env.insert(k.clone(), v.clone());
    }
    env
}

/// Valor de una variable de entorno (o `$?`), si existe.
pub fn env_value(state: &GameState, name: &str) -> Option<String> {
    if name == "?" {
        return Some(state.core.last_exit.to_string());
    }
    env_pairs(state).get(name).cloned()
}

/// Sustituye `$VAR`, `${VAR}` y `$?` en `input` con sus valores (vacío si la
/// variable no existe, como bash). Otros usos de `$` se dejan intactos.
pub fn expand_vars(state: &GameState, input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // Estamos en '$'.
        if i + 1 >= chars.len() {
            out.push('$');
            break;
        }
        let next = chars[i + 1];
        if next == '?' {
            out.push_str(&state.core.last_exit.to_string());
            i += 2;
        } else if next == '{' {
            // ${NOMBRE}
            if let Some(close) = chars[i + 2..].iter().position(|&c| c == '}') {
                let name: String = chars[i + 2..i + 2 + close].iter().collect();
                out.push_str(&env_value(state, &name).unwrap_or_default());
                i += 2 + close + 1;
            } else {
                out.push('$');
                i += 1;
            }
        } else if next == '_' || next.is_ascii_alphabetic() {
            // $NOMBRE (letras, dígitos y guion bajo).
            let mut j = i + 1;
            while j < chars.len() && (chars[j] == '_' || chars[j].is_ascii_alphanumeric()) {
                j += 1;
            }
            let name: String = chars[i + 1..j].iter().collect();
            out.push_str(&env_value(state, &name).unwrap_or_default());
            i = j;
        } else {
            // '$' seguido de algo no-variable: literal.
            out.push('$');
            i += 1;
        }
    }
    out
}

/// Renderiza una plantilla de salida: sustituye `{clock}`, `{user}`, `{host}`,
/// `{ip}`, `{os}`, `{cwd}`, `{env:NOMBRE}` y luego las variables `$VAR`/`$?`. Lo
/// usan los comandos de terminal autorados por la campaña.
pub fn render(state: &GameState, input: &str) -> String {
    let mut s = input.to_string();
    s = s.replace("{clock}", &state.core.clock.to_string());
    s = s.replace("{user}", username(state));
    s = s.replace("{host}", state.pentest().target.short_name());
    s = s.replace("{ip}", &state.pentest().target.ip);
    s = s.replace("{os}", &state.pentest().target.os);
    s = s.replace("{cwd}", &state.cwd_display());
    // {env:NOMBRE}
    while let Some(start) = s.find("{env:") {
        let Some(rel_end) = s[start..].find('}') else {
            break;
        };
        let end = start + rel_end;
        let name = &s[start + 5..end];
        let value = env_value(state, name).unwrap_or_default();
        s.replace_range(start..=end, &value);
    }
    expand_vars(state, &s)
}

// --------------------------- Comandos de sistema ---------------------------

/// Verbos que describen el host comprometido y solo tienen sentido con una shell.
fn needs_shell(verb: &str) -> bool {
    matches!(
        verb,
        "uname" | "hostname" | "ps" | "netstat" | "ss" | "ifconfig" | "ip" | "env" | "export"
    )
}

/// Nombre de proceso plausible para un servicio, por su nombre.
fn process_for_service(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "http" | "https" | "http-proxy" | "http-alt" => "nginx: worker process",
        "ssh" => "sshd",
        "smb" | "netbios" | "netbios-ssn" | "microsoft-ds" => "smbd",
        "mysql" => "mysqld",
        "pgsql" | "postgresql" => "postgres",
        "redis" => "redis-server",
        "mongodb" => "mongod",
        "mssql" => "sqlservr",
        "oracle" => "oracle",
        _ => "service",
    }
}

fn uname(state: &GameState, args: &[String]) -> ShellOutput {
    let host = state.pentest().target.short_name();
    let os = &state.pentest().target.os;
    let flags: String = args.iter().flat_map(|a| a.chars()).collect();
    let all = args.iter().any(|a| a == "-a" || a.contains('a'));
    let line = if all {
        format!("Linux {host} {os} #1 SMP x86_64 GNU/Linux")
    } else if flags.contains('n') {
        host.to_string()
    } else if flags.contains('r') {
        os.clone()
    } else if flags.contains('m') {
        String::from("x86_64")
    } else {
        String::from("Linux")
    };
    ShellOutput::ok(vec![line])
}

fn hostname(state: &GameState, args: &[String]) -> ShellOutput {
    let line = if args.iter().any(|a| a == "-f" || a == "--fqdn") {
        state.pentest().target.hostname.clone()
    } else {
        state.pentest().target.short_name().to_string()
    };
    ShellOutput::ok(vec![line])
}

fn id_cmd(state: &GameState) -> ShellOutput {
    let u = username(state);
    let n = uid(state);
    ShellOutput::ok(vec![format!("uid={n}({u}) gid={n}({u}) groups={n}({u})")])
}

fn whoami(state: &GameState) -> ShellOutput {
    ShellOutput::ok(vec![username(state).to_string()])
}

fn ps(state: &GameState) -> ShellOutput {
    let mut lines = vec![String::from("USER         PID  COMMAND")];
    let mut pid = 1u32;
    let row = |lines: &mut Vec<String>, user: &str, command: &str, pid: &mut u32| {
        lines.push(format!("{user:<10} {p:>5}  {command}", p = *pid));
        *pid += 1;
    };
    row(&mut lines, "root", "/sbin/init", &mut pid);
    row(&mut lines, "root", "/usr/sbin/sshd", &mut pid);
    for s in &state.pentest().target.services {
        let user = match s.name.to_lowercase().as_str() {
            "http" | "https" | "http-proxy" | "http-alt" => "www-data",
            "pgsql" | "postgresql" => "postgres",
            "mysql" => "mysql",
            "redis" => "redis",
            _ => "root",
        };
        let cmd = process_for_service(&s.name);
        row(&mut lines, user, cmd, &mut pid);
    }
    for extra in &state.campaign.processes {
        lines.push(extra.clone());
    }
    row(&mut lines, username(state), "-bash", &mut pid);
    ShellOutput::ok(lines)
}

fn netstat(state: &GameState) -> ShellOutput {
    let mut lines = vec![String::from(
        "Proto Recv-Q Send-Q Local Address           Foreign Address         State",
    )];
    for s in &state.pentest().target.services {
        lines.push(format!(
            "tcp        0      0 {:<23} {:<23} LISTEN",
            format!("0.0.0.0:{}", s.port),
            "0.0.0.0:*"
        ));
    }
    ShellOutput::ok(lines)
}

fn ifconfig(state: &GameState) -> ShellOutput {
    let ip = &state.pentest().target.ip;
    ShellOutput::ok(vec![
        String::from("eth0: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500"),
        format!("        inet {ip}  netmask 255.255.255.0  broadcast 0.0.0.0"),
        String::new(),
        String::from("lo: flags=73<UP,LOOPBACK,RUNNING>  mtu 65536"),
        String::from("        inet 127.0.0.1  netmask 255.0.0.0"),
    ])
}

fn ip_cmd(state: &GameState, args: &[String]) -> ShellOutput {
    // `ip a` / `ip addr`: variante compacta de ifconfig.
    if args.iter().any(|a| a.starts_with('a')) {
        let ip = &state.pentest().target.ip;
        ShellOutput::ok(vec![
            String::from("1: lo: <LOOPBACK,UP,LOWER_UP>"),
            String::from("    inet 127.0.0.1/8 scope host lo"),
            String::from("2: eth0: <BROADCAST,MULTICAST,UP,LOWER_UP>"),
            format!("    inet {ip}/24 scope global eth0"),
        ])
    } else {
        ShellOutput::code(
            vec![String::from(
                "Usage: ip [ OPTIONS ] OBJECT { COMMAND | help }",
            )],
            255,
        )
    }
}

fn env_cmd(state: &GameState) -> ShellOutput {
    let lines = env_pairs(state)
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    ShellOutput::ok(lines)
}

fn export_cmd(state: &mut GameState, args: &[String]) -> ShellOutput {
    if args.is_empty() {
        return env_cmd(state);
    }
    for a in args {
        if let Some((k, v)) = a.split_once('=') {
            if k.is_empty() {
                continue;
            }
            // El valor puede contener referencias a otras variables.
            let value = expand_vars(state, v);
            if let Some(slot) = state.core.env_session.iter_mut().find(|(name, _)| name == k) {
                slot.1 = value;
            } else {
                state.core.env_session.push((k.to_string(), value));
            }
        }
        // `export VAR` a secas (sin '=') no cambia nada visible aquí.
    }
    ShellOutput::ok(Vec::new())
}

// ------------------------- Utilidades de fichero -------------------------

/// Lee el contenido en claro de un fichero del VFS, respetando permisos de root.
/// Devuelve `Err(ShellOutput)` con el error POSIX apropiado si no procede.
pub(crate) fn read_lines(state: &GameState, arg: &str) -> Result<Vec<String>, ShellOutput> {
    let comps = filesystem::normalize(&state.core.cwd, arg);
    match filesystem::read_file(&state.pentest().target.filesystem, &comps) {
        ReadOutcome::NotFound => Err(ShellOutput::code(
            vec![format!("{arg}: No such file or directory")],
            1,
        )),
        ReadOutcome::IsDir => Err(ShellOutput::code(vec![format!("{arg}: Is a directory")], 1)),
        ReadOutcome::File {
            content,
            root,
            is_binary,
            ..
        } => {
            if root && !state.pentest().is_root {
                Err(ShellOutput::code(
                    vec![format!("{arg}: Permission denied")],
                    1,
                ))
            } else if is_binary {
                // Binario: contenido no imprimible (sin efecto de juego aquí).
                Ok(vec![String::from("<binary data>")])
            } else {
                Ok(content)
            }
        }
    }
}

fn file_cmd(state: &GameState, args: &[String]) -> ShellOutput {
    let Some(path) = args.first() else {
        return ShellOutput::code(vec![String::from("usage: file FILE")], 2);
    };
    let comps = filesystem::normalize(&state.core.cwd, path);
    match filesystem::read_file(&state.pentest().target.filesystem, &comps) {
        ReadOutcome::NotFound => ShellOutput::code(
            vec![format!("{path}: cannot open (No such file or directory)")],
            1,
        ),
        ReadOutcome::IsDir => ShellOutput::ok(vec![format!("{path}: directory")]),
        ReadOutcome::File { is_binary, .. } => {
            let kind = if is_binary {
                "ELF 64-bit LSB executable, x86-64"
            } else {
                "ASCII text"
            };
            ShellOutput::ok(vec![format!("{path}: {kind}")])
        }
    }
}

/// Ejecuta un comando de sistema `verb` con sus `args`. Devuelve `None` si `verb`
/// no es un comando de sistema emulado (el frontend probará otras vías).
pub fn run(state: &mut GameState, verb: &str, args: &[String]) -> Option<ShellOutput> {
    // Los comandos que describen el host requieren una shell en él.
    // En dominios con VFS libre (Bare/laboratorios no-pentest) la consola ya es
    // el entorno operativo, así que estos comandos deben estar disponibles sin
    // la mecánica de foothold.
    if needs_shell(verb) && state.campaign.shell_for_vfs() && !state.has_foothold() {
        return Some(ShellOutput::not_found(verb));
    }
    let out = match verb {
        "uname" => uname(state, args),
        "hostname" => hostname(state, args),
        "id" => id_cmd(state),
        "whoami" => whoami(state),
        "ps" => ps(state),
        "netstat" | "ss" => netstat(state),
        "ifconfig" => ifconfig(state),
        "ip" => ip_cmd(state, args),
        "env" => env_cmd(state),
        "export" => export_cmd(state, args),
        // Filtros de texto (grep, head, tail, wc, sort, uniq, nl, cat…): los
        // implementa `shell`, que además soporta stdin en las tuberías. Aquí se
        // invocan sin stdin (comando suelto que lee de un fichero-argumento).
        v if crate::runtime::shell::is_filter(v) => {
            match crate::runtime::shell::filter(state, v, args, None) {
                Some(o) => o,
                None => return None,
            }
        }
        "file" => file_cmd(state, args),
        _ => return None,
    };
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::campaign::Campaign;
    use crate::model::language::Language;
    use crate::model::mission::{EntryVector, Mission};
    use crate::model::target::{ExploitReliability, Service, TargetNode, Vulnerability};
    use crate::model::theme::Theme;
    use crate::runtime::state::{GameState, Phase};
    use std::collections::BTreeMap;

    fn host() -> TargetNode {
        TargetNode {
            hostname: String::from("web-01.lab.local"),
            ip: String::from("10.0.0.5"),
            os: String::from("Linux 5.15.0"),
            services: vec![
                Service {
                    port: 80,
                    name: String::from("http"),
                    version: String::from("nginx"),
                    requires: None,
                },
                Service {
                    port: 22,
                    name: String::from("ssh"),
                    version: String::from("OpenSSH"),
                    requires: None,
                },
            ],
            vulnerabilities: vec![Vulnerability {
                id: String::from("V"),
                name: String::from("x"),
                affected_service: 80,
                difficulty: 4,
                stealth_cost: 5,
                reliability: ExploitReliability::Reliable,
            }],
            filesystem: vec![],
            accepts_token: None,
            local_privesc: None,
        }
    }

    fn state() -> GameState {
        let mut env = BTreeMap::new();
        env.insert(String::from("PATH"), String::from("/usr/bin:/bin"));
        env.insert(String::from("APP_SECRET"), String::from("s3cr3t"));
        let campaign = Campaign {
            name: String::from("T"),
            language: Language::En,
            intro: vec![],
            stages: crate::model::campaign::default_stages(),
            domain: None,
            features: Default::default(),
            theme: Theme::default(),
            easter_eggs: vec![],
            fortunes: vec![],
            signals: vec![],
            achievements: vec![],
            commands: vec![],
            env,
            processes: vec![],
            terminal: vec![],
            missions: vec![Mission {
                id: String::from("m0"),
                name: String::from("M0"),
                briefing: vec![],
                detection_limit: 100.0,
                meters: vec![],
                time_limit: None,
                reactive: false,
                skill: 0.5,
                root_difficulty: 4,
                objective: None,
                debrief: vec![],
                entry: EntryVector::Active,
                endings: vec![],
                target: host(),
                network: vec![],
                music: None,
                autoplay: vec![],
            }],
        };
        GameState::new(campaign)
    }

    #[test]
    fn uname_y_netstat_sintetizan_del_host() {
        let mut g = state();
        g.reach_phase(Phase::Post); // simula foothold
        let u = run(&mut g, "uname", &[String::from("-a")]).unwrap();
        assert!(u.lines[0].contains("web-01"));
        assert!(u.lines[0].contains("Linux 5.15.0"));

        let n = run(&mut g, "netstat", &[]).unwrap();
        assert!(n.lines.iter().any(|l| l.contains("0.0.0.0:80")));
        assert!(n.lines.iter().any(|l| l.contains("0.0.0.0:22")));
    }

    #[test]
    fn expand_vars_resuelve_derivadas_campaign_y_exit() {
        let mut g = state();
        g.reach_phase(Phase::Post);
        g.pentest_mut().is_root = true;
        g.core.last_exit = 3;
        assert_eq!(expand_vars(&g, "$USER"), "root");
        assert_eq!(expand_vars(&g, "${APP_SECRET}"), "s3cr3t");
        assert_eq!(expand_vars(&g, "code=$?"), "code=3");
        // Variable inexistente -> vacío (como bash).
        assert_eq!(expand_vars(&g, "x=$NOPE."), "x=.");
    }

    #[test]
    fn export_overridea_y_env_lo_muestra() {
        let mut g = state();
        g.reach_phase(Phase::Post);
        run(&mut g, "export", &[String::from("FOO=bar")]);
        assert_eq!(env_value(&g, "FOO").as_deref(), Some("bar"));
        let e = run(&mut g, "env", &[]).unwrap();
        assert!(e.lines.iter().any(|l| l == "FOO=bar"));
    }

    #[test]
    fn sistema_sin_shell_es_command_not_found() {
        let mut g = state(); // sin foothold
        let out = run(&mut g, "netstat", &[]).unwrap();
        assert_eq!(out.exit, 127);
        assert!(out.lines[0].contains("command not found"));
    }

    #[test]
    fn sistema_funciona_en_vfs_libre_sin_foothold() {
        let mut g = state();
        g.campaign.features.shell_for_vfs = Some(false);

        let out = run(&mut g, "env", &[]).unwrap();
        assert_eq!(out.exit, 0);
        assert!(out.lines.iter().any(|l| l == "APP_SECRET=s3cr3t"));
    }
}
