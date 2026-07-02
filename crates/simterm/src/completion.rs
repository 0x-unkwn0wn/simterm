//! Autocompletado con Tab, al estilo de una shell real.
//!
//! Es contextual: en la primera posición completa **comandos**; a partir de ahí
//! completa según el verbo —rutas del VFS (`ls`/`cat`/`cd`), puertos descubiertos
//! (herramientas de enumeración) o IDs de hallazgo (`exploit`/`searchsploit`)—.
//!
//! Comportamiento (como bash con una sola pulsación):
//!   - 1 candidato  -> se completa el token (y se añade espacio salvo en directorios).
//!   - varios con prefijo común mayor -> se extiende hasta el prefijo común.
//!   - varios sin avance posible -> se listan los candidatos.

use simterm_engine::{filesystem, toolbox, GameState};

use crate::registry;

/// Resultado del autocompletado para una línea de entrada.
pub enum Completion {
    /// Sin candidatos: no se hace nada.
    None,
    /// Sustituir la línea de entrada por esta.
    Replace(String),
    /// Varios candidatos sin avance posible: mostrar la lista, dejar la línea.
    List { options: Vec<String> },
}

/// Calcula el autocompletado para `input` en el estado de juego actual.
pub fn complete(state: &GameState, input: &str) -> Completion {
    let frag = current_fragment(input);
    let split = input.len() - frag.len();
    let prefix = &input[..split];

    let tokens: Vec<&str> = input.split_whitespace().collect();
    let ends_space = input.ends_with(|c: char| c.is_whitespace());
    let completing_verb = tokens.is_empty() || (tokens.len() == 1 && !ends_space);

    let candidates = if completing_verb {
        verb_candidates(state, frag)
    } else {
        arg_candidates(state, tokens[0], frag)
    };

    finish(prefix, frag, candidates)
}

/// El "token actual": lo que hay tras el último espacio (vacío si la línea
/// termina en espacio, es decir, se empieza un argumento nuevo).
fn current_fragment(input: &str) -> &str {
    if input.ends_with(|c: char| c.is_whitespace()) {
        ""
    } else {
        input
            .rsplit(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("")
    }
}

fn finish(prefix: &str, frag: &str, mut cands: Vec<String>) -> Completion {
    cands.sort();
    cands.dedup();

    match cands.len() {
        0 => Completion::None,
        1 => {
            let c = &cands[0];
            // Tras un directorio no se añade espacio (se sigue navegando).
            let tail = if c.ends_with('/') { "" } else { " " };
            Completion::Replace(format!("{prefix}{c}{tail}"))
        }
        _ => {
            let cp = common_prefix(&cands);
            if cp.chars().count() > frag.chars().count() {
                Completion::Replace(format!("{prefix}{cp}"))
            } else {
                Completion::List { options: cands }
            }
        }
    }
}

// ----------------------------- Candidatos -----------------------------

fn verb_candidates(state: &GameState, frag: &str) -> Vec<String> {
    // Verbos built-in (nombres + alias) y herramientas, desde el registro único.
    let mut v: Vec<String> = registry::all_verbs()
        .iter()
        .map(|s| s.to_string())
        .collect();
    // Comandos declarativos de campaña no ocultos: se completan como cualquier otro.
    for cmd in &state.campaign.commands {
        if cmd.hidden {
            continue;
        }
        for t in &cmd.triggers {
            v.push(t.clone());
        }
    }
    // Comandos de terminal autorados no ocultos.
    for cmd in &state.campaign.terminal {
        if cmd.hidden {
            continue;
        }
        for t in &cmd.triggers {
            v.push(t.clone());
        }
    }
    v.retain(|c| c.starts_with(frag));
    v
}

fn arg_candidates(state: &GameState, verb: &str, frag: &str) -> Vec<String> {
    let verb = verb.to_lowercase();
    match verb.as_str() {
        // Rutas del sistema de archivos (solo con shell).
        "ls" | "dir" | "cat" | "read" | "type" | "exfil" | "cd" | "john" | "hashcat"
        | "strings" | "disasm" | "objdump" | "r2" | "solve" | "base64" | "xor" | "grep"
        | "head" | "tail" | "wc" | "file" => path_candidates(state, frag),
        "sudo" => ["-l"]
            .iter()
            .filter(|c| c.starts_with(frag))
            .map(|s| s.to_string())
            .collect(),
        // IDs de hallazgo para investigar/explotar.
        "exploit" | "run" | "searchsploit" | "verify" | "research" => id_candidates(state, frag),
        // Puertos descubiertos para las herramientas de enumeración.
        other if toolbox::tool_by_name(other).is_some() => port_candidates(state, frag),
        _ => Vec::new(),
    }
}

fn path_candidates(state: &GameState, frag: &str) -> Vec<String> {
    if !state.has_foothold() {
        return Vec::new();
    }
    // Se separa el fragmento en "base" (hasta la última '/') y "hoja".
    let (base, leaf) = match frag.rfind('/') {
        Some(i) => (&frag[..=i], &frag[i + 1..]),
        None => ("", frag),
    };
    let comps = filesystem::normalize(&state.cwd, base);
    let children = match filesystem::dir_children(&state.target.filesystem, &comps) {
        Some(c) => c,
        None => return Vec::new(),
    };
    children
        .into_iter()
        .filter(|(name, _)| name.starts_with(leaf))
        .map(|(name, is_dir)| {
            let slash = if is_dir { "/" } else { "" };
            format!("{base}{name}{slash}")
        })
        .collect()
}

fn port_candidates(state: &GameState, frag: &str) -> Vec<String> {
    state
        .discovered_ports
        .iter()
        .map(|p| p.to_string())
        .filter(|p| p.starts_with(frag))
        .collect()
}

fn id_candidates(state: &GameState, frag: &str) -> Vec<String> {
    state
        .intel
        .iter()
        .map(|f| f.public_id.to_string())
        .filter(|id| id.starts_with(frag))
        .collect()
}

/// Dispone una lista de candidatos en columnas alineadas que quepan en `width`,
/// como hace una shell real al listar el autocompletado. El relleno es vertical
/// (column-major, igual que `ls`): se recorre hacia abajo y luego a la derecha.
/// Devuelve una línea de log por fila.
pub fn format_columns(items: &[String], width: u16) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    let width = width.max(20) as usize;
    let longest = items.iter().map(|s| s.chars().count()).max().unwrap_or(1);
    let col_w = longest + 2; // dos espacios de separación entre columnas
    let cols = (width / col_w).max(1);
    let rows = items.len().div_ceil(cols);

    let mut lines = Vec::with_capacity(rows);
    for r in 0..rows {
        let mut line = String::new();
        for c in 0..cols {
            if let Some(item) = items.get(c * rows + r) {
                line.push_str(&format!("{item:<col_w$}"));
            }
        }
        // El relleno de la última celda deja espacios sobrantes: se recortan.
        lines.push(line.trim_end().to_string());
    }
    lines
}

/// Prefijo común (por caracteres) de un conjunto no vacío de candidatos.
fn common_prefix(cands: &[String]) -> String {
    let mut prefix: Vec<char> = cands[0].chars().collect();
    for c in &cands[1..] {
        let other: Vec<char> = c.chars().collect();
        let mut k = 0;
        while k < prefix.len() && k < other.len() && prefix[k] == other[k] {
            k += 1;
        }
        prefix.truncate(k);
    }
    prefix.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use simterm_engine::load_campaign;
    use std::path::PathBuf;

    /// Construye un estado de juego cargando la campaña de ejemplo del repo.
    fn game() -> GameState {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("sample_campaign");
        GameState::new(load_campaign(path).expect("la campaña de ejemplo debe cargar"))
    }

    #[test]
    fn completa_prefijo_unico_de_comando() {
        let g = game();
        // 'priv' solo casa con 'privesc'.
        match complete(&g, "priv") {
            Completion::Replace(s) => assert_eq!(s, "privesc "),
            _ => panic!("esperaba Replace"),
        }
    }

    #[test]
    fn extiende_al_prefijo_comun() {
        let g = game();
        // 'lo' -> 'logs' y 'loot' comparten 'lo'; no hay avance -> lista.
        match complete(&g, "lo") {
            Completion::List { options } => {
                assert!(options.contains(&"logs".to_string()));
                assert!(options.contains(&"loot".to_string()));
            }
            _ => panic!("esperaba List"),
        }
    }

    #[test]
    fn sin_candidatos_no_hace_nada() {
        let g = game();
        assert!(matches!(complete(&g, "zzzz"), Completion::None));
    }

    #[test]
    fn columnas_ajustan_al_ancho_sin_romper_palabras() {
        let items: Vec<String> = ["ls", "cat", "cd", "privesc", "exploit", "nmap"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // Ancho estrecho: varias filas, ninguna excede el ancho.
        let lines = format_columns(&items, 24);
        assert!(lines.len() > 1, "esperaba varias filas");
        for l in &lines {
            assert!(l.chars().count() <= 24, "fila demasiado ancha: {l:?}");
            // Ningún candidato debe aparecer partido: todos íntegros en alguna fila.
        }
        let joined: String = lines.join(" ");
        for it in &items {
            assert!(joined.contains(it.as_str()), "falta '{it}' íntegro");
        }
        // Lista vacía -> sin líneas.
        assert!(format_columns(&[], 80).is_empty());
    }
}
