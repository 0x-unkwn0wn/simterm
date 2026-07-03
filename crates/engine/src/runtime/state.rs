//! Estado global del juego (runtime) sobre una campaña (definición).
//!
//! - **Definición** (inmutable): la `Campaign` cargada y el `TargetNode` del
//!   nivel activo. No se modifica durante la partida.
//! - **Runtime**: fase de la kill chain, puertos descubiertos, `intel`,
//!   `detection`, `clock`, `logs`, `outcome`... Se reinicia al pasar de nivel
//!   (salvo `extra_skill`, que persiste como progreso del operador).
//!
//! El motor NO carga la campaña aquí: la recibe ya construida (ver
//! [`crate::loader`]). Así el runtime no depende del disco ni de rutas.

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::model::campaign::{Campaign, CampaignAchievement, CampaignAchievementTrigger};
use crate::model::filesystem::{self, Loot, Reward};
use crate::model::intel::{FindingSource, FindingStatus, IntelFinding};
use crate::model::language::EngineText;
use crate::model::meter::OnLimit;
use crate::model::mission::{EntryVector, Mission};
use crate::model::target::TargetNode;
use crate::model::toolbox::{self, ServiceCat};
use crate::runtime::balance;
use crate::runtime::core::CoreState;
use crate::runtime::meter::Meter;
use crate::runtime::probability::clamp01;

// Logros builtin reubicados en el dominio de pentesting (Fase 1). Se re-exportan
// para que `crate::runtime::state::{AchievementId, ACHIEVEMENTS}` sigan resolviendo.
pub use crate::domains::pentest::achievements::{AchievementId, ACHIEVEMENTS};
// La fase de la kill chain es una vista tipada del dominio pentest sobre el
// cursor de etapas genérico (`stage`). Se re-exporta para compatibilidad.
pub use crate::domains::pentest::stage::Phase;

/// Fichero de guardado de progreso de campaña.
const SAVE_PATH: &str = "save.ron";
/// Versión del formato de guardado. Si cambia, los saves viejos se ignoran.
const SAVE_VERSION: u32 = 1;
/// Si es `true`, al cargar cada nivel se permuta la dificultad/ruido entre sus
/// vulnerabilidades reales: cambia cuál es la "vía fácil" en cada partida sin
/// alterar el balance global. Ponlo en `false` para objetivos deterministas.
const SHUFFLE_VULNS: bool = true;

/// Progreso de campaña persistido entre sesiones.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SaveData {
    #[serde(default)]
    version: u32,
    level_index: usize,
    extra_skill: f32,
    creds: Vec<String>,
    campaign_clock: u32,
    #[serde(default)]
    foothold_tokens: Vec<String>,
    #[serde(default)]
    has_wordlist: bool,
    #[serde(default)]
    achievements: Vec<AchievementId>,
    #[serde(default)]
    campaign_achievements: Vec<String>,
    #[serde(default)]
    flags: Vec<String>,
}

/// Estado (definición + runtime) de un host dentro de una red interna. En las
/// misiones de un solo host hay exactamente uno. El host *activo* mantiene su
/// runtime en los campos vivos de `GameState`; los inactivos lo guardan aquí.
#[derive(Debug, Clone)]
pub struct HostSlot {
    pub def: TargetNode,
    pub objective: Option<String>,
    pub links: Vec<String>,
    pub reachable: bool,
    // Snapshot del runtime mientras NO es el host activo.
    pub discovered_ports: Vec<u16>,
    pub stage: usize,
    pub intel: Vec<IntelFinding>,
    pub next_id: usize,
    pub is_root: bool,
    pub privesc_unlocked: bool,
    pub cwd: Vec<String>,
    pub looted_paths: Vec<String>,
    pub cracked_paths: Vec<String>,
    pub solved_paths: Vec<String>,
}

impl HostSlot {
    fn new(
        def: TargetNode,
        objective: Option<String>,
        links: Vec<String>,
        reachable: bool,
    ) -> Self {
        HostSlot {
            def,
            objective,
            links,
            reachable,
            discovered_ports: Vec::new(),
            stage: 0,
            intel: Vec::new(),
            next_id: 1,
            is_root: false,
            privesc_unlocked: false,
            cwd: Vec::new(),
            looted_paths: Vec::new(),
            cracked_paths: Vec::new(),
            solved_paths: Vec::new(),
        }
    }
}

/// Permuta la dificultad/ruido entre las vulnerabilidades reales de un host.
fn shuffle_vulns_of(def: &mut TargetNode) {
    if !SHUFFLE_VULNS || def.vulnerabilities.len() < 2 {
        return;
    }
    let mut pairs: Vec<(u8, u8)> = def
        .vulnerabilities
        .iter()
        .map(|v| (v.difficulty, v.stealth_cost))
        .collect();
    pairs.shuffle(&mut rand::thread_rng());
    for (v, (d, s)) in def.vulnerabilities.iter_mut().zip(pairs) {
        v.difficulty = d;
        v.stealth_cost = s;
    }
    ensure_easy_non_ssh(def);
}

/// Guard de balance: si la vulnerabilidad más fácil cae en un servicio SSH
/// (cuya única herramienta afín, `hydra`, es muy ruidosa), la intercambia con la
/// más fácil de un servicio no-SSH. Así cada host conserva una vía de entrada
/// barata y sigilosa, aunque la dificultad se baraje.
fn ensure_easy_non_ssh(def: &mut TargetNode) {
    let cat_of = |port: u16| -> ServiceCat {
        def.services
            .iter()
            .find(|s| s.port == port)
            .map(|s| toolbox::category(&s.name))
            .unwrap_or(ServiceCat::Other)
    };
    // (índice, dificultad, ¿es SSH?) de cada vulnerabilidad.
    let info: Vec<(usize, u8, bool)> = def
        .vulnerabilities
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                i,
                v.difficulty,
                cat_of(v.affected_service) == ServiceCat::Ssh,
            )
        })
        .collect();

    let easiest = info.iter().min_by_key(|x| x.1).copied();
    if let Some((mi, _, true)) = easiest {
        // La más fácil está en SSH: busca la más fácil en un servicio no-SSH.
        if let Some((ai, _, _)) = info.iter().filter(|x| !x.2).min_by_key(|x| x.1).copied() {
            let a = (
                def.vulnerabilities[mi].difficulty,
                def.vulnerabilities[mi].stealth_cost,
            );
            let b = (
                def.vulnerabilities[ai].difficulty,
                def.vulnerabilities[ai].stealth_cost,
            );
            def.vulnerabilities[mi].difficulty = b.0;
            def.vulnerabilities[mi].stealth_cost = b.1;
            def.vulnerabilities[ai].difficulty = a.0;
            def.vulnerabilities[ai].stealth_cost = a.1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOutcome {
    Victory,
    Defeat,
}

pub struct GameState {
    // ----- Definición -----
    pub campaign: Campaign,
    pub level_index: usize,
    pub target: TargetNode,
    pub detection_limit: f32,
    /// Ventana de tiempo del nivel (ticks). `None` = sin límite de tiempo.
    pub time_limit: Option<u32>,
    /// Defensa activa: el host responde por etapas a la traza.
    pub reactive: bool,
    /// Nº de etapas de contramedidas ya disparadas en este nivel.
    pub defense_stage: u8,
    /// Penalización acumulada a la prob. de `exploit`/`privesc` por la defensa.
    pub defense_penalty: f32,
    /// Habilidad base del nivel (de la misión).
    pub base_skill: f32,
    /// Dificultad de la escalada de privilegios del nivel.
    pub root_difficulty: u8,
    /// Ruta del fichero objetivo a exfiltrar (si la misión lo define).
    pub objective: Option<String>,
    /// Vector de entrada del nivel (cómo se arranca la operación).
    pub entry: EntryVector,
    /// Hosts del nivel (red interna). Los de un solo host tienen longitud 1.
    /// El host activo es `hosts[active]`; su runtime vive en los campos de abajo.
    pub hosts: Vec<HostSlot>,
    /// Índice del host activo dentro de `hosts`.
    pub active: usize,

    // ----- Runtime del nivel (del host ACTIVO) -----
    /// Etapa de progresión actual, como índice en `campaign.stages`. El dominio
    /// pentest la interpreta como su `Phase` (ver `phase()`).
    pub stage: usize,
    /// Solo para entradas `Pivot`: ¿se ha establecido ya el túnel (`connect`)?
    pub pivoted: bool,
    /// Nº de encubrimientos (`cleanup`) hechos en este nivel (riesgo creciente).
    pub cleanups_done: u32,
    /// Puertos ya descubiertos por reconocimiento.
    pub discovered_ports: Vec<u16>,
    pub intel: Vec<IntelFinding>,
    pub detection: Meter,
    pub clock: u32,
    pub next_id: usize,
    /// ¿Se ha conseguido acceso root en este nivel (vía privesc)?
    pub is_root: bool,
    /// Ruta segura: ¿se ha recogido la llave/credencial local de este nivel?
    /// Si es `true`, `privesc` tiene éxito garantizado (sin RNG). Se reinicia
    /// en cada nivel.
    pub privesc_unlocked: bool,
    /// Directorio de trabajo actual en el VFS (componentes; vacío = "/").
    pub cwd: Vec<String>,
    /// Rutas de ficheros cuyo botín ya se ha recogido en este nivel.
    pub looted_paths: Vec<String>,
    /// Rutas de hashes ya rotos con `john` en este nivel (evita re-cracking).
    pub cracked_paths: Vec<String>,
    /// Rutas de binarios ya resueltos con `solve` en este nivel.
    pub solved_paths: Vec<String>,

    // ----- Runtime global (persiste entre niveles) -----
    /// Bonus de habilidad acumulado (botín de ficheros).
    pub extra_skill: f32,
    /// Inventario de credenciales saqueadas a lo largo de la campaña.
    pub creds: Vec<String>,
    /// ¿Se ha saqueado un wordlist (tipo rockyou)? Habilita romper hashes que lo
    /// requieren. Persiste durante toda la campaña.
    pub has_wordlist: bool,
    /// Tokens de credenciales reutilizables (para foothold determinista con
    /// `login` en niveles que los acepten). Persisten entre niveles.
    pub foothold_tokens: Vec<String>,
    pub logs: Vec<String>,
    pub running: bool,
    /// Desenlace de la *campaña*: solo se fija al perder o al completarla.
    pub outcome: Option<GameOutcome>,
    /// Logros desbloqueados durante esta campaña.
    pub achievements: Vec<AchievementId>,
    /// Estado de runtime domain-agnóstico (sesión de shell, medidores de campaña,
    /// bookkeeping persistente...). Núcleo naciente de la separación
    /// núcleo/dominio (sub-paso 5).
    pub core: CoreState,
}

impl GameState {
    pub fn text(&self) -> EngineText {
        self.campaign.language.text()
    }

    /// Construye el estado a partir de una campaña ya cargada. El motor no toca
    /// el disco aquí; la campaña la resuelve [`crate::loader`].
    pub fn new(campaign: Campaign) -> Self {
        let mut state = GameState {
            campaign,
            level_index: 0,
            target: TargetNode::empty(),
            detection_limit: 100.0,
            time_limit: None,
            reactive: false,
            defense_stage: 0,
            defense_penalty: 0.0,
            base_skill: 0.5,
            root_difficulty: 5,
            objective: None,
            entry: EntryVector::Active,
            hosts: Vec::new(),
            active: 0,
            stage: 0,
            pivoted: false,
            cleanups_done: 0,
            discovered_ports: Vec::new(),
            intel: Vec::new(),
            detection: Meter::new(),
            clock: 0,
            next_id: 1,
            is_root: false,
            privesc_unlocked: false,
            cwd: Vec::new(),
            looted_paths: Vec::new(),
            cracked_paths: Vec::new(),
            solved_paths: Vec::new(),
            extra_skill: 0.0,
            creds: Vec::new(),
            has_wordlist: false,
            foothold_tokens: Vec::new(),
            logs: Vec::new(),
            running: true,
            outcome: None,
            achievements: Vec::new(),
            core: CoreState::new(),
        };

        // Carga la primera operación (construye sus hosts y siembra la entrada).
        state.apply_mission(0);

        state.log(format!("=== {} ===", state.campaign.name));
        let intro = state.campaign.intro.clone();
        for line in intro {
            state.log(line);
        }
        state.announce_level();
        state.log(String::from(state.text().help_hint()));
        state
    }

    // ---------- Consultas ----------

    pub fn level_count(&self) -> usize {
        self.campaign.missions.len()
    }

    pub fn level_number(&self) -> usize {
        self.level_index + 1
    }

    pub fn level_name(&self) -> &str {
        &self.campaign.missions[self.level_index].name
    }

    pub fn is_over(&self) -> bool {
        self.outcome.is_some()
    }

    /// Habilidad efectiva del operador (base del nivel + bonus de campaña).
    pub fn effective_skill(&self) -> f32 {
        (self.base_skill + self.extra_skill).clamp(0.0, 0.95)
    }

    // --- Etapas de progresión (API genérica del núcleo) ---

    /// ¿La etapa actual es al menos la de índice `rank`?
    pub fn stage_at_least(&self, rank: usize) -> bool {
        self.stage >= rank
    }

    /// Etiqueta de la etapa actual (de `campaign.stages`).
    pub fn stage_label(&self) -> &str {
        self.stage_label_at(self.stage)
    }

    /// Etiqueta de la etapa de índice `rank` (vacía si está fuera de rango).
    pub fn stage_label_at(&self, rank: usize) -> &str {
        self.campaign
            .stages
            .get(rank)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Índice de la etapa cuyo nombre coincide (sin distinguir mayúsculas).
    pub fn stage_index_of(&self, name: &str) -> Option<usize> {
        self.campaign
            .stages
            .iter()
            .position(|s| s.eq_ignore_ascii_case(name))
    }

    // --- Vista tipada del dominio pentest (transitorio: se reubica en el
    // sub-paso 5, cuando GameState se parta en núcleo + estado de dominio) ---

    /// Etapa actual interpretada como fase de la kill chain.
    pub fn phase(&self) -> Phase {
        Phase::from_rank(self.stage)
    }

    pub fn phase_at_least(&self, p: Phase) -> bool {
        self.stage_at_least(p.rank())
    }

    pub fn has_foothold(&self) -> bool {
        self.stage_at_least(Phase::Post.rank())
    }

    pub fn is_port_discovered(&self, port: u16) -> bool {
        self.discovered_ports.contains(&port)
    }

    /// Ruta actual del VFS como texto ("/", "/etc", ...).
    pub fn cwd_display(&self) -> String {
        filesystem::path_string(&self.cwd)
    }

    /// Prompt de la shell. Antes del foothold es la consola del operador (texto
    /// definido por la campaña en su `theme`); después refleja el host
    /// comprometido, el usuario y el directorio actual.
    pub fn prompt(&self) -> String {
        if !self.has_foothold() {
            return self.campaign.theme.operator_prompt.clone();
        }
        let host = self
            .target
            .hostname
            .split('.')
            .next()
            .unwrap_or(&self.target.hostname);
        let user = if self.is_root { "root" } else { "user" };
        let sym = if self.is_root { '#' } else { '$' };
        format!("{user}@{host}:{}{} ", self.cwd_display(), sym)
    }

    /// ¿La ruta (ya normalizada) es el objetivo del nivel?
    pub fn is_objective(&self, comps: &[String]) -> bool {
        match &self.objective {
            Some(obj) => filesystem::normalize(&[], obj) == comps,
            None => false,
        }
    }

    /// Aplica el botín de un fichero la primera vez que se lee. Devuelve true
    /// si era nuevo (para que el llamante informe del efecto).
    pub fn apply_loot(&mut self, path: &str, loot: &Loot) -> bool {
        if self.looted_paths.iter().any(|p| p == path) {
            return false;
        }
        self.looted_paths.push(path.to_string());
        if loot.skill > 0.0 {
            self.extra_skill = (self.extra_skill + loot.skill).min(0.30);
        }
        if let Some(c) = &loot.credential {
            if !self.creds.contains(c) {
                self.creds.push(c.clone());
            }
        }
        // Ruta segura: la llave local del nivel habilita la escalada determinista.
        if loot.privesc_key {
            self.privesc_unlocked = true;
        }
        // Credencial reutilizable: se guarda para un posible foothold posterior.
        if let Some(tok) = &loot.foothold_token {
            if !self.foothold_tokens.contains(tok) {
                self.foothold_tokens.push(tok.clone());
            }
        }
        // Wordlist: habilita romper hashes que lo requieren (persiste).
        if loot.wordlist {
            self.has_wordlist = true;
        }
        self.unlock_achievement(AchievementId::FirstLoot);
        true
    }

    /// Aplica una recompensa de `john` (hash roto) o `solve` (binario resuelto).
    /// Reusa los mismos canales que el botín normal.
    pub fn apply_reward(&mut self, reward: &Reward) {
        match reward {
            Reward::Skill(s) => {
                self.extra_skill = (self.extra_skill + s).min(0.30);
            }
            Reward::Credential(c) => {
                if !self.creds.contains(c) {
                    self.creds.push(c.clone());
                }
            }
            Reward::Token(t) => {
                if !self.foothold_tokens.contains(t) {
                    self.foothold_tokens.push(t.clone());
                }
            }
            Reward::PrivescKey => {
                self.privesc_unlocked = true;
            }
        }
    }

    // ---------- Mutaciones de runtime ----------

    pub fn log(&mut self, msg: String) {
        self.logs.push(format!("[t={:>3}] {}", self.clock, msg));
    }

    /// ¿Está activa la flag de campaña indicada?
    pub fn has_flag(&self, name: &str) -> bool {
        self.core.flags.iter().any(|f| f == name)
    }

    /// Activa una flag de campaña. Devuelve `true` si era nueva.
    pub fn set_flag(&mut self, name: &str) -> bool {
        if self.has_flag(name) {
            return false;
        }
        self.core.flags.push(name.to_string());
        true
    }

    /// Desactiva una flag de campaña. Devuelve `true` si existía.
    pub fn clear_flag(&mut self, name: &str) -> bool {
        if let Some(i) = self.core.flags.iter().position(|f| f == name) {
            self.core.flags.remove(i);
            true
        } else {
            false
        }
    }

    pub fn unlock_achievement(&mut self, id: AchievementId) -> bool {
        if self.achievements.contains(&id) {
            return false;
        }
        self.achievements.push(id);
        let text = self.text();
        self.log(text.achievement_unlocked(id.title_in(text.language())));
        self.log(format!("   {}", id.description_in(text.language())));
        true
    }

    pub fn unlock_campaign_achievement(&mut self, id: &str) -> bool {
        if self.core.campaign_achievements.iter().any(|got| got == id) {
            return false;
        }
        let Some(achievement) = self
            .campaign
            .achievements
            .iter()
            .find(|a| a.id == id)
            .cloned()
        else {
            return false;
        };
        self.core.campaign_achievements.push(id.to_string());
        self.log(self.text().achievement_unlocked(&achievement.title));
        if !achievement.description.is_empty() {
            self.log(format!("   {}", achievement.description));
        }
        true
    }

    fn matching_campaign_achievements(
        &self,
        predicate: impl Fn(&CampaignAchievementTrigger) -> bool,
    ) -> Vec<CampaignAchievement> {
        self.campaign
            .achievements
            .iter()
            .filter(|a| !self.core.campaign_achievements.iter().any(|got| got == &a.id))
            .filter(|a| predicate(&a.trigger))
            .cloned()
            .collect()
    }

    pub fn unlock_campaign_read_file(&mut self, path: &str) {
        for achievement in self.matching_campaign_achievements(
            |trigger| matches!(trigger, CampaignAchievementTrigger::ReadFile(p) if p == path),
        ) {
            self.unlock_campaign_achievement(&achievement.id);
        }
    }

    fn unlock_campaign_complete_mission(&mut self, mission_id: &str) {
        for achievement in self.matching_campaign_achievements(|trigger| {
            matches!(trigger, CampaignAchievementTrigger::CompleteMission(id) if id == mission_id)
        }) {
            self.unlock_campaign_achievement(&achievement.id);
        }
    }

    fn unlock_campaign_choose_ending(&mut self, mission_id: &str, choice: usize) {
        for achievement in self.matching_campaign_achievements(|trigger| {
            matches!(
                trigger,
                CampaignAchievementTrigger::ChooseEnding { mission, choice: c }
                    if mission == mission_id && *c == choice
            )
        }) {
            self.unlock_campaign_achievement(&achievement.id);
        }
    }

    fn unlock_campaign_complete_campaign(&mut self) {
        for achievement in self.matching_campaign_achievements(|trigger| {
            matches!(trigger, CampaignAchievementTrigger::CampaignComplete)
        }) {
            self.unlock_campaign_achievement(&achievement.id);
        }
    }

    pub fn advance_clock(&mut self, ticks: u32) {
        self.clock += ticks;
        // Dwell: durante las fases activas (RECON/ENUM/EXPLOIT), el tiempo en el
        // sistema sube la traza poco a poco (penaliza demorarse). En POST, con
        // shell ya conseguida, la exploración del VFS es gratis.
        if !self.has_foothold() {
            self.detection.add_passive(ticks as f32 * balance::DWELL_RATE);
        }
        self.apply_meter_drift(ticks);
        self.check_time();
        self.check_meters();
    }

    /// Ticks restantes de la ventana de la operación (`None` = sin límite).
    pub fn time_remaining(&self) -> Option<u32> {
        self.time_limit.map(|lim| lim.saturating_sub(self.clock))
    }

    /// Comprueba la derrota por ventana de tiempo agotada. La invoca
    /// `advance_clock`, de modo que se evalúa tras cada acción que gasta reloj.
    pub fn check_time(&mut self) {
        if self.outcome.is_none() {
            if let Some(limit) = self.time_limit {
                if self.clock >= limit {
                    self.outcome = Some(GameOutcome::Defeat);
                    self.log(String::from(self.text().time_window_closed()));
                }
            }
        }
    }

    // ---------- Medidores de campaña (genéricos) ----------

    /// Valor vivo de un medidor de campaña por su id.
    pub fn meter(&self, id: &str) -> Option<&Meter> {
        self.core.meters.get(id)
    }

    /// Modifica un medidor de campaña (positivo suma, negativo resta) y evalúa su
    /// `on_limit`. No hace nada si el id no existe.
    pub fn add_meter(&mut self, id: &str, delta: f32) {
        match self.core.meters.get_mut(id) {
            Some(m) => {
                if delta >= 0.0 {
                    m.add_passive(delta);
                } else {
                    m.reduce(-delta);
                }
            }
            None => return,
        }
        self.check_meters();
    }

    /// Aplica la deriva por tiempo (`per_tick`) de cada medidor de campaña.
    fn apply_meter_drift(&mut self, ticks: u32) {
        for i in 0..self.core.meter_defs.len() {
            let per = self.core.meter_defs[i].per_tick;
            if per == 0.0 {
                continue;
            }
            let id = self.core.meter_defs[i].id.clone();
            let delta = per * ticks as f32;
            if let Some(m) = self.core.meters.get_mut(&id) {
                if delta >= 0.0 {
                    m.add_passive(delta);
                } else {
                    m.reduce(-delta);
                }
            }
        }
    }

    /// Evalúa los medidores de campaña: si alguno cruza su umbral con `Fail`
    /// pierde el nivel; con `Win` lo completa. `None` no tiene efecto.
    fn check_meters(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        let mut failed: Option<String> = None;
        let mut won = false;
        for def in &self.core.meter_defs {
            if let Some(m) = self.core.meters.get(&def.id) {
                if def.triggered(m.value) {
                    match def.on_limit {
                        OnLimit::Fail => {
                            failed = Some(def.label().to_string());
                            break;
                        }
                        OnLimit::Win => won = true,
                        OnLimit::None => {}
                    }
                }
            }
        }
        if let Some(label) = failed {
            self.outcome = Some(GameOutcome::Defeat);
            self.log(format!("!! {label} en el límite — OPERACIÓN FALLIDA."));
        } else if won {
            self.complete_level();
        }
    }

    /// Avanza a la etapa `rank` si es posterior a la actual (nunca retrocede).
    pub fn reach_stage(&mut self, rank: usize) {
        if rank > self.stage {
            self.stage = rank;
            let label = self.stage_label_at(rank).to_string();
            self.log(self.text().phase_reached(&label));
        }
    }

    /// Avanza la fase de la kill chain (envoltorio pentest de `reach_stage`).
    pub fn reach_phase(&mut self, p: Phase) {
        self.reach_stage(p.rank());
    }

    pub fn discover_port(&mut self, port: u16) -> bool {
        if self.discovered_ports.contains(&port) {
            false
        } else {
            self.discovered_ports.push(port);
            true
        }
    }

    /// Crea un hallazgo y devuelve su id público.
    pub fn push_finding(
        &mut self,
        title: String,
        confidence: f32,
        source: FindingSource,
        real_vuln_id: Option<String>,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.intel.push(IntelFinding {
            public_id: id,
            title,
            target_node: self.target.hostname.clone(),
            confidence: clamp01(confidence),
            status: FindingStatus::Unverified,
            source,
            real_vuln_id,
            verify_pos: 0,
            verify_neg: 0,
        });
        id
    }

    pub fn has_finding_for(&self, vuln_id: &str) -> bool {
        self.intel
            .iter()
            .any(|f| f.real_vuln_id.as_deref() == Some(vuln_id))
    }

    pub fn find(&self, public_id: usize) -> Option<&IntelFinding> {
        self.intel.iter().find(|f| f.public_id == public_id)
    }

    pub fn find_mut(&mut self, public_id: usize) -> Option<&mut IntelFinding> {
        self.intel.iter_mut().find(|f| f.public_id == public_id)
    }

    /// Comprueba la derrota por detección. Debe llamarse tras cada acción ruidosa.
    pub fn check_detection(&mut self) {
        // En hosts con defensa activa, el equipo azul reacciona ANTES de evaluar
        // la derrota (su respuesta puede inyectar ruido que la precipite).
        self.escalate_defense();
        if self.outcome.is_none() && self.detection.reached(self.detection_limit) {
            self.outcome = Some(GameOutcome::Defeat);
            self.log(String::from(
                "!! TRAZA AL LÍMITE — Han identificado el origen de la conexión. OPERACIÓN ABORTADA.",
            ));
        }
    }

    /// Defensa activa (blue team): dispara las etapas de contramedidas cuyo
    /// umbral de traza se haya alcanzado. Cada etapa endurece el sistema
    /// (penaliza `exploit`/`privesc`) y puede inyectar ruido. Es acumulativo y
    /// no reversible dentro del nivel: una vez te ven, no te dejan de buscar.
    /// Los textos de respuesta los define la campaña en su `theme`.
    fn escalate_defense(&mut self) {
        if !self.reactive || self.outcome.is_some() {
            return;
        }
        // Se recalcula la traza en cada iteración: el ruido de respuesta de una
        // etapa puede encadenar la siguiente (un cerco que se cierra de golpe).
        while (self.defense_stage as usize) < balance::DEFENSE_STAGES.len() {
            let (threshold, penalty, response_noise) =
                balance::DEFENSE_STAGES[self.defense_stage as usize];
            if self.detection.ratio(self.detection_limit) < threshold {
                break;
            }
            self.defense_stage += 1;
            self.defense_penalty += penalty;
            let msg = self
                .campaign
                .theme
                .defense_message(self.defense_stage)
                .to_string();
            self.log(msg);
            if response_noise > 0.0 {
                self.detection.add(response_noise);
            }
        }
    }

    // ---------- Progresión de niveles ----------

    /// Aplica la definición del nivel `index` y reinicia el runtime del nivel.
    /// `extra_skill` (progreso de campaña) NO se reinicia.
    pub fn apply_mission(&mut self, index: usize) {
        let m = self.campaign.missions[index].clone();
        self.level_index = index;
        self.detection_limit = m.detection_limit;
        self.time_limit = m.time_limit;
        self.reactive = m.reactive;
        self.defense_stage = 0;
        self.defense_penalty = 0.0;
        self.base_skill = m.skill;
        self.root_difficulty = m.root_difficulty;
        self.entry = m.entry.clone();

        // Runtime de nivel (global al nivel, no por host).
        self.detection = Meter::new();
        // Medidores de campaña del nivel: se arrancan en su valor inicial.
        self.core.meter_defs = m.meters.clone();
        self.core.meters = m
            .meters
            .iter()
            .map(|d| (d.id.clone(), Meter::starting(d.start)))
            .collect();
        self.clock = 0;
        self.cleanups_done = 0;
        self.pivoted = false;
        self.core.awaiting_choice = false;
        self.core.epilogue = None;
        // El entorno de sesión y el último código de salida son del host actual.
        self.core.env_session.clear();
        self.core.last_exit = 0;

        // Construye los hosts del nivel y carga el de entrada en los campos vivos.
        self.build_hosts(&m);
        self.setup_entry();
    }

    /// Construye `hosts` a partir de la misión (uno solo o una red) y activa el
    /// host de entrada, cargando su runtime (vacío) en los campos vivos.
    fn build_hosts(&mut self, m: &Mission) {
        let mut slots: Vec<HostSlot> = if m.network.is_empty() {
            let mut def = m.target.clone();
            shuffle_vulns_of(&mut def);
            vec![HostSlot::new(def, m.objective.clone(), Vec::new(), true)]
        } else {
            m.network
                .iter()
                .map(|nh| {
                    let mut def = nh.target.clone();
                    shuffle_vulns_of(&mut def);
                    HostSlot::new(def, nh.objective.clone(), nh.links.clone(), nh.entry)
                })
                .collect()
        };
        // Si ninguno está marcado como entrada, el primero lo es.
        if !slots.iter().any(|h| h.reachable) {
            if let Some(first) = slots.first_mut() {
                first.reachable = true;
            }
        }
        self.active = slots.iter().position(|h| h.reachable).unwrap_or(0);
        self.hosts = slots;
        self.load_active();
    }

    /// Vuelca el runtime guardado del host activo a los campos vivos.
    fn load_active(&mut self) {
        let a = self.active;
        self.target = self.hosts[a].def.clone();
        self.objective = self.hosts[a].objective.clone();
        self.stage = self.hosts[a].stage;
        self.discovered_ports = std::mem::take(&mut self.hosts[a].discovered_ports);
        self.intel = std::mem::take(&mut self.hosts[a].intel);
        self.next_id = self.hosts[a].next_id;
        self.is_root = self.hosts[a].is_root;
        self.privesc_unlocked = self.hosts[a].privesc_unlocked;
        self.cwd = std::mem::take(&mut self.hosts[a].cwd);
        self.looted_paths = std::mem::take(&mut self.hosts[a].looted_paths);
        self.cracked_paths = std::mem::take(&mut self.hosts[a].cracked_paths);
        self.solved_paths = std::mem::take(&mut self.hosts[a].solved_paths);
    }

    /// Guarda el runtime vivo en el slot del host activo (antes de pivotar).
    fn snapshot_active(&mut self) {
        let a = self.active;
        self.hosts[a].stage = self.stage;
        self.hosts[a].discovered_ports = std::mem::take(&mut self.discovered_ports);
        self.hosts[a].intel = std::mem::take(&mut self.intel);
        self.hosts[a].next_id = self.next_id;
        self.hosts[a].is_root = self.is_root;
        self.hosts[a].privesc_unlocked = self.privesc_unlocked;
        self.hosts[a].cwd = std::mem::take(&mut self.cwd);
        self.hosts[a].looted_paths = std::mem::take(&mut self.looted_paths);
        self.hosts[a].cracked_paths = std::mem::take(&mut self.cracked_paths);
        self.hosts[a].solved_paths = std::mem::take(&mut self.solved_paths);
    }

    pub fn is_single_host(&self) -> bool {
        self.hosts.len() <= 1
    }

    /// Localiza un host por nombre corto o FQDN.
    pub fn host_index(&self, name: &str) -> Option<usize> {
        self.hosts
            .iter()
            .position(|h| h.def.hostname == name || h.def.short_name() == name)
    }

    /// Pivota al host `index` (intercambiando el runtime activo). No valida la
    /// alcanzabilidad; eso lo hace la acción `pivot`.
    pub fn pivot_to(&mut self, index: usize) {
        if index == self.active || index >= self.hosts.len() {
            return;
        }
        self.snapshot_active();
        self.active = index;
        self.load_active();
        self.unlock_achievement(AchievementId::FirstPivot);
    }

    /// Marca como alcanzables los vecinos del host activo (descubrimiento de red
    /// interna). Devuelve los nombres de los hosts revelados nuevos.
    pub fn reveal_neighbors(&mut self) -> Vec<String> {
        let links = self.hosts[self.active].links.clone();
        let mut revealed = Vec::new();
        for name in &links {
            if let Some(i) = self.host_index(name) {
                if !self.hosts[i].reachable {
                    self.hosts[i].reachable = true;
                    revealed.push(self.hosts[i].def.hostname.clone());
                }
            }
        }
        revealed
    }

    /// Resumen de la red para la UI: (nombre corto, marcador, ¿activo?).
    /// Marcadores: `*` activo · `#` comprometido · `+` alcanzable · `·` oculto.
    pub fn network_overview(&self) -> Vec<(String, char, bool)> {
        self.hosts
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let active = i == self.active;
                let post = Phase::Post.rank();
                let compromised = if active {
                    self.stage_at_least(post)
                } else {
                    h.stage >= post
                };
                let mark = if active {
                    '*'
                } else if compromised {
                    '#'
                } else if h.reachable {
                    '+'
                } else {
                    '·'
                };
                (h.def.short_name().to_string(), mark, active)
            })
            .collect()
    }

    /// Aplica el vector de entrada del nivel: siembra el estado inicial según
    /// cómo arranque la operación (frío, pasivo, pivote o escaneo activo).
    fn setup_entry(&mut self) {
        self.pivoted = false;
        if let EntryVector::Cold { ports } = &self.entry {
            // El cliente ya señaló servicios: se empieza en ENUM con ellos.
            let seeds: Vec<u16> = if ports.is_empty() {
                self.target.services.iter().map(|s| s.port).collect()
            } else {
                ports.clone()
            };
            for p in seeds {
                if !self.discovered_ports.contains(&p) {
                    self.discovered_ports.push(p);
                }
            }
            if !self.discovered_ports.is_empty() {
                self.stage = Phase::Enum.rank();
            }
        }
    }

    fn announce_level(&mut self) {
        let n = self.level_number();
        let total = self.level_count();
        let m = self.campaign.missions[self.level_index].clone();
        let limit = self.detection_limit;

        let text = self.text();
        self.log(text.mission_header(n, total, &m.name));
        // En misiones multi-host el `target` raíz va vacío: la cabecera muestra el
        // host de entrada (el primero marcado `entry`, o el primero de la red).
        let (head, multi) = if m.network.is_empty() {
            (&m.target, false)
        } else {
            let h = m.network.iter().find(|h| h.entry).unwrap_or(&m.network[0]);
            (&h.target, true)
        };
        self.log(text.target_header(&head.hostname, &head.ip, &head.os, multi));
        for line in m.briefing {
            self.log(line);
        }
        let hint = match &m.entry {
            EntryVector::Active => text.entry_hint_active(),
            EntryVector::Cold { .. } => text.entry_hint_cold(),
            EntryVector::Passive => text.entry_hint_passive(),
            EntryVector::Pivot { .. } => text.entry_hint_pivot(),
        };
        self.log(text.trace_hint(limit, hint));
    }

    /// Foothold conseguido: pasa a fase POST. El nivel se completa con privesc.
    pub fn gain_foothold(&mut self) {
        self.reach_phase(Phase::Post);
        self.unlock_achievement(AchievementId::FirstFoothold);
    }

    /// Nivel superado (tras privesc): avanza al siguiente, abre la decisión
    /// final o gana la campaña.
    pub fn complete_level(&mut self) {
        if self.outcome.is_some() || self.core.awaiting_choice {
            return;
        }
        self.log(String::from(self.text().level_completed()));
        let completed_mission_id = self.campaign.missions[self.level_index].id.clone();
        self.unlock_campaign_complete_mission(&completed_mission_id);

        // Resumen del nivel: grado de sigilo según la traza dejada (del tema).
        let ratio = self.detection.ratio(self.detection_limit);
        let stealth_unlocked =
            ratio < 0.25 && self.unlock_achievement(AchievementId::StealthOperation);
        let g = self.campaign.theme.grade(ratio).to_string();
        self.core.last_summary = Some(self.text().level_summary(
            self.level_number(),
            &g,
            self.detection.value,
            self.detection_limit,
            self.clock,
        ));
        self.core.campaign_clock += self.clock;

        // Debrief (lore de cierre) de la misión recién superada.
        let debrief = self.campaign.missions[self.level_index].debrief.clone();
        for line in debrief {
            self.log(line);
        }

        let next = self.level_index + 1;
        if next < self.level_count() {
            self.apply_mission(next);
            self.save(); // punto de reanudación = nueva operación
                         // Consola limpia para la nueva operación: el cierre del nivel anterior
                         // ya se mostró (y queda en el overlay de debrief). Transición más nítida.
            self.logs.clear();
            self.announce_level();
            if stealth_unlocked {
                let text = self.text();
                self.log(text.achievement_unlocked(
                    AchievementId::StealthOperation.title_in(text.language()),
                ));
                self.log(format!(
                    "   {}",
                    AchievementId::StealthOperation.description_in(text.language())
                ));
            }
            return;
        }

        // Última operación. Si define desenlaces, abre la decisión final.
        let endings = self.campaign.missions[self.level_index].endings.clone();
        if endings.is_empty() {
            self.finalize_victory();
        } else {
            self.core.awaiting_choice = true;
            self.log(String::from(self.text().final_choice_prompt()));
            for (i, e) in endings.iter().enumerate() {
                self.log(format!("  {}. {}", i + 1, e.title));
            }
            self.log(String::from(self.text().choose_hint()));
        }
    }

    /// Resuelve el final con elección: muestra el epílogo y cierra la campaña.
    pub fn resolve_ending(&mut self, choice: usize) {
        if !self.core.awaiting_choice {
            self.log(String::from(self.text().no_pending_choice()));
            return;
        }
        let endings = self.campaign.missions[self.level_index].endings.clone();
        let e = match endings.get(choice) {
            Some(e) => e.clone(),
            None => {
                self.log(self.text().invalid_choice(endings.len()));
                return;
            }
        };

        self.core.awaiting_choice = false;
        self.log(format!("// {} //", e.title));
        for line in &e.lines {
            self.log(line.clone());
        }
        let mission_id = self.campaign.missions[self.level_index].id.clone();
        self.unlock_campaign_choose_ending(&mission_id, choice + 1);
        // El epílogo se muestra también en el overlay de cierre de campaña.
        let mut epi = vec![e.title.clone(), String::new()];
        epi.extend(e.lines.clone());
        self.core.epilogue = Some(epi);

        self.finalize_victory();
    }

    /// Cierra la campaña como victoria: borra el guardado y registra el resumen.
    fn finalize_victory(&mut self) {
        self.unlock_achievement(AchievementId::CampaignComplete);
        self.unlock_campaign_complete_campaign();
        self.outcome = Some(GameOutcome::Victory);
        self.delete_save(); // campaña terminada: sin punto de reanudación
        self.log(String::from(self.text().campaign_completed()));
        self.log(
            self.text()
                .campaign_summary(self.level_count(), self.core.campaign_clock),
        );
    }

    // ---------- Persistencia ----------

    fn save(&self) {
        if cfg!(test) {
            return; // los tests no tocan el disco
        }
        let data = SaveData {
            version: SAVE_VERSION,
            level_index: self.level_index,
            extra_skill: self.extra_skill,
            creds: self.creds.clone(),
            campaign_clock: self.core.campaign_clock,
            foothold_tokens: self.foothold_tokens.clone(),
            has_wordlist: self.has_wordlist,
            achievements: self.achievements.clone(),
            campaign_achievements: self.core.campaign_achievements.clone(),
            flags: self.core.flags.clone(),
        };
        if let Ok(text) = ron::ser::to_string(&data) {
            let _ = std::fs::write(SAVE_PATH, text); // best-effort
        }
    }

    fn delete_save(&self) {
        if cfg!(test) {
            return;
        }
        let _ = std::fs::remove_file(SAVE_PATH);
    }

    fn load_save() -> Option<SaveData> {
        let text = std::fs::read_to_string(SAVE_PATH).ok()?;
        ron::de::from_str(&text).ok()
    }

    /// Si hay una partida guardada (en un nivel posterior al primero), reanuda
    /// la campaña en ese punto. Lo invoca el frontend (no `new`), para no afectar
    /// a los tests, que construyen un estado limpio.
    pub fn try_resume(&mut self) {
        if let Some(sd) = Self::load_save() {
            // Ignora saves de un formato distinto o fuera de rango (p. ej. tras
            // editar la campaña): mejor empezar limpio que reanudar un estado roto.
            if sd.version == SAVE_VERSION
                && sd.level_index > 0
                && sd.level_index < self.level_count()
            {
                self.extra_skill = sd.extra_skill;
                self.creds = sd.creds;
                self.core.campaign_clock = sd.campaign_clock;
                self.foothold_tokens = sd.foothold_tokens;
                self.has_wordlist = sd.has_wordlist;
                self.achievements = sd.achievements;
                self.core.campaign_achievements = sd.campaign_achievements;
                self.core.flags = sd.flags;
                self.apply_mission(sd.level_index);
                self.logs.clear();
                self.log(format!("=== {} ===", self.campaign.name));
                self.log(self.text().resumed(self.level_number(), self.level_count()));
                self.announce_level();
                self.log(String::from(self.text().reset_hint()));
            }
        }
    }

    /// Borra el guardado y reinicia la campaña desde la primera operación.
    pub fn reset_campaign(&mut self) {
        self.delete_save();
        self.extra_skill = 0.0;
        self.creds.clear();
        self.has_wordlist = false;
        self.foothold_tokens.clear();
        self.achievements.clear();
        self.core.campaign_achievements.clear();
        self.core.flags.clear();
        self.core.campaign_clock = 0;
        self.core.last_summary = None;
        self.outcome = None;
        self.running = true;
        self.apply_mission(0);
        self.logs.clear();
        self.log(String::from(self.text().reset_done()));
        self.announce_level();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::target::{ExploitReliability, Service, Vulnerability};

    #[test]
    fn guard_mueve_la_vuln_facil_fuera_de_ssh() {
        let mut def = TargetNode {
            hostname: String::from("h"),
            ip: String::from("1"),
            os: String::from("x"),
            services: vec![
                Service {
                    port: 22,
                    name: String::from("ssh"),
                    version: String::new(),
                    requires: None,
                },
                Service {
                    port: 80,
                    name: String::from("http"),
                    version: String::new(),
                    requires: None,
                },
            ],
            vulnerabilities: vec![
                Vulnerability {
                    id: String::from("A"),
                    name: String::new(),
                    affected_service: 22,
                    difficulty: 3,
                    stealth_cost: 5,
                    reliability: ExploitReliability::Reliable,
                },
                Vulnerability {
                    id: String::from("B"),
                    name: String::new(),
                    affected_service: 80,
                    difficulty: 7,
                    stealth_cost: 9,
                    reliability: ExploitReliability::Unstable,
                },
            ],
            filesystem: Vec::new(),
            accepts_token: None,
            local_privesc: None,
        };
        ensure_easy_non_ssh(&mut def);
        let ssh = def
            .vulnerabilities
            .iter()
            .find(|v| v.affected_service == 22)
            .unwrap();
        let http = def
            .vulnerabilities
            .iter()
            .find(|v| v.affected_service == 80)
            .unwrap();
        assert_eq!(http.difficulty, 3, "la vía fácil pasa al servicio no-SSH");
        assert_eq!(ssh.difficulty, 7);
    }
}
