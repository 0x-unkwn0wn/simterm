//! Tuberías (`|`) y redirecciones de salida (`>`, `>>`) para la shell emulada.
//!
//! El resto del motor trata cada comando como "verbo + argumentos" que imprime su
//! salida directamente en el log. Este módulo añade la composición típica de una
//! shell real: encadenar filtros por una tubería y volcar el resultado a un
//! fichero del VFS. Es mecánica **neutral** (sirve a cualquier dominio con VFS):
//! un laboratorio de Bash puede así enseñar `grep ... | wc -l` o
//! `grep ERROR app.log > errores.txt` ejecutándolos DE VERDAD, no solo
//! contándolos.
//!
//! Los **filtros** ([`filter`]) son funciones puras sobre líneas que leen de un
//! fichero-argumento o, si no lo hay, de la entrada estándar (`stdin`) que les
//! pasa la etapa anterior de la tubería. `cat` es además la fuente típica y
//! registra la lectura para la verificación de trabajo real (ver
//! [`GameState::record_read`]).

use crate::model::filesystem::{self, WriteOutcome};
use crate::runtime::state::GameState;
use crate::runtime::sysemu::{self, ShellOutput};

/// Verbos que funcionan como **filtro** (leen de fichero o de stdin). Es el
/// conjunto que puede aparecer en una etapa de tubería.
const FILTERS: &[&str] = &[
    "cat", "grep", "head", "tail", "wc", "sort", "uniq", "nl", "echo", "ls", "find",
];

/// ¿Es `verb` un filtro de texto que sabe leer de una tubería?
pub fn is_filter(verb: &str) -> bool {
    FILTERS.contains(&verb)
}

/// ¿Contiene la línea una tubería (`|`) o una redirección de salida (`>`/`>>`)?
/// El frontend la usa para decidir si enrutar por [`run_pipeline`].
pub fn is_pipeline(line: &str) -> bool {
    line.contains('|') || line.contains('>')
}

/// Aplica un filtro `verb` con sus `args` y un `stdin` opcional (la salida de la
/// etapa anterior). Devuelve `None` si `verb` no es un filtro conocido.
pub fn filter(
    state: &mut GameState,
    verb: &str,
    args: &[String],
    stdin: Option<&[String]>,
) -> Option<ShellOutput> {
    let out = match verb {
        "cat" => cat(state, args, stdin),
        "grep" => grep(state, args, stdin),
        "head" | "tail" => head_tail(state, verb, args, stdin),
        "wc" => wc(state, args, stdin),
        "sort" => sort(state, args, stdin),
        "uniq" => uniq(state, args, stdin),
        "nl" => nl(state, args, stdin),
        "echo" => echo(state, args),
        "ls" => ls(state, args),
        "find" => find(state, args),
        _ => return None,
    };
    Some(out)
}

/// Resultado de ejecutar una línea con tuberías/redirecciones.
pub struct PipelineResult {
    /// Líneas a imprimir en el log. Vacías si la salida se redirigió a fichero.
    pub lines: Vec<String>,
    /// Código de salida de la última etapa (`$?`).
    pub exit: i32,
}

/// Ejecuta una línea que contiene tubería y/o redirección de salida.
///
/// - Separa una eventual redirección de la cola (`> f` o `>> f`).
/// - Divide el resto en etapas por `|` y las encadena: la salida de una es el
///   `stdin` de la siguiente.
/// - Si hay redirección, escribe la salida final en el fichero (creándolo si su
///   directorio existe); si no, la devuelve para imprimirla.
pub fn run_pipeline(state: &mut GameState, line: &str) -> PipelineResult {
    // En dominios con VFS tras foothold (pentest), la tubería necesita shell.
    if state.campaign.shell_for_vfs() && !state.has_foothold() {
        return PipelineResult {
            lines: vec![String::from(
                "bash: no hay shell en el objetivo todavía (consigue un foothold).",
            )],
            exit: 127,
        };
    }

    let (body, redirect) = split_redirect(line);
    let stages: Vec<&str> = body.split('|').map(str::trim).collect();

    let mut stdin: Option<Vec<String>> = None;
    let mut exit = 0;
    for stage in &stages {
        if stage.is_empty() {
            return PipelineResult {
                lines: vec![String::from("bash: error de sintaxis cerca de '|'")],
                exit: 2,
            };
        }
        let mut toks = stage.split_whitespace();
        let verb = toks.next().unwrap_or("").to_lowercase();
        let args: Vec<String> = toks.map(str::to_string).collect();

        let out = match filter(state, &verb, &args, stdin.as_deref()) {
            Some(o) => o,
            None => {
                return PipelineResult {
                    lines: vec![format!("bash: {verb}: no se puede usar en una tubería")],
                    exit: 127,
                }
            }
        };
        exit = out.exit;
        // Error de uso/fichero (código >= 2): aborta la tubería y muéstralo,
        // como haría una shell real al fallar una etapa.
        if out.exit >= 2 {
            return PipelineResult {
                lines: out.lines,
                exit: out.exit,
            };
        }
        stdin = Some(out.lines);
    }

    let final_lines = stdin.unwrap_or_default();
    match redirect {
        Some((path, append)) => write_redirect(state, &path, &final_lines, append, exit),
        None => PipelineResult {
            lines: final_lines,
            exit,
        },
    }
}

/// Separa la redirección de salida de la cola de la línea. Devuelve el cuerpo
/// (los comandos y tuberías) y, si la había, `(ruta, append)` donde `append` es
/// `true` para `>>`. Si tras el operador no hay destino, no se considera
/// redirección (se devuelve la línea intacta).
fn split_redirect(line: &str) -> (String, Option<(String, bool)>) {
    let parse = |pos: usize, op_len: usize, append: bool| -> (String, Option<(String, bool)>) {
        let body = line[..pos].to_string();
        let target = line[pos + op_len..]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        if target.is_empty() {
            (line.to_string(), None)
        } else {
            (body, Some((target, append)))
        }
    };
    if let Some(pos) = line.find(">>") {
        return parse(pos, 2, true);
    }
    if let Some(pos) = line.find('>') {
        return parse(pos, 1, false);
    }
    (line.to_string(), None)
}

/// Escribe la salida final de una tubería en un fichero del VFS.
fn write_redirect(
    state: &mut GameState,
    path: &str,
    lines: &[String],
    append: bool,
    exit: i32,
) -> PipelineResult {
    let comps = filesystem::normalize(&state.core.cwd, path);
    let outcome = filesystem::write_file(
        &mut state.pentest_mut().target.filesystem,
        &comps,
        lines,
        append,
    );
    let err = |msg: String| PipelineResult {
        lines: vec![msg],
        exit: 1,
    };
    match outcome {
        WriteOutcome::Ok => {
            state.advance_clock(1);
            // Sin salida en el log: el resultado quedó en el fichero. `$?` conserva
            // el código de la última etapa de la tubería.
            PipelineResult {
                lines: Vec::new(),
                exit,
            }
        }
        WriteOutcome::NoParentDir => err(format!("bash: {path}: No such file or directory")),
        WriteOutcome::IsDir => err(format!("bash: {path}: Is a directory")),
        WriteOutcome::Protected => err(format!("bash: {path}: Permission denied")),
    }
}

// ------------------------------- Filtros -------------------------------

/// Origen de líneas de un filtro: un fichero-argumento si lo hay, o el `stdin`
/// de la tubería. Prefija los errores de fichero con el nombre del verbo.
fn input(
    state: &GameState,
    file: Option<&str>,
    stdin: Option<&[String]>,
    verb: &str,
) -> Result<Vec<String>, ShellOutput> {
    match file {
        Some(f) => sysemu::read_lines(state, f).map_err(|mut e| {
            e.lines = e.lines.iter().map(|l| format!("{verb}: {l}")).collect();
            e.exit = 2;
            e
        }),
        None => Ok(stdin.map(<[String]>::to_vec).unwrap_or_default()),
    }
}

/// Primer argumento posicional (no-flag), si lo hay.
fn first_positional(args: &[String]) -> Option<&str> {
    args.iter().find(|a| !a.starts_with('-')).map(String::as_str)
}

/// `cat`: concatena ficheros (o pasa el stdin si no hay ninguno). Registra la
/// lectura de cada fichero para la verificación de trabajo real (`FileRead`).
fn cat(state: &mut GameState, args: &[String], stdin: Option<&[String]>) -> ShellOutput {
    let files: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .collect();
    if files.is_empty() {
        return ShellOutput::ok(stdin.map(<[String]>::to_vec).unwrap_or_default());
    }
    let mut out = Vec::new();
    for f in &files {
        match sysemu::read_lines(state, f) {
            Ok(lines) => {
                let comps = filesystem::normalize(&state.core.cwd, f);
                let disp = filesystem::path_string(&comps);
                state.unlock_campaign_read_file(&disp);
                out.extend(lines);
            }
            Err(mut e) => {
                e.lines = e.lines.iter().map(|l| format!("cat: {l}")).collect();
                e.exit = 1;
                return e;
            }
        }
    }
    ShellOutput::ok(out)
}

/// `grep PATRÓN [fichero]`: filtra por subcadena. Sin fichero, filtra el stdin.
fn grep(state: &mut GameState, args: &[String], stdin: Option<&[String]>) -> ShellOutput {
    let positionals: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    let Some(pattern) = positionals.first() else {
        return ShellOutput::code(vec![String::from("usage: grep PATTERN [FILE]")], 2);
    };
    let file = positionals.get(1).map(|s| s.as_str());
    let lines = match input(state, file, stdin, "grep") {
        Ok(l) => l,
        Err(e) => return e,
    };
    let invert = args.iter().any(|a| a == "-v");
    let hits: Vec<String> = lines
        .into_iter()
        .filter(|l| l.contains(pattern.as_str()) != invert)
        .collect();
    let exit = if hits.is_empty() { 1 } else { 0 };
    ShellOutput::code(hits, exit)
}

/// `head`/`tail [-n N] [fichero]`: primeras/últimas `N` líneas (por defecto 10).
fn head_tail(
    state: &mut GameState,
    verb: &str,
    args: &[String],
    stdin: Option<&[String]>,
) -> ShellOutput {
    let mut n = 10usize;
    let mut file: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "-n" {
            if let Some(v) = it.next() {
                n = v.parse().unwrap_or(10);
            }
        } else if let Some(v) = a.strip_prefix("-n") {
            n = v.parse().unwrap_or(10);
        } else if !a.starts_with('-') {
            file = Some(a.clone());
        }
    }
    let lines = match input(state, file.as_deref(), stdin, verb) {
        Ok(l) => l,
        Err(e) => return e,
    };
    let out: Vec<String> = if verb == "head" {
        lines.into_iter().take(n).collect()
    } else {
        let len = lines.len();
        lines.into_iter().skip(len.saturating_sub(n)).collect()
    };
    ShellOutput::ok(out)
}

/// `wc [-l|-w|-c] [fichero]`: cuenta líneas/palabras/bytes. Sin flags, las tres.
fn wc(state: &mut GameState, args: &[String], stdin: Option<&[String]>) -> ShellOutput {
    let file = first_positional(args);
    let lines = match input(state, file, stdin, "wc") {
        Ok(l) => l,
        Err(e) => return e,
    };
    let l = lines.len();
    let w: usize = lines.iter().map(|s| s.split_whitespace().count()).sum();
    let c: usize = lines.iter().map(|s| s.len() + 1).sum();

    let flags: String = args
        .iter()
        .filter(|a| a.starts_with('-'))
        .flat_map(|a| a.chars().skip(1))
        .collect();
    let mut parts: Vec<String> = Vec::new();
    if flags.is_empty() {
        parts.push(format!("{l:>7}"));
        parts.push(format!("{w:>7}"));
        parts.push(format!("{c:>7}"));
    } else {
        if flags.contains('l') {
            parts.push(format!("{l:>7}"));
        }
        if flags.contains('w') {
            parts.push(format!("{w:>7}"));
        }
        if flags.contains('c') {
            parts.push(format!("{c:>7}"));
        }
    }
    // Con fichero se muestra su nombre (como wc real); desde una tubería, no.
    let mut line = parts.join(" ");
    if let Some(f) = file {
        line.push(' ');
        line.push_str(f);
    }
    ShellOutput::ok(vec![line])
}

/// `sort [-r] [-u] [fichero]`: ordena líneas. `-r` inverso, `-u` únicas.
fn sort(state: &mut GameState, args: &[String], stdin: Option<&[String]>) -> ShellOutput {
    let file = first_positional(args);
    let mut lines = match input(state, file, stdin, "sort") {
        Ok(l) => l,
        Err(e) => return e,
    };
    lines.sort();
    if args.iter().any(|a| a == "-u") {
        lines.dedup();
    }
    if args.iter().any(|a| a == "-r") {
        lines.reverse();
    }
    ShellOutput::ok(lines)
}

/// `uniq [-c] [fichero]`: colapsa líneas repetidas ADYACENTES. `-c` antepone su
/// número de repeticiones (combínalo con `sort` para contar de verdad).
fn uniq(state: &mut GameState, args: &[String], stdin: Option<&[String]>) -> ShellOutput {
    let file = first_positional(args);
    let lines = match input(state, file, stdin, "uniq") {
        Ok(l) => l,
        Err(e) => return e,
    };
    let count = args.iter().any(|a| a == "-c");
    let mut groups: Vec<(String, usize)> = Vec::new();
    for line in lines {
        match groups.last_mut() {
            Some((prev, n)) if *prev == line => *n += 1,
            _ => groups.push((line, 1)),
        }
    }
    let out: Vec<String> = groups
        .into_iter()
        .map(|(line, n)| {
            if count {
                format!("{n:>7} {line}")
            } else {
                line
            }
        })
        .collect();
    ShellOutput::ok(out)
}

/// `nl [fichero]`: numera las líneas (1-indexado).
fn nl(state: &mut GameState, args: &[String], stdin: Option<&[String]>) -> ShellOutput {
    let file = first_positional(args);
    let lines = match input(state, file, stdin, "nl") {
        Ok(l) => l,
        Err(e) => return e,
    };
    let out: Vec<String> = lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| format!("{:>6}\t{}", i + 1, l))
        .collect();
    ShellOutput::ok(out)
}

/// `echo`: imprime sus argumentos (con expansión de `$VAR`). Ignora el stdin.
fn echo(state: &GameState, args: &[String]) -> ShellOutput {
    let joined = args.join(" ");
    ShellOutput::ok(vec![sysemu::expand_vars(state, &joined)])
}

/// `ls [ruta]` como **fuente** de tubería: emite las entradas del directorio,
/// una por línea, sin la decoración del `ls` suelto (para poder contarlas/filtrarlas).
fn ls(state: &GameState, args: &[String]) -> ShellOutput {
    let target = first_positional(args).unwrap_or("");
    let comps = filesystem::normalize(&state.core.cwd, target);
    match filesystem::list_entries(&state.pentest().target.filesystem, &comps) {
        filesystem::ListOutcome::Dir(entries) => ShellOutput::ok(entries),
        filesystem::ListOutcome::File(entry) => ShellOutput::ok(vec![entry]),
        filesystem::ListOutcome::NotFound => ShellOutput::code(
            vec![format!("ls: {target}: No such file or directory")],
            2,
        ),
    }
}

/// `find [patrón]` como **fuente** de tubería: emite las rutas cuyo nombre
/// contiene `patrón` (todas si se omite), una por línea y sin sangrado.
fn find(state: &GameState, args: &[String]) -> ShellOutput {
    let needle = first_positional(args);
    let hits = filesystem::search(&state.pentest().target.filesystem, needle);
    ShellOutput::ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::campaign::Campaign;
    use crate::model::filesystem::FsNode;
    use crate::model::language::Language;
    use crate::model::mission::{EntryVector, Mission};
    use crate::model::target::TargetNode;
    use crate::model::theme::Theme;
    use std::collections::BTreeMap;

    fn host() -> TargetNode {
        TargetNode {
            hostname: String::from("lab"),
            ip: String::from("LAB"),
            os: String::from("sim"),
            services: vec![],
            vulnerabilities: vec![],
            filesystem: vec![
                FsNode::Dir {
                    name: String::from("datos"),
                    children: vec![FsNode::File {
                        name: String::from("app.log"),
                        content: vec![
                            String::from("INFO boot"),
                            String::from("ERROR a"),
                            String::from("WARN x"),
                            String::from("ERROR b"),
                        ],
                        root: false,
                        loot: None,
                        binary: None,
                        encoding: None,
                    }],
                },
                FsNode::Dir {
                    name: String::from("tmp"),
                    children: vec![],
                },
            ],
            accepts_token: None,
            local_privesc: None,
        }
    }

    fn state() -> GameState {
        let campaign = Campaign {
            name: String::from("T"),
            language: Language::En,
            intro: vec![],
            stages: crate::model::campaign::default_stages(),
            domain: Some(crate::model::campaign::DomainKind::Bare),
            features: Default::default(),
            theme: Theme::default(),
            easter_eggs: vec![],
            fortunes: vec![],
            signals: vec![],
            achievements: vec![],
            commands: vec![],
            env: BTreeMap::new(),
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
    fn tuberia_grep_wc_cuenta_coincidencias() {
        let mut g = state();
        let r = run_pipeline(&mut g, "grep ERROR /datos/app.log | wc -l");
        assert_eq!(r.exit, 0);
        assert_eq!(r.lines.len(), 1);
        assert_eq!(r.lines[0].trim(), "2");
    }

    #[test]
    fn redireccion_escribe_y_se_puede_leer() {
        let mut g = state();
        let w = run_pipeline(&mut g, "grep ERROR /datos/app.log > /tmp/err.txt");
        assert_eq!(w.exit, 0);
        assert!(w.lines.is_empty(), "la salida fue al fichero, no al log");
        // El fichero creado se lee con cat y registra FileRead.
        let c = run_pipeline(&mut g, "cat /tmp/err.txt | wc -l");
        assert_eq!(c.lines[0].trim(), "2");
        assert!(g.has_read("/tmp/err.txt"));
    }

    #[test]
    fn append_agrega_al_final() {
        let mut g = state();
        run_pipeline(&mut g, "grep ERROR /datos/app.log > /tmp/err.txt");
        run_pipeline(&mut g, "grep WARN /datos/app.log >> /tmp/err.txt");
        let c = run_pipeline(&mut g, "wc -l /tmp/err.txt");
        assert_eq!(c.lines[0].trim_start().chars().next(), Some('3'));
    }

    #[test]
    fn redireccion_a_directorio_inexistente_falla() {
        let mut g = state();
        let r = run_pipeline(&mut g, "echo hola > /no/existe.txt");
        assert_eq!(r.exit, 1);
        assert!(r.lines[0].contains("No such file"));
    }

    #[test]
    fn sort_uniq_cuenta_niveles() {
        let mut g = state();
        // Extrae la primera palabra de cada línea sería ideal, pero probamos el
        // encadenado sort|uniq -c sobre líneas completas.
        let r = run_pipeline(&mut g, "cat /datos/app.log | sort | uniq -c");
        // 4 líneas distintas -> 4 grupos de 1.
        assert_eq!(r.lines.len(), 4);
        assert!(r.lines.iter().all(|l| l.trim_start().starts_with('1')));
    }
}
