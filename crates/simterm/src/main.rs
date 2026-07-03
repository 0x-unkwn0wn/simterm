//! Terminal frontend for `simterm-engine`.
//!
//! Resolves which campaign to load (CLI `--campaign` or positional path; by
//! default, the sample campaign in the repository), passes it to the framework
//! runtime, and renders the TUI. The frontend contains no campaign content.

mod app;
mod audio;
mod autoplay;
mod command;
mod completion;
mod effects;
mod embedded;
mod registry;
mod ui;

use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use simterm_engine::{load_campaign, validate_campaign, AssetSource, Campaign, ValidationReport};

use app::App;

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Ruta de campaña por defecto si no se indica ninguna (relativa al cwd).
const DEFAULT_CAMPAIGN: &str = "examples/sample_campaign";

fn main() -> ExitCode {
    let (path, mode, no_music, mut autoplay) = match parse_args() {
        Args::Run {
            path,
            mode,
            no_music,
            autoplay,
        } => (path, mode, no_music, autoplay),
        Args::Help => {
            print_usage();
            return ExitCode::SUCCESS;
        }
    };

    // Campaña empaquetada con el autoplay desactivado: ignora cualquier
    // `--autoplay*` para que el jugador no pueda auto-resolver (spoilear) la
    // campaña. En binarios normales `autoplay_disabled()` es siempre `false`.
    if embedded::autoplay_disabled() && autoplay.is_some() {
        eprintln!("simterm: el autoplay está desactivado en esta campaña.");
        autoplay = None;
    }

    // Carga la campaña ANTES de tocar el terminal: si falla, mensaje limpio.
    let loaded = match load_selected_campaign(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("simterm: {e}");
            eprintln!("Indica una campaña válida con: simterm --campaign <ruta>");
            return ExitCode::FAILURE;
        }
    };
    let campaign = loaded.campaign.clone();

    // El autoplayer razona con la lógica de la kill chain (recon/exploit/...): no
    // aplica a un dominio propio. Se desactiva silenciosamente en ese caso.
    if autoplay.is_some() && !campaign.kill_chain() {
        eprintln!("simterm: --autoplay solo está disponible en campañas de intrusión.");
        autoplay = None;
    }

    match mode {
        // Validación básica: confirma que la campaña carga y termina (sin TUI).
        Mode::Check => {
            println!(
                "ok: '{}' — {} misión(es). Campaña válida.",
                campaign.name,
                campaign.missions.len()
            );
            ExitCode::SUCCESS
        }
        // Validación avanzada: análisis semántico legible. Sale con código no-cero
        // si hay errores (no si solo hay avisos).
        Mode::Doctor => {
            let reserved = registry::reserved_verbs();
            let report = validate_campaign(&campaign, &reserved);
            print_doctor_report(&campaign.name, &report);
            if report.has_errors() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Mode::Play => {
            // El audio es opcional: solo se prepara si la campaña declara alguna
            // pista (`Mission.music`) o existe el `music/` convencional, y salvo
            // `--no-music`. Sin dispositivo o sin ficheros, se juega en silencio.
            let has_music = loaded.has_music(&path);
            let audio = if no_music || !has_music {
                None
            } else if let Some(assets) = loaded.assets {
                audio::Audio::try_new_assets(assets)
            } else {
                let root = campaign_root(&path);
                audio::Audio::try_new(root)
            };
            if let Err(err) = run_game(campaign, audio, autoplay) {
                eprintln!("Error en la ejecución: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
    }
}

struct LoadedCampaign {
    campaign: Campaign,
    assets: Option<Arc<dyn AssetSource>>,
}

impl LoadedCampaign {
    fn has_music(&self, path: &Path) -> bool {
        if let Some(assets) = &self.assets {
            self.campaign.missions.iter().enumerate().any(|(i, m)| {
                let track = m
                    .music
                    .clone()
                    .unwrap_or_else(|| format!("music/mission_{}_theme.wav", i + 1));
                assets.contains(&track)
            })
        } else {
            let root = campaign_root(path);
            self.campaign.missions.iter().any(|m| m.music.is_some()) || root.join("music").is_dir()
        }
    }
}

fn load_selected_campaign(path: &Path) -> Result<LoadedCampaign, String> {
    if embedded::available() {
        let embedded = embedded::load().map_err(|e| e.to_string())?;
        let assets: Arc<dyn AssetSource> = Arc::new(embedded.assets);
        return Ok(LoadedCampaign {
            campaign: embedded.campaign,
            assets: Some(assets),
        });
    }

    let campaign = load_campaign(path).map_err(|e| e.to_string())?;
    Ok(LoadedCampaign {
        campaign,
        assets: None,
    })
}

/// Directorio raíz de una campaña: la propia ruta si `--campaign` es un
/// directorio, o su carpeta contenedora si apunta directamente al `campaign.ron`.
/// Las rutas de música (`Mission.music`) son relativas a esta raíz.
fn campaign_root(campaign_path: &Path) -> PathBuf {
    if campaign_path.is_dir() {
        campaign_path.to_path_buf()
    } else {
        campaign_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Imprime el informe de `--doctor` de forma legible.
fn print_doctor_report(name: &str, report: &ValidationReport) {
    println!("doctor: analizando '{name}'...");
    if report.is_clean() {
        println!("ok: sin errores ni avisos. Campaña sana.");
        return;
    }
    if !report.errors.is_empty() {
        println!("\nERRORES ({}):", report.errors.len());
        for issue in &report.errors {
            println!("  [error] {}: {}", issue.location, issue.message);
        }
    }
    if !report.warnings.is_empty() {
        println!("\nAVISOS ({}):", report.warnings.len());
        for issue in &report.warnings {
            println!("  [aviso] {}: {}", issue.location, issue.message);
        }
    }
    println!();
    if report.has_errors() {
        println!(
            "resultado: {} error(es), {} aviso(s). Corrige los errores.",
            report.errors.len(),
            report.warnings.len()
        );
    } else {
        println!(
            "resultado: 0 errores, {} aviso(s). La campaña es jugable.",
            report.warnings.len()
        );
    }
}

/// Modo de ejecución seleccionado por CLI.
enum Mode {
    Play,
    Check,
    Doctor,
}

enum Args {
    Run {
        path: PathBuf,
        mode: Mode,
        no_music: bool,
        autoplay: Option<autoplay::AutoplayConfig>,
    },
    Help,
}

/// Parseo de argumentos mínimo (sin dependencias externas):
///   simterm [--campaign|-c <ruta>] [--check | --doctor] [--no-music]
///           [--autoplay | --autoplay-deterministic] [--autoplay-delay <ms>] [ruta]
fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut path: Option<PathBuf> = None;
    let mut mode = Mode::Play;
    let mut no_music = false;
    let mut autoplay: Option<autoplay::AutoplayConfig> = None;

    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => return Args::Help,
            "--check" => mode = Mode::Check,
            "--doctor" => mode = Mode::Doctor,
            "--no-music" | "--mute" => no_music = true,
            "--autoplay" => autoplay = Some(autoplay::AutoplayConfig::default()),
            "--autoplay-deterministic" => autoplay = Some(autoplay::AutoplayConfig::strict()),
            "--autoplay-delay" => {
                let delay_ms = args
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(900);
                match autoplay.as_mut() {
                    Some(config) => config.set_delay(delay_ms),
                    None => autoplay = Some(autoplay::AutoplayConfig::with_delay(delay_ms)),
                }
            }
            "-c" | "--campaign" => {
                if let Some(p) = args.next() {
                    path = Some(PathBuf::from(p));
                }
            }
            other => path = Some(PathBuf::from(other)),
        }
    }

    Args::Run {
        path: path.unwrap_or_else(|| PathBuf::from(DEFAULT_CAMPAIGN)),
        mode,
        no_music,
        autoplay,
    }
}

fn print_usage() {
    println!("SimTerm — framework for immersive terminal-based experiences");
    println!();
    println!("USO:");
    println!("  simterm [--campaign <ruta>] [ruta]");
    println!();
    println!("OPCIONES:");
    println!("  -c, --campaign <ruta>   Directorio de campaña (con campaign.ron) o fichero .ron");
    println!("      --check             Valida que la campaña carga y termina (no abre la TUI)");
    println!("      --doctor            Validación semántica avanzada (errores/avisos; sale ≠0 si hay errores)");
    println!("      --no-music          Desactiva la música (por defecto suena '<campaña>/music/mission_N_theme.wav')");
    println!("      --autoplay          Juega la campaña automáticamente, visible paso a paso");
    println!("      --autoplay-deterministic");
    println!(
        "                          Autoplay estricto: evita exploits/escaladas probabilísticas"
    );
    println!("      --autoplay-delay ms Pausa entre comandos del autoplay (por defecto 900)");
    println!("  -h, --help              Muestra esta ayuda");
    println!();
    println!("EJEMPLOS:");
    println!("  simterm --campaign ./examples/sample_campaign");
    println!("  simterm ./campaigns/mi_campaña");
    println!();
    println!("Si no se indica ruta, se usa '{DEFAULT_CAMPAIGN}'.");
}

fn run_game(
    campaign: simterm_engine::Campaign,
    audio: Option<audio::Audio>,
    autoplay: Option<autoplay::AutoplayConfig>,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new(campaign, audio);
    if let Some(config) = autoplay {
        app.enable_autoplay(config);
    }

    let result = run(&mut terminal, &mut app);

    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()
}

fn run(terminal: &mut Tui, app: &mut App) -> io::Result<()> {
    while app.game.core.running {
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Durante una animación refrescamos más a menudo para que sea fluida.
        let timeout = if app.animating() {
            Duration::from_millis(40)
        } else {
            Duration::from_millis(150)
        };

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
                Event::Mouse(me) => match me.kind {
                    MouseEventKind::ScrollUp => app.scroll_wheel(true),
                    MouseEventKind::ScrollDown => app.scroll_wheel(false),
                    _ => {}
                },
                _ => {}
            }
        }

        // Avanza las animaciones por tiempo real.
        app.on_tick();
    }
    Ok(())
}
