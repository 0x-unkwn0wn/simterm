//! Terminal frontend for `simterm-engine`.
//!
//! Resolves which campaign to load (CLI `--campaign` or positional path; by
//! default, the sample campaign in the repository), passes it to the framework
//! runtime, and renders the TUI. The frontend contains no campaign content.

mod app;
mod command;
mod completion;
mod effects;
mod ui;

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use simterm_engine::load_campaign;

use app::App;

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Ruta de campaña por defecto si no se indica ninguna (relativa al cwd).
const DEFAULT_CAMPAIGN: &str = "examples/sample_campaign";

fn main() -> ExitCode {
    let (path, check_only) = match parse_args() {
        Args::Run { path, check } => (path, check),
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

    // Modo validación: confirma que la campaña carga y termina (no abre la TUI).
    if check_only {
        println!(
            "ok: '{}' — {} misión(es). Campaña válida.",
            campaign.name,
            campaign.missions.len()
        );
        return ExitCode::SUCCESS;
    }

    if let Err(err) = run_game(campaign) {
        eprintln!("Error en la ejecución: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

enum Args {
    Run { path: PathBuf, check: bool },
    Help,
}

/// Parseo de argumentos mínimo (sin dependencias externas):
///   simterm [--campaign|-c <ruta>] [--check] [ruta]
fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut path: Option<PathBuf> = None;
    let mut check = false;

    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => return Args::Help,
            "--check" => check = true,
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
        check,
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
    println!("  -h, --help              Muestra esta ayuda");
    println!();
    println!("EJEMPLOS:");
    println!("  simterm --campaign ./examples/sample_campaign");
    println!("  simterm ./campaigns/mi_campaña");
    println!();
    println!("Si no se indica ruta, se usa '{DEFAULT_CAMPAIGN}'.");
}

fn run_game(campaign: simterm_engine::Campaign) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new(campaign);

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
