//! Terminal frontend for `simterm-engine`.
//!
//! Resolves which campaign to load (CLI `--campaign` or positional path; by
//! default, the sample campaign in the repository), passes it to the framework
//! runtime, and renders the TUI. The frontend contains no campaign content.

mod app;
mod audio;
mod command;
mod completion;
mod effects;
mod registry;
mod ui;

use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use simterm_engine::{load_campaign, validate_campaign, ValidationReport};

use app::App;

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Ruta de campaña por defecto si no se indica ninguna (relativa al cwd).
const DEFAULT_CAMPAIGN: &str = "examples/sample_campaign";

fn main() -> ExitCode {
    let (path, mode, no_music) = match parse_args() {
        Args::Run {
            path,
            mode,
            no_music,
        } => (path, mode, no_music),
        Args::Help => {
            print_usage();
            return ExitCode::SUCCESS;
        }
    };

    // Carga la campaña ANTES de tocar el terminal: si falla, mensaje limpio.
    let campaign = match load_campaign(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("simterm: {e}");
            eprintln!("Indica una campaña válida con: simterm --campaign <ruta>");
            return ExitCode::FAILURE;
        }
    };

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
            let root = campaign_root(&path);
            let has_music =
                campaign.missions.iter().any(|m| m.music.is_some()) || root.join("music").is_dir();
            let audio = if no_music || !has_music {
                None
            } else {
                audio::Audio::try_new(root)
            };
            if let Err(err) = run_game(campaign, audio) {
                eprintln!("Error en la ejecución: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
    }
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
    },
    Help,
}

/// Parseo de argumentos mínimo (sin dependencias externas):
///   simterm [--campaign|-c <ruta>] [--check | --doctor] [--no-music] [ruta]
fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut path: Option<PathBuf> = None;
    let mut mode = Mode::Play;
    let mut no_music = false;

    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => return Args::Help,
            "--check" => mode = Mode::Check,
            "--doctor" => mode = Mode::Doctor,
            "--no-music" | "--mute" => no_music = true,
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
    println!("  -h, --help              Muestra esta ayuda");
    println!();
    println!("EJEMPLOS:");
    println!("  simterm --campaign ./examples/sample_campaign");
    println!("  simterm ./campaigns/mi_campaña");
    println!();
    println!("Si no se indica ruta, se usa '{DEFAULT_CAMPAIGN}'.");
}

fn run_game(campaign: simterm_engine::Campaign, audio: Option<audio::Audio>) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new(campaign, audio);

    let result = run(&mut terminal, &mut app);

    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

fn run(terminal: &mut Tui, app: &mut App) -> io::Result<()> {
    while app.game.running {
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Durante una animación refrescamos más a menudo para que sea fluida.
        let timeout = if app.animating() {
            Duration::from_millis(40)
        } else {
            Duration::from_millis(150)
        };

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }

        // Avanza las animaciones por tiempo real.
        app.on_tick();
    }
    Ok(())
}
