//! # simterm-engine
//!
//! Framework runtime for building immersive terminal-based games and
//! experiences. The engine does not know any concrete mission or story: it
//! interprets a [`Campaign`] loaded from disk.
//!
//! ## Capas
//!
//! - [`model`]: tipos de **definición** (campaña, misiones, hosts, tema...),
//!   cargados desde RON. Inmutables y sin contenido incrustado.
//! - [`runtime`]: el **estado de partida** ([`GameState`]) y las [`actions`]
//!   que lo transforman.
//! - [`loader`]: carga e interpreta una campaña desde una ruta.
//!
//! ## Uso típico (desde un frontend)
//!
//! ```no_run
//! use simterm_engine::{load_campaign, GameState, actions};
//!
//! let campaign = load_campaign("examples/sample_campaign").expect("campaña válida");
//! let mut game = GameState::new(campaign);
//! actions::recon(&mut game);
//! ```
//!
//! The frontend (TUI/CLI) lives in a separate crate (`simterm`), so the runtime
//! does not depend on any interface library.

pub mod asset;
pub mod loader;
pub mod model;
pub mod runtime;

// --- Re-exports de conveniencia para los frontends ---

pub use model::campaign::{Campaign, CampaignAchievement, CampaignAchievementTrigger};
pub use model::filesystem::{self, FsNode, Loot};
pub use model::intel::{FindingSource, FindingStatus, IntelFinding};
pub use model::language::{EngineText, Language};
pub use model::mission::{Ending, EntryVector, Mission, NetHost};
pub use model::target::{Service, TargetNode, Vulnerability};
pub use model::theme::{EasterEgg, Theme};
pub use model::toolbox::{self, EnumTool, ServiceCat};

pub use runtime::actions;
pub use runtime::state::{AchievementId, GameOutcome, GameState, Phase, ACHIEVEMENTS};

pub use asset::{AssetSource, DirAssetSource, MemAssetSource, NoAssets};
pub use loader::{load_campaign, load_open_campaign, LoadError, OpenCampaign};

#[cfg(test)]
mod loader_tests {
    //! Verifies that the repository sample campaign parses and satisfies basic
    //! invariants. This is the only runtime test that touches disk: it validates
    //! the format, not a concrete story.

    use crate::model::filesystem::FsNode;
    use crate::{load_campaign, Mission, TargetNode};
    use std::path::PathBuf;

    /// Ruta a la campaña de ejemplo, relativa a la raíz del workspace.
    fn sample_path() -> PathBuf {
        // CARGO_MANIFEST_DIR = crates/engine → subir dos niveles a la raíz.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("sample_campaign")
    }

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

    fn has_privesc_key(nodes: &[FsNode]) -> bool {
        nodes.iter().any(|n| match n {
            FsNode::Dir { children, .. } => has_privesc_key(children),
            FsNode::File { loot: Some(l), .. } => l.privesc_key,
            FsNode::File { .. } => false,
        })
    }

    #[test]
    fn sample_campaign_parses_and_is_playable() {
        let camp = load_campaign(sample_path()).expect("la campaña de ejemplo debe cargar");
        assert!(!camp.missions.is_empty());

        for m in &camp.missions {
            for (host, _) in hosts_of(m) {
                assert!(!host.services.is_empty(), "{} sin servicios", m.id);
                assert!(
                    !host.vulnerabilities.is_empty(),
                    "{} sin vulnerabilidades",
                    m.id
                );
            }

            // El host que aloja el objetivo debe ofrecer una ruta segura (llave).
            let hosts = hosts_of(m);
            let target_host = hosts
                .iter()
                .find(|(_, obj)| obj.is_some())
                .or_else(|| hosts.first())
                .expect("toda misión tiene al menos un host");
            assert!(
                has_privesc_key(&target_host.0.filesystem),
                "la misión '{}' no ofrece ruta segura en el host objetivo",
                m.id
            );
        }
    }

    #[test]
    fn sample_objectives_point_to_real_files() {
        use crate::filesystem::{normalize, read_file, ReadOutcome};

        let camp = load_campaign(sample_path()).expect("la campaña de ejemplo debe cargar");
        for m in &camp.missions {
            for (host, objective) in hosts_of(m) {
                if let Some(obj) = objective {
                    let comps = normalize(&[], obj);
                    match read_file(&host.filesystem, &comps) {
                        ReadOutcome::File { .. } => {}
                        _ => panic!(
                            "el objetivo '{}' de '{}' no es un fichero del VFS",
                            obj, m.id
                        ),
                    }
                }
            }
        }
    }
}
