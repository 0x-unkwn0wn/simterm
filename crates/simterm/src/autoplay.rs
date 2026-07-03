//! Autoplayer visible: decide el siguiente comando y lo pasa por el dispatcher.
//!
//! No es una vía rápida del motor. Solo inspecciona el estado cargado para evitar
//! adivinar y emite comandos normales (`nmap`, `nikto 443`, `cat ...`, etc.).
//!
//! Nota de portabilidad: el motor open source no rastrea qué binarios se han
//! inspeccionado con `strings` (no es estado de partida), así que ese recuerdo
//! —usado solo para que el autoplay haga `strings` una vez antes de `solve`—
//! vive aquí, dentro del propio `Autoplay`.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use simterm_engine::filesystem::{self, FsNode, Reward};
use simterm_engine::model::target::{ExploitReliability, Vulnerability};
use simterm_engine::{toolbox, EntryVector, FindingStatus, GameOutcome, GameState, ServiceCat};

#[derive(Debug, Clone, Copy)]
pub struct AutoplayConfig {
    pub delay: Duration,
    pub mode: AutoplayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoplayMode {
    Normal,
    Strict,
}

impl AutoplayConfig {
    pub fn with_delay(delay_ms: u64) -> Self {
        Self {
            delay: Duration::from_millis(delay_ms.max(50)),
            mode: AutoplayMode::Normal,
        }
    }

    pub fn strict() -> Self {
        Self {
            mode: AutoplayMode::Strict,
            ..Self::default()
        }
    }

    pub fn set_delay(&mut self, delay_ms: u64) {
        self.delay = Duration::from_millis(delay_ms.max(50));
    }
}

impl Default for AutoplayConfig {
    fn default() -> Self {
        Self::with_delay(900)
    }
}

pub struct Autoplay {
    config: AutoplayConfig,
    last_step: Option<Instant>,
    stopped: bool,
    /// Binarios ya inspeccionados con `strings` en esta sesión (frontend-only:
    /// el motor no lo rastrea). Evita repetir `strings` antes de `solve`.
    inspected: HashSet<String>,
}

impl Autoplay {
    pub fn new(config: AutoplayConfig) -> Self {
        Self {
            config,
            last_step: None,
            stopped: false,
            inspected: HashSet::new(),
        }
    }

    pub fn next_command(&mut self, game: &GameState, now: Instant) -> Option<String> {
        if self.stopped {
            return None;
        }
        if self
            .last_step
            .is_some_and(|last| now.duration_since(last) < self.config.delay)
        {
            return None;
        }

        let Decision::Command(cmd) = decide(game, self.config.mode, &mut self.inspected)?;
        self.last_step = Some(now);
        Some(cmd)
    }
}

enum Decision {
    Command(String),
}

fn command(cmd: impl Into<String>) -> Option<Decision> {
    Some(Decision::Command(cmd.into()))
}

fn decide(game: &GameState, mode: AutoplayMode, inspected: &mut HashSet<String>) -> Option<Decision> {
    match game.outcome {
        Some(GameOutcome::Victory) => return command("quit"),
        Some(GameOutcome::Defeat) => return None,
        None => {}
    }

    if game.core.awaiting_choice {
        // El guion original elige el desenlace 3 (la campaña oficial tiene 3+
        // finales). Se acota al nº de finales para no colgarse en campañas con
        // menos: en la oficial sigue siendo exactamente `choose 3`.
        let endings = game.campaign.missions[game.level_index].endings.len();
        let choice = 3.min(endings.max(1));
        return command(format!("choose {choice}"));
    }

    if !game.has_foothold() {
        return pre_foothold(game, mode);
    }

    if !game.is_root {
        return post_user(game, inspected);
    }

    post_root(game, inspected).map(Decision::Command)
}

fn pre_foothold(game: &GameState, mode: AutoplayMode) -> Option<Decision> {
    if matches!(game.entry, EntryVector::Pivot { .. }) && !game.pivoted {
        return command("connect");
    }

    if let Some(tok) = &game.target.accepts_token {
        if game.foothold_tokens.contains(tok) {
            return command("login");
        }
    }

    let candidate = best_vuln(game, mode).or_else(|| best_vuln(game, AutoplayMode::Normal))?;
    if !game
        .discovered_ports
        .contains(&candidate.vuln.affected_service)
    {
        return command(match game.entry {
            EntryVector::Passive => String::from("sniff"),
            _ => String::from("nmap"),
        });
    }

    let finding = game
        .intel
        .iter()
        .find(|f| f.real_vuln_id.as_deref() == Some(candidate.vuln.id.as_str()));

    let Some(finding) = finding else {
        return command(format!(
            "{} {}",
            tool_for(candidate.service_cat),
            candidate.vuln.affected_service
        ));
    };

    if finding.status == FindingStatus::Failed {
        return fallback_after_failed_exploit(game, mode, &candidate.vuln.id);
    }

    if finding.verify_pos + finding.verify_neg == 0 {
        return command(format!("searchsploit {}", finding.public_id));
    }

    command(format!("exploit {}", finding.public_id))
}

fn fallback_after_failed_exploit(
    game: &GameState,
    mode: AutoplayMode,
    failed_vuln: &str,
) -> Option<Decision> {
    let fallback = sorted_vulns(game, mode)
        .into_iter()
        .chain(sorted_vulns(game, AutoplayMode::Normal))
        .find(|c| c.vuln.id != failed_vuln);
    let fallback = fallback?;

    if !game
        .discovered_ports
        .contains(&fallback.vuln.affected_service)
    {
        return command("nmap");
    }
    if let Some(f) = game
        .intel
        .iter()
        .find(|f| f.real_vuln_id.as_deref() == Some(fallback.vuln.id.as_str()))
    {
        return command(format!("exploit {}", f.public_id));
    }
    command(format!(
        "{} {}",
        tool_for(fallback.service_cat),
        fallback.vuln.affected_service
    ))
}

fn post_user(game: &GameState, inspected: &mut HashSet<String>) -> Option<Decision> {
    if let Some(cmd) = next_crack_or_solve_command(game, inspected) {
        return command(cmd);
    }

    if let Some(cmd) = next_loot_command(game, false, false) {
        return command(cmd);
    }

    if !game.privesc_unlocked {
        if let Some(local) = &game.target.local_privesc {
            let _ = local;
            return command("linpeas");
        }
        if let Some(cmd) = next_loot_command(game, false, true) {
            return command(cmd);
        }
    }

    command("privesc")
}

fn post_root(game: &GameState, inspected: &mut HashSet<String>) -> Option<String> {
    if let Some(cmd) = next_loot_command(game, true, true) {
        return Some(cmd);
    }

    if let Some(cmd) = next_crack_or_solve_command(game, inspected) {
        return Some(cmd);
    }

    if let Some(obj) = &game.objective {
        return Some(format!("exfil {obj}"));
    }

    if !game.is_single_host() {
        if let Some((name, _, _)) = game
            .network_overview()
            .into_iter()
            .find(|(_, marker, active)| !*active && *marker == '+')
        {
            return Some(format!("pivot {name}"));
        }
        return Some(String::from("netmap"));
    }

    None
}

fn next_loot_command(
    game: &GameState,
    include_root: bool,
    include_privesc_keys: bool,
) -> Option<String> {
    let mut files = Vec::new();
    collect_files(&game.target.filesystem, &mut Vec::new(), &mut files);

    files
        .into_iter()
        .filter(|f| include_root || !f.root)
        .filter(|f| !game.looted_paths.contains(&f.path))
        .filter(|f| {
            f.loot
                .as_ref()
                .is_some_and(|loot| include_privesc_keys || !loot_unlocks_privesc(loot))
        })
        .min_by_key(|f| loot_priority(f.loot.as_ref().unwrap()))
        .map(|f| {
            if f.binary_secret.is_some() {
                format!("strings {}", f.path)
            } else if let Some(filesystem::Encoding::Base64) = &f.encoding {
                format!("base64 {}", f.path)
            } else if let Some(filesystem::Encoding::Xor(key)) = &f.encoding {
                format!("xor {} {key}", f.path)
            } else {
                format!("cat {}", f.path)
            }
        })
}

fn next_crack_or_solve_command(game: &GameState, inspected: &mut HashSet<String>) -> Option<String> {
    let mut files = Vec::new();
    collect_files(&game.target.filesystem, &mut Vec::new(), &mut files);

    for f in &files {
        if f.root && !game.is_root {
            continue;
        }
        if f.binary_secret.is_some()
            && !inspected.contains(&f.path)
            && !game.solved_paths.contains(&f.path)
        {
            inspected.insert(f.path.clone());
            return Some(format!("strings {}", f.path));
        }
    }

    for f in &files {
        if f.root && !game.is_root {
            continue;
        }
        let Some(loot) = &f.loot else {
            continue;
        };
        if let Some(hash) = &loot.hash {
            if (!hash.needs_wordlist || game.has_wordlist)
                && game.looted_paths.contains(&f.path)
                && !game.cracked_paths.contains(&f.path)
            {
                return Some(format!("john {}", f.path));
            }
        }
    }

    for f in &files {
        if f.root && !game.is_root {
            continue;
        }
        let Some(secret) = &f.binary_secret else {
            continue;
        };
        if !game.solved_paths.contains(&f.path) {
            return Some(format!("solve {} {secret}", f.path));
        }
    }

    None
}

fn loot_priority(loot: &filesystem::Loot) -> u8 {
    if loot.wordlist {
        0
    } else if loot
        .hash
        .as_ref()
        .is_some_and(|h| matches!(h.yields, Reward::PrivescKey))
    {
        1
    } else if loot.hash.is_some() {
        2
    } else if loot.foothold_token.is_some() {
        3
    } else if loot.credential.is_some() {
        4
    } else if loot.privesc_key {
        5
    } else {
        6
    }
}

fn loot_unlocks_privesc(loot: &filesystem::Loot) -> bool {
    if loot.privesc_key {
        true
    } else {
        loot.hash
            .as_ref()
            .is_some_and(|h| matches!(h.yields, Reward::PrivescKey))
    }
}

#[derive(Clone)]
struct FileInfo {
    path: String,
    root: bool,
    loot: Option<filesystem::Loot>,
    encoding: Option<filesystem::Encoding>,
    binary_secret: Option<String>,
}

fn collect_files(nodes: &[FsNode], path: &mut Vec<String>, out: &mut Vec<FileInfo>) {
    for node in nodes {
        path.push(node.name().to_string());
        match node {
            FsNode::Dir { children, .. } => collect_files(children, path, out),
            FsNode::File {
                root,
                loot,
                encoding,
                binary,
                ..
            } => out.push(FileInfo {
                path: filesystem::path_string(path),
                root: *root,
                loot: loot.clone(),
                encoding: encoding.clone(),
                binary_secret: binary.as_ref().map(|b| b.secret.clone()),
            }),
        }
        path.pop();
    }
}

struct VulnCandidate<'a> {
    vuln: &'a Vulnerability,
    service_cat: ServiceCat,
}

fn best_vuln(game: &GameState, mode: AutoplayMode) -> Option<VulnCandidate<'_>> {
    sorted_vulns(game, mode).into_iter().next()
}

fn sorted_vulns(game: &GameState, mode: AutoplayMode) -> Vec<VulnCandidate<'_>> {
    let mut candidates: Vec<_> = game
        .target
        .vulnerabilities
        .iter()
        .filter(|vuln| {
            mode != AutoplayMode::Strict || matches!(vuln.reliability, ExploitReliability::Reliable)
        })
        .filter_map(|vuln| {
            let service = game
                .target
                .services
                .iter()
                .find(|s| s.port == vuln.affected_service)?;
            if let Some(required) = &service.requires {
                if !game.foothold_tokens.contains(required) {
                    return None;
                }
            }
            Some(VulnCandidate {
                vuln,
                service_cat: toolbox::category(&service.name),
            })
        })
        .collect();

    candidates.sort_by_key(|c| {
        let reliable = if matches!(c.vuln.reliability, ExploitReliability::Reliable) {
            0
        } else {
            1
        };
        (
            reliable,
            c.vuln.difficulty,
            c.vuln.stealth_cost,
            tool_noise(tool_for(c.service_cat)) as u8,
        )
    });
    candidates
}

fn tool_for(cat: ServiceCat) -> &'static str {
    match cat {
        ServiceCat::Web => "nikto",
        ServiceCat::Smb => "enum4linux",
        ServiceCat::Ssh => "hydra",
        ServiceCat::Db => "sqlmap",
        ServiceCat::Other => "probe",
    }
}

fn tool_noise(name: &str) -> f32 {
    toolbox::tool_by_name(name).map(|t| t.noise).unwrap_or(99.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{self, Command};
    use simterm_engine::{actions, load_campaign};
    use std::path::PathBuf;

    fn sample_campaign_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("sample_campaign")
    }

    /// Aplica una línea del autoplay al estado, tal como haría el dispatcher del
    /// frontend. Hace panic si el autoplay emite un comando no soportado (esa es
    /// justamente la comprobación clave: solo debe emitir comandos válidos).
    fn run(game: &mut GameState, line: &str) -> bool {
        match command::parse(line, true) {
            Command::Recon => actions::recon(game),
            Command::Sniff => actions::sniff(game),
            Command::Connect(host) => actions::connect(game, host),
            Command::Netmap => actions::netmap(game),
            Command::Pivot(host) => actions::pivot(game, host),
            Command::Enumerate(tool, port) => actions::enumerate(game, &tool, port),
            Command::Research(id) => actions::research(game, id),
            Command::Exploit(id) => actions::exploit(game, id),
            Command::Login => actions::login(game),
            Command::Privesc => actions::privesc(game),
            Command::Cat(path) => actions::fs_cat(game, path),
            Command::Exfil(path) => actions::fs_exfil(game, path),
            Command::John(path) => actions::john(game, path),
            Command::Strings(path) => actions::strings(game, path),
            Command::Disasm(path) => actions::disasm(game, path),
            Command::Solve(path, secret) => actions::solve(game, path, secret),
            Command::DecodeFile { tool, path, key } => actions::decode_cmd(game, &tool, path, key),
            Command::LocalEnum(tool) => actions::local_enum(game, &tool),
            Command::Choose(Some(c)) if c >= 1 => game.resolve_ending(c - 1),
            Command::Quit => return false,
            other => panic!("autoplay emitió un comando no soportado desde '{line}': {other:?}"),
        }
        true
    }

    #[test]
    fn decision_pura_es_determinista() {
        let campaign = load_campaign(sample_campaign_path()).expect("la campaña de ejemplo carga");
        let mut game = GameState::new(campaign);
        let mut inspected = HashSet::new();

        // Victoria -> 'quit'.
        game.outcome = Some(GameOutcome::Victory);
        assert!(matches!(
            decide(&game, AutoplayMode::Strict, &mut inspected),
            Some(Decision::Command(ref c)) if c == "quit"
        ));

        // Derrota -> no hay nada que hacer.
        game.outcome = Some(GameOutcome::Defeat);
        assert!(decide(&game, AutoplayMode::Strict, &mut inspected).is_none());
    }

    #[test]
    fn autoplay_solo_emite_comandos_validos_y_termina() {
        let campaign = load_campaign(sample_campaign_path()).expect("la campaña de ejemplo carga");
        let mut game = GameState::new(campaign);
        let mut inspected = HashSet::new();

        let mut commands = Vec::new();
        let mut terminated = false;
        for _ in 0..400 {
            let Some(Decision::Command(cmd)) = decide(&game, AutoplayMode::Normal, &mut inspected)
            else {
                terminated = true;
                break;
            };
            let keep_going = run(&mut game, &cmd);
            commands.push(cmd);
            if !keep_going || game.outcome.is_some() {
                terminated = true;
                break;
            }
        }

        // Ni bucles infinitos ni comandos inválidos: el autoplay avanza y termina.
        assert!(terminated, "el autoplay no terminó: {commands:?}");
        assert!(
            commands.len() >= 5,
            "el autoplay apenas avanzó: {commands:?}"
        );
    }
}
