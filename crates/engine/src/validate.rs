//! Validación semántica de campañas (`--doctor`).
//!
//! A diferencia del [`crate::loader`], que solo comprueba que el RON se puede
//! interpretar como una [`Campaign`], esta capa hace un análisis *semántico*:
//! busca referencias colgantes, contenido inalcanzable, IDs duplicados y valores
//! fuera de rango que la simple carga no detecta.
//!
//! Vive en el motor (no en el frontend) para que cualquier herramienta pueda
//! reutilizarla. Para detectar colisiones con comandos "built-in" (que son
//! propiedad del frontend), el llamante pasa una lista neutral de verbos
//! reservados; el motor no conoce el catálogo concreto del frontend.

use std::collections::HashSet;

use crate::model::campaign::{Campaign, CampaignAchievementTrigger};
use crate::model::command::{CommandCondition, CommandEffect};
use crate::model::filesystem::{self, FsNode, ReadOutcome, Reward};
use crate::model::mission::Mission;
use crate::model::target::TargetNode;

/// Un hallazgo de validación (error o aviso) con su localización legible.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Dónde se detectó (p. ej. `"misión 'op1'"`).
    pub location: String,
    /// Descripción del problema.
    pub message: String,
}

/// Resultado de validar una campaña: errores (rompen la campaña) y avisos
/// (huelen a error pero no la invalidan).
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

impl ValidationReport {
    fn error(&mut self, location: impl Into<String>, message: impl Into<String>) {
        self.errors.push(ValidationIssue {
            location: location.into(),
            message: message.into(),
        });
    }

    fn warn(&mut self, location: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(ValidationIssue {
            location: location.into(),
            message: message.into(),
        });
    }

    /// ¿Hay al menos un error? (Determina el código de salida de `--doctor`.)
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// ¿Sin errores ni avisos?
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }
}

/// Hosts jugables de una misión: en modo clásico, el `target` con el objetivo de
/// la misión; en modo red, cada `NetHost` con su propio objetivo.
fn hosts_of(m: &Mission) -> Vec<(&TargetNode, &Option<String>)> {
    if m.network.is_empty() {
        vec![(&m.target, &m.objective)]
    } else {
        m.network
            .iter()
            .map(|h| (&h.target, &h.objective))
            .collect()
    }
}

/// ¿Existe un fichero (no directorio) en el VFS `root` para la ruta `path`?
fn vfs_has_file(root: &[FsNode], path: &str) -> bool {
    let comps = filesystem::normalize(&[], path);
    matches!(
        filesystem::read_file(root, &comps),
        ReadOutcome::File { .. }
    )
}

/// Recorre un árbol VFS acumulando en `out` los tokens que se pueden OBTENER en
/// él: `foothold_token` de botín y recompensas `Token` de hashes y binarios.
fn collect_tokens(nodes: &[FsNode], out: &mut HashSet<String>) {
    for n in nodes {
        match n {
            FsNode::Dir { children, .. } => collect_tokens(children, out),
            FsNode::File { loot, binary, .. } => {
                if let Some(l) = loot {
                    if let Some(tok) = &l.foothold_token {
                        out.insert(tok.clone());
                    }
                    if let Some(h) = &l.hash {
                        if let Reward::Token(t) = &h.yields {
                            out.insert(t.clone());
                        }
                    }
                }
                if let Some(b) = binary {
                    if let Reward::Token(t) = &b.yields {
                        out.insert(t.clone());
                    }
                }
            }
        }
    }
}

/// Rangos de balance del motor usados para avisar de valores atípicos.
const SKILL_RANGE: (f32, f32) = (0.0, 1.0);
const DIFFICULTY_RANGE: (u8, u8) = (1, 10);

/// Valida una campaña ya cargada y devuelve el informe de errores y avisos.
///
/// `reserved_verbs` es la lista neutral de comandos "built-in" del frontend (sin
/// los verbos que el frontend permita reutilizar como sabor, p. ej. `sudo`).
/// Sirve para detectar colisiones de easter eggs y comandos declarativos con la
/// mecánica del juego. Pasa `&[]` si no quieres esa comprobación.
pub fn validate_campaign(campaign: &Campaign, reserved_verbs: &[&str]) -> ValidationReport {
    let mut report = ValidationReport::default();

    // ------------------------------ Misiones ------------------------------
    if campaign.missions.is_empty() {
        report.error("campaña", "no contiene misiones");
    }

    let mut seen_missions: HashSet<&str> = HashSet::new();
    let mission_ids: HashSet<&str> = campaign.missions.iter().map(|m| m.id.as_str()).collect();

    // Ids de medidores declarados en cualquier misión (para validar `AddMeter`).
    let declared_meters: HashSet<&str> = campaign
        .missions
        .iter()
        .flat_map(|m| m.meters.iter().map(|d| d.id.as_str()))
        .collect();

    // Tokens obtenibles en toda la campaña (para gating de servicios).
    let mut obtainable: HashSet<String> = HashSet::new();
    for m in &campaign.missions {
        for (host, _) in hosts_of(m) {
            collect_tokens(&host.filesystem, &mut obtainable);
        }
    }

    for m in &campaign.missions {
        let loc = format!("misión '{}'", m.id);

        if m.id.trim().is_empty() {
            report.error(&loc, "el id de misión está vacío");
        } else if !seen_missions.insert(m.id.as_str()) {
            report.error(&loc, format!("id de misión duplicado: '{}'", m.id));
        }

        // Valores fuera de rango (avisos: el motor los tolera/clampa).
        if m.skill < SKILL_RANGE.0 || m.skill > SKILL_RANGE.1 {
            report.warn(
                &loc,
                format!(
                    "skill {:.2} fuera de rango [{:.1}, {:.1}]",
                    m.skill, SKILL_RANGE.0, SKILL_RANGE.1
                ),
            );
        }
        if m.root_difficulty < DIFFICULTY_RANGE.0 || m.root_difficulty > DIFFICULTY_RANGE.1 {
            report.warn(
                &loc,
                format!(
                    "root_difficulty {} fuera de rango [{}, {}]",
                    m.root_difficulty, DIFFICULTY_RANGE.0, DIFFICULTY_RANGE.1
                ),
            );
        }
        if m.detection_limit <= 0.0 {
            report.warn(
                &loc,
                format!("detection_limit debe ser > 0 (es {:.1})", m.detection_limit),
            );
        }
        if matches!(m.time_limit, Some(0)) {
            report.warn(
                &loc,
                "time_limit es Some(0): la operación se pierde al instante",
            );
        }

        // Medidores del nivel: ids no vacíos y únicos dentro de la misión.
        let mut seen_meters: HashSet<&str> = HashSet::new();
        for d in &m.meters {
            if d.id.trim().is_empty() {
                report.error(&loc, "un medidor del nivel tiene id vacío");
            } else if !seen_meters.insert(d.id.as_str()) {
                report.error(&loc, format!("medidor de nivel duplicado: '{}'", d.id));
            }
        }

        // Hosts de la misión.
        let hosts = hosts_of(m);
        for (host, objective) in &hosts {
            let host_loc = if host.hostname.is_empty() {
                loc.clone()
            } else {
                format!("{loc} · host '{}'", host.hostname)
            };
            let ports: HashSet<u16> = host.services.iter().map(|s| s.port).collect();

            // Vulnerabilidades: su servicio afectado debe existir.
            for v in &host.vulnerabilities {
                if !ports.contains(&v.affected_service) {
                    report.error(
                        &host_loc,
                        format!(
                            "la vulnerabilidad '{}' afecta al puerto {} que no está en services",
                            v.id, v.affected_service
                        ),
                    );
                }
                if v.difficulty < DIFFICULTY_RANGE.0 || v.difficulty > DIFFICULTY_RANGE.1 {
                    report.warn(
                        &host_loc,
                        format!(
                            "la vulnerabilidad '{}' tiene difficulty {} fuera de [{}, {}]",
                            v.id, v.difficulty, DIFFICULTY_RANGE.0, DIFFICULTY_RANGE.1
                        ),
                    );
                }
            }

            // Servicios con `requires`: el token debe poder obtenerse.
            for s in &host.services {
                if let Some(tok) = &s.requires {
                    if !obtainable.contains(tok) {
                        report.error(
                            &host_loc,
                            format!(
                                "el servicio {}/{} exige el token '{}', que no se obtiene en ningún punto de la campaña",
                                s.port, s.name, tok
                            ),
                        );
                    }
                }
            }

            // `accepts_token`: si nunca se obtiene, el `login` es inútil (aviso;
            // el `exploit` sigue siendo una vía alternativa).
            if let Some(tok) = &host.accepts_token {
                if !obtainable.contains(tok) {
                    report.warn(
                        &host_loc,
                        format!(
                            "accepts_token '{}' no se obtiene en ningún punto: 'login' nunca funcionará aquí",
                            tok
                        ),
                    );
                }
            }

            // Objetivo: debe apuntar a un fichero real del VFS del host.
            if let Some(obj) = objective {
                if !vfs_has_file(&host.filesystem, obj) {
                    report.error(
                        &host_loc,
                        format!(
                            "el objetivo '{}' no apunta a ningún fichero del VFS del host",
                            obj
                        ),
                    );
                }
            }
        }

        // Red interna: hostnames duplicados y referencias de pivot (`links`).
        if !m.network.is_empty() {
            let mut seen_hosts: HashSet<&str> = HashSet::new();
            let names: HashSet<&str> = m
                .network
                .iter()
                .flat_map(|h| [h.target.hostname.as_str(), h.target.short_name()])
                .collect();
            for h in &m.network {
                if !seen_hosts.insert(h.target.hostname.as_str()) {
                    report.error(
                        &loc,
                        format!("host de red duplicado: '{}'", h.target.hostname),
                    );
                }
                for link in &h.links {
                    if !names.contains(link.as_str()) {
                        report.error(
                            &loc,
                            format!(
                                "el host '{}' enlaza (pivot) con '{}', que no existe en la red",
                                h.target.hostname, link
                            ),
                        );
                    }
                }
            }
            if !m.network.iter().any(|h| h.entry) {
                report.warn(
                    &loc,
                    "ningún host de la red está marcado como entry: se usará el primero",
                );
            }
        }
    }

    // ------------------------------ Logros ------------------------------
    let mut seen_achievements: HashSet<&str> = HashSet::new();
    for a in &campaign.achievements {
        let loc = format!("logro '{}'", a.id);
        if a.id.trim().is_empty() {
            report.error("logro", "el id de logro está vacío");
        } else if !seen_achievements.insert(a.id.as_str()) {
            report.error(&loc, format!("id de logro duplicado: '{}'", a.id));
        }

        match &a.trigger {
            CampaignAchievementTrigger::ReadFile(path) => {
                let exists = campaign.missions.iter().any(|m| {
                    hosts_of(m)
                        .iter()
                        .any(|(host, _)| vfs_has_file(&host.filesystem, path))
                });
                if !exists {
                    report.warn(
                        &loc,
                        format!("el trigger ReadFile('{}') no existe en ningún VFS", path),
                    );
                }
            }
            CampaignAchievementTrigger::CompleteMission(id) => {
                if !mission_ids.contains(id.as_str()) {
                    report.error(
                        &loc,
                        format!(
                            "el trigger CompleteMission('{}') referencia una misión inexistente",
                            id
                        ),
                    );
                }
            }
            CampaignAchievementTrigger::ChooseEnding { mission, choice } => {
                match campaign.missions.iter().find(|m| &m.id == mission) {
                    None => report.error(
                        &loc,
                        format!(
                            "el trigger ChooseEnding referencia la misión inexistente '{}'",
                            mission
                        ),
                    ),
                    Some(m) => {
                        if *choice < 1 || *choice > m.endings.len() {
                            report.error(
                                &loc,
                                format!(
                                    "ChooseEnding(mission: '{}', choice: {}) fuera de rango: la misión tiene {} finales",
                                    mission,
                                    choice,
                                    m.endings.len()
                                ),
                            );
                        }
                    }
                }
            }
            CampaignAchievementTrigger::CampaignComplete => {}
        }
    }

    // -------------------- Colisiones de verbos (easter eggs) --------------------
    let reserved: HashSet<&str> = reserved_verbs.iter().copied().collect();
    for egg in &campaign.easter_eggs {
        if egg.triggers.is_empty() {
            report.warn("easter egg", "no define ningún trigger");
        }
        for t in &egg.triggers {
            if reserved.contains(t.as_str()) {
                report.warn(
                    "easter egg",
                    format!(
                        "el trigger '{}' colisiona con un comando built-in y quedará oculto",
                        t
                    ),
                );
            }
        }
    }

    // ------------------ Comandos declarativos de campaña ------------------
    // Flags que ALGÚN comando activa (para detectar condiciones imposibles).
    let mut settable_flags: HashSet<&str> = HashSet::new();
    for cmd in &campaign.commands {
        for e in &cmd.effects {
            if let CommandEffect::SetFlag(f) = e {
                settable_flags.insert(f.as_str());
            }
        }
    }
    let achievement_ids: HashSet<&str> = campaign
        .achievements
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    let egg_triggers: HashSet<&str> = campaign
        .easter_eggs
        .iter()
        .flat_map(|e| e.triggers.iter().map(String::as_str))
        .collect();
    let command_triggers: HashSet<&str> = campaign
        .commands
        .iter()
        .flat_map(|c| c.triggers.iter().map(String::as_str))
        .collect();

    let mut seen_cmd_triggers: HashSet<&str> = HashSet::new();
    for cmd in &campaign.commands {
        let label = cmd.triggers.first().cloned().unwrap_or_default();
        let loc = format!("comando '{label}'");
        if cmd.triggers.is_empty() {
            report.warn("comando", "no define ningún trigger");
        }
        for t in &cmd.triggers {
            if reserved.contains(t.as_str()) {
                report.warn(
                    &loc,
                    format!(
                        "el trigger '{}' colisiona con un comando built-in y quedará oculto",
                        t
                    ),
                );
            }
            if egg_triggers.contains(t.as_str()) {
                report.warn(
                    &loc,
                    format!("el trigger '{}' también es un easter egg; el comando declarativo tiene prioridad", t),
                );
            }
            if !seen_cmd_triggers.insert(t.as_str()) {
                report.warn(
                    &loc,
                    format!(
                        "el trigger '{}' está definido en más de un comando declarativo",
                        t
                    ),
                );
            }
        }

        for e in &cmd.effects {
            if let CommandEffect::UnlockAchievement(id) = e {
                if !achievement_ids.contains(id.as_str()) {
                    report.error(
                        &loc,
                        format!(
                            "UnlockAchievement('{}') referencia un logro inexistente",
                            id
                        ),
                    );
                }
            }
            if let CommandEffect::AddMeter(id, _) = e {
                if !declared_meters.contains(id.as_str()) {
                    report.error(
                        &loc,
                        format!(
                            "AddMeter('{}') referencia un medidor no declarado en ninguna misión",
                            id
                        ),
                    );
                }
            }
            if let CommandEffect::ReachStage(name) = e {
                if !campaign.stages.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                    report.error(
                        &loc,
                        format!(
                            "ReachStage('{}') no coincide con ninguna etapa declarada de la campaña",
                            name
                        ),
                    );
                }
            }
        }

        for c in &cmd.conditions {
            match c {
                CommandCondition::Mission(id) => {
                    if !mission_ids.contains(id.as_str()) {
                        report.error(
                            &loc,
                            format!(
                                "la condición Mission('{}') referencia una misión inexistente",
                                id
                            ),
                        );
                    }
                }
                CommandCondition::Phase(p) => {
                    let valid = campaign.stages.iter().any(|s| s.eq_ignore_ascii_case(p));
                    if !valid {
                        report.error(
                            &loc,
                            format!(
                                "la condición Phase('{}') no coincide con ninguna etapa declarada de la campaña",
                                p
                            ),
                        );
                    }
                }
                CommandCondition::FlagSet(f) => {
                    if !settable_flags.contains(f.as_str()) {
                        report.warn(
                            &loc,
                            format!("la condición FlagSet('{}') nunca se cumple: ningún comando activa esa flag", f),
                        );
                    }
                }
                CommandCondition::FlagNotSet(_) => {}
            }
        }
    }

    // ------------------ Entorno y comandos de terminal autorados ------------------
    // Variables de entorno que el motor deriva siempre (no hace falta declararlas).
    const DERIVED_ENV: &[&str] = &["USER", "LOGNAME", "HOME", "PWD", "SHELL", "HOSTNAME"];
    let known_env: HashSet<&str> = campaign
        .env
        .keys()
        .map(String::as_str)
        .chain(DERIVED_ENV.iter().copied())
        .collect();
    for key in campaign.env.keys() {
        if key.trim().is_empty() {
            report.warn("env", "hay una variable de entorno con nombre vacío");
        }
    }

    let mut seen_term_triggers: HashSet<&str> = HashSet::new();
    for cmd in &campaign.terminal {
        let label = cmd.triggers.first().cloned().unwrap_or_default();
        let loc = format!("terminal '{label}'");
        if cmd.triggers.is_empty() {
            report.warn("terminal", "no define ningún trigger");
        }
        for t in &cmd.triggers {
            if reserved.contains(t.as_str()) {
                report.warn(
                    &loc,
                    format!(
                        "el trigger '{}' colisiona con un comando built-in y quedará oculto",
                        t
                    ),
                );
            }
            if command_triggers.contains(t.as_str()) {
                report.warn(
                    &loc,
                    format!(
                        "el trigger '{}' también es un comando declarativo, que tiene prioridad",
                        t
                    ),
                );
            }
            if !seen_term_triggers.insert(t.as_str()) {
                report.warn(
                    &loc,
                    format!(
                        "el trigger '{}' está definido en más de un comando de terminal",
                        t
                    ),
                );
            }
        }
        if cmd.exit < 0 || cmd.exit > 255 {
            report.warn(&loc, format!("exit {} fuera de rango [0, 255]", cmd.exit));
        }
        // Referencias `{env:NOMBRE}` colgantes en la salida (default y por-argumento).
        let outputs = cmd
            .output
            .iter()
            .chain(cmd.args.iter().flat_map(|(_, v)| v.iter()));
        for line in outputs {
            for name in env_refs(line) {
                if !known_env.contains(name.as_str()) {
                    report.warn(
                        &loc,
                        format!(
                            "la plantilla usa {{env:{name}}}, que no existe en env ni es derivada"
                        ),
                    );
                }
            }
        }
    }

    report
}

/// Extrae los nombres referenciados como `{env:NOMBRE}` en una línea de plantilla.
fn env_refs(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("{env:") {
        let after = &rest[start + 5..];
        if let Some(end) = after.find('}') {
            out.push(after[..end].to_string());
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::campaign::{Campaign, CampaignAchievement, CampaignAchievementTrigger};
    use crate::model::command::{CampaignCommand, CommandCondition, CommandEffect};
    use crate::model::filesystem::FsNode;
    use crate::model::language::Language;
    use crate::model::mission::{EntryVector, Mission};
    use crate::model::target::{ExploitReliability, Service, TargetNode, Vulnerability};
    use crate::model::theme::Theme;

    fn file(name: &str) -> FsNode {
        FsNode::File {
            name: name.to_string(),
            content: vec![String::from("x")],
            root: false,
            loot: None,
            binary: None,
            encoding: None,
        }
    }

    fn dir(name: &str, children: Vec<FsNode>) -> FsNode {
        FsNode::Dir {
            name: name.to_string(),
            children,
        }
    }

    fn base_host() -> TargetNode {
        TargetNode {
            hostname: String::from("h.lab"),
            ip: String::from("10.0.0.1"),
            os: String::from("Linux"),
            services: vec![Service {
                port: 80,
                name: String::from("http"),
                version: String::from("v"),
                requires: None,
            }],
            vulnerabilities: vec![Vulnerability {
                id: String::from("V80"),
                name: String::from("a"),
                affected_service: 80,
                difficulty: 4,
                stealth_cost: 5,
                reliability: ExploitReliability::Reliable,
            }],
            filesystem: vec![dir("root", vec![file("flag.txt")])],
            accepts_token: None,
            local_privesc: None,
        }
    }

    fn mission(id: &str, objective: Option<String>, host: TargetNode) -> Mission {
        Mission {
            id: id.to_string(),
            name: id.to_uppercase(),
            briefing: vec![],
            detection_limit: 100.0,
            meters: vec![],
            time_limit: None,
            reactive: false,
            skill: 0.5,
            root_difficulty: 4,
            objective,
            debrief: vec![],
            entry: EntryVector::Active,
            endings: vec![],
            target: host,
            network: vec![],
            music: None,
        }
    }

    fn campaign_with(missions: Vec<Mission>) -> Campaign {
        Campaign {
            name: String::from("T"),
            language: Language::Es,
            intro: vec![],
            stages: crate::model::campaign::default_stages(),
            features: Default::default(),
            theme: Theme::default(),
            easter_eggs: vec![],
            fortunes: vec![],
            signals: vec![],
            achievements: vec![],
            commands: vec![],
            env: std::collections::BTreeMap::new(),
            processes: vec![],
            terminal: vec![],
            missions,
        }
    }

    #[test]
    fn campana_valida_no_produce_errores() {
        let camp = campaign_with(vec![mission(
            "op1",
            Some(String::from("/root/flag.txt")),
            base_host(),
        )]);
        let report = validate_campaign(&camp, &[]);
        assert!(!report.has_errors(), "errores: {:?}", report.errors);
    }

    #[test]
    fn detecta_objetivo_inexistente_e_id_duplicado() {
        let mut camp = campaign_with(vec![
            mission("dup", Some(String::from("/root/flag.txt")), base_host()),
            mission("dup", Some(String::from("/no/existe.txt")), base_host()),
        ]);
        // El id 'dup' está duplicado y el segundo objetivo no existe en el VFS.
        camp.missions[1].id = String::from("dup");
        let report = validate_campaign(&camp, &[]);
        assert!(report.has_errors());
        let joined = report
            .errors
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.contains("duplicado"), "{joined}");
        assert!(joined.contains("objetivo"), "{joined}");
    }

    #[test]
    fn host_generico_sin_servicios_ni_vulns_es_valido() {
        // Un nodo de otro dominio (sin payload de intrusión) no produce errores.
        let mut host = base_host();
        host.services.clear();
        host.vulnerabilities.clear();
        let camp = campaign_with(vec![mission("op1", None, host)]);
        let report = validate_campaign(&camp, &[]);
        assert!(!report.has_errors(), "errores: {:?}", report.errors);
    }

    #[test]
    fn detecta_vuln_sin_servicio_y_token_inobtenible() {
        let mut host = base_host();
        host.vulnerabilities[0].affected_service = 443; // no existe ese puerto
        host.services[0].requires = Some(String::from("token-fantasma"));
        let camp = campaign_with(vec![mission("op1", None, host)]);
        let report = validate_campaign(&camp, &[]);
        let joined = report
            .errors
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.contains("no está en services"), "{joined}");
        assert!(joined.contains("token"), "{joined}");
    }

    #[test]
    fn detecta_colision_de_easter_egg_con_builtin() {
        let mut camp = campaign_with(vec![mission("op1", None, base_host())]);
        camp.easter_eggs.push(crate::model::theme::EasterEgg {
            triggers: vec![String::from("nmap")],
            lines: vec![String::from("no")],
        });
        let report = validate_campaign(&camp, &["nmap", "help", "quit"]);
        assert!(!report.has_errors());
        assert!(report
            .warnings
            .iter()
            .any(|w| w.message.contains("built-in")));
    }

    #[test]
    fn detecta_referencias_colgantes_en_comandos_y_logros() {
        let mut camp = campaign_with(vec![mission("op1", None, base_host())]);
        camp.achievements.push(CampaignAchievement {
            id: String::from("a1"),
            title: String::from("t"),
            description: String::new(),
            trigger: CampaignAchievementTrigger::CompleteMission(String::from("no-existe")),
        });
        camp.commands.push(CampaignCommand {
            triggers: vec![String::from("look")],
            lines: vec![],
            effects: vec![CommandEffect::UnlockAchievement(String::from("no-such"))],
            conditions: vec![
                CommandCondition::Mission(String::from("no-existe")),
                CommandCondition::Phase(String::from("orbit")),
            ],
            hidden: false,
        });
        let report = validate_campaign(&camp, &[]);
        let joined = report
            .errors
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.contains("CompleteMission"), "{joined}");
        assert!(joined.contains("UnlockAchievement"), "{joined}");
        assert!(joined.contains("Mission("), "{joined}");
        assert!(joined.contains("Phase("), "{joined}");
    }

    #[test]
    fn detecta_problemas_de_comandos_terminal() {
        use crate::model::terminal::TerminalCommand;
        let mut camp = campaign_with(vec![mission("op1", None, base_host())]);
        camp.terminal.push(TerminalCommand {
            triggers: vec![String::from("nmap")], // colisiona con built-in
            output: vec![String::from("hola {env:NOPE}")], // env colgante
            args: vec![],
            exit: 999, // fuera de rango
            hidden: false,
        });
        let report = validate_campaign(&camp, &["nmap"]);
        assert!(!report.has_errors());
        let joined = report
            .warnings
            .iter()
            .map(|w| w.message.clone())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.contains("built-in"), "{joined}");
        assert!(joined.contains("env:NOPE"), "{joined}");
        assert!(joined.contains("exit 999"), "{joined}");
    }
}
