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
pub mod domains;
pub mod loader;
pub mod model;
pub mod runtime;
pub mod validate;

// --- Re-exports de conveniencia para los frontends ---

pub use model::campaign::{
    Campaign, CampaignAchievement, CampaignAchievementTrigger, DomainKind, Features,
};
pub use model::command::{CampaignCommand, CommandCondition, CommandEffect};
pub use model::filesystem::{self, FsNode, Loot};
pub use model::meter::{MeterDef, MeterTrigger, OnLimit};
pub use model::intel::{FindingSource, FindingStatus, IntelFinding};
pub use model::language::{EngineText, Language};
pub use model::mission::{Ending, EntryVector, Mission, NetHost};
pub use model::target::{Service, TargetNode, Vulnerability};
pub use model::terminal::TerminalCommand;
pub use model::theme::{EasterEgg, Theme};
pub use model::world::WorldNode;
pub use model::toolbox::{self, EnumTool, ServiceCat};

pub use runtime::actions;
pub use runtime::core::CoreState;
pub use runtime::state::{AchievementId, GameOutcome, GameState, Phase, ACHIEVEMENTS};
pub use runtime::sysemu::{self, ShellOutput};
pub use runtime::shell::{self, run_pipeline, PipelineResult};

pub use asset::{AssetSource, DirAssetSource, MemAssetSource, NoAssets};
pub use loader::{load_campaign, load_open_campaign, LoadError, OpenCampaign};
pub use validate::{validate_campaign, ValidationIssue, ValidationReport};

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

#[cfg(test)]
mod demo_orbita_tests {
    //! La campaña demo NO-hacking (ORBITA-7) es jugable SOLO con datos: etapas
    //! propias, medidores (batería=derrota, enlace=victoria) y comandos
    //! declarativos que conducen tanto a la victoria como a la derrota. Es la
    //! prueba de que el motor ya no está atado al dominio de pentesting.

    use crate::{actions, load_campaign, GameOutcome, GameState};
    use std::path::PathBuf;

    fn demo() -> GameState {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("demo_orbita");
        GameState::new(load_campaign(path).expect("la campaña demo debe cargar"))
    }

    #[test]
    fn ruta_de_victoria_con_paneles_solares() {
        let mut g = demo();
        assert_eq!(g.stage_label(), "ARRANQUE");
        assert_eq!(g.meter("bateria").map(|m| m.value), Some(10.0));

        // El arranque de un dominio propio NO debe filtrar hints de la kill chain.
        assert!(
            !g.core.logs.iter().any(|l| l.contains("nmap") || l.contains("Traza")),
            "no deben colarse hints pentest en el arranque: {:?}",
            g.core.logs
        );

        assert!(actions::campaign_command(&mut g, "encender"));
        assert_eq!(g.stage_label(), "DIAGNÓSTICO", "ReachStage avanza la etapa");
        assert!(actions::campaign_command(&mut g, "desplegar")); // +8 batería
        assert!(actions::campaign_command(&mut g, "diagnostico"));
        assert!(actions::campaign_command(&mut g, "orientar"));
        assert_eq!(g.stage_label(), "ENLACE");
        assert!(actions::campaign_command(&mut g, "transmitir")); // enlace -> 100

        // Enlace al 100% (on_limit: Win) cierra el nivel; como hay finales, abre
        // la decisión. Elegir uno cierra la campaña en victoria.
        assert!(g.core.awaiting_choice, "el enlace completo abre el final");
        // El cierre es neutral: ni "exfiltrado" ni "traza" (mecánica pentest).
        assert!(
            g.core.logs.iter().any(|l| l.contains("NIVEL COMPLETADO")),
            "cierre neutral esperado"
        );
        assert!(!g.core.logs.iter().any(|l| l.contains("exfiltrado")));
        assert!(
            g.core
                .last_summary
                .as_deref()
                .is_some_and(|s| !s.contains("traza")),
            "el resumen no debe mencionar traza: {:?}",
            g.core.last_summary
        );
        g.resolve_ending(0);
        assert_eq!(g.core.outcome, Some(GameOutcome::Victory));
    }

    /// El VFS es explorable sin "shell" en un dominio propio (shell_for_vfs
    /// cae a `false` por defecto al declarar etapas propias).
    #[test]
    fn vfs_libre_sin_foothold() {
        let mut g = demo();
        assert!(!g.has_foothold(), "el satélite nunca tiene 'foothold' pentest");
        actions::fs_ls(&mut g, None);
        actions::fs_cat(&mut g, Some(String::from("/bitacora.log")));
        assert!(
            g.core.logs.iter().any(|l| l.contains("Modo supervivencia")),
            "cat debe leer el fichero sin exigir shell: {:?}",
            g.core.logs
        );
    }

    #[test]
    fn sin_paneles_la_bateria_se_agota() {
        let mut g = demo();
        actions::campaign_command(&mut g, "encender"); // 10 -> 9
        actions::campaign_command(&mut g, "diagnostico"); // 9 -> 7
        actions::campaign_command(&mut g, "orientar"); // 7 -> 4
        actions::campaign_command(&mut g, "transmitir"); // 4 -> 0 => Fail
        assert_eq!(
            g.core.outcome,
            Some(GameOutcome::Defeat),
            "sin recargar con paneles, la batería se agota y la sonda muere"
        );
    }
}
