//! Recorrido de integración de la campaña `bash_avanzado`.
//!
//! Carga la campaña REAL de disco y reproduce, comando a comando, los guiones
//! `autoplay` de cada nivel con la misma lógica que `App::submit_line` (registro
//! de verbos, enrutado de tuberías/redirecciones y despacho). Así se verifica de
//! extremo a extremo que:
//!   - no se puede entregar un nivel sin haber hecho el trabajo (gates reales), y
//!   - siguiendo el guion (leer pistas + ejecutar comandos + tuberías + '>') la
//!     campaña se completa hasta la victoria.
//!
//! Es un test de la BINARIO (no un test externo) para poder usar los módulos
//! privados `command` y la lógica de despacho junto al motor.

#![cfg(test)]

use std::path::PathBuf;

use simterm_engine::{actions, load_campaign, sysemu, GameOutcome, GameState};

use crate::command::{self, Command};

/// Ruta a la campaña de práctica, relativa al manifiesto del crate.
fn campaign_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("campaigns")
        .join("bash_avanzado")
}

/// Carga la campaña de práctica y construye una partida nueva. Devuelve `None`
/// si la campaña no está presente: `campaigns/` está en `.gitignore`, así que en
/// un clon limpio o en CI no existe y estos tests se saltan en vez de fallar.
fn fresh_game() -> Option<GameState> {
    if !campaign_dir().join("campaign.ron").exists() {
        eprintln!("(skip) campaña bash_avanzado no presente (campaigns/ está en .gitignore)");
        return None;
    }
    let campaign = load_campaign(campaign_dir()).expect("la campaña bash_avanzado debe cargar");
    Some(GameState::new(campaign))
}

/// Reproduce una línea igual que `App::submit_line`: registra los verbos (para
/// los gates `RanCommand`), enruta tuberías/redirecciones por el motor de shell
/// y, si no, parsea y despacha el subconjunto de comandos que usa la campaña.
fn run_line(game: &mut GameState, raw: &str) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    for verb in command::verbs_in_line(trimmed) {
        game.record_command(&verb);
    }

    if simterm_engine::shell::is_pipeline(trimmed) {
        let result = simterm_engine::run_pipeline(game, trimmed);
        for l in result.lines {
            game.log(l);
        }
        game.core.last_exit = result.exit;
        return;
    }

    match command::parse(raw, game.campaign.kill_chain()) {
        Command::Cat(p) => actions::fs_cat(game, p),
        Command::Ls(p) => actions::fs_ls(game, p),
        Command::Find(n) => actions::fs_find(game, n),
        Command::Pwd => actions::fs_pwd(game),
        Command::Cd(p) => actions::fs_cd(game, p),
        Command::Choose(n) => {
            if let Some(c) = n {
                game.resolve_ending(c - 1);
            }
        }
        Command::Shell { verb, args } => {
            if let Some(out) = sysemu::run(game, &verb, &args) {
                for l in out.lines {
                    game.log(l);
                }
                game.core.last_exit = out.exit;
            }
        }
        Command::Unknown { verb, args } => {
            if !actions::campaign_command(game, &verb) {
                actions::terminal_command(game, &verb, &args.join(" "));
            }
        }
        Command::Empty => {}
        other => panic!("la campaña usó un comando no cubierto por el test: {other:?}"),
    }
}

#[test]
fn no_se_puede_entregar_el_nivel_1_sin_trabajo() {
    let Some(mut game) = fresh_game() else { return; };
    assert_eq!(game.level_index, 0);

    // Entregar de primeras: el comando está bloqueado (muestra 'locked'), no
    // completa el nivel ni avanza.
    run_line(&mut game, "entregar-globs");
    assert_eq!(game.level_index, 0, "no debe avanzar sin haber hecho el trabajo");
    assert!(game.core.outcome.is_none());
    // Se guió al alumno (mensaje 'locked'), no un críptico 'command not found'.
    assert!(
        game.core.logs.iter().any(|l| l.contains("Aún no")),
        "debe mostrarse el mensaje de ayuda del comando bloqueado"
    );
    // Y un checklist con todo pendiente (nada hecho aún).
    assert!(
        game.core.logs.iter().any(|l| l.contains("[ ]")),
        "debe mostrarse el checklist de requisitos pendientes"
    );
}

#[test]
fn el_checklist_marca_lo_ya_hecho() {
    let Some(mut game) = fresh_game() else { return; };
    // Hace parte del trabajo: explora, pero no lee la pista.
    run_line(&mut game, "ls /datos");
    run_line(&mut game, "find log");
    let before = game.core.logs.len();
    run_line(&mut game, "entregar-globs");
    let shown = &game.core.logs[before..];
    // 'ls' y 'find' hechos -> ✓; 'leer /pistas/globs.md' pendiente -> [ ].
    assert!(
        shown.iter().any(|l| l.contains("[✓]") && l.contains("find")),
        "lo ya ejecutado debe salir con ✓"
    );
    assert!(
        shown.iter().any(|l| l.contains("[ ]") && l.contains("globs.md")),
        "lo que falta debe salir como pendiente"
    );
}

#[test]
fn leer_pista_y_ejecutar_abre_el_gate_del_nivel_1() {
    let Some(mut game) = fresh_game() else { return; };
    // Trabajo real parcial: solo ls + find, sin leer la pista.
    run_line(&mut game, "ls /datos");
    run_line(&mut game, "find log");
    run_line(&mut game, "entregar-globs");
    assert_eq!(game.level_index, 0, "falta leer la pista: sigue bloqueado");

    // Ahora sí: leer la pista completa el trabajo requerido.
    run_line(&mut game, "cat /pistas/globs.md");
    assert!(game.has_read("/pistas/globs.md"));
    run_line(&mut game, "entregar-globs");
    assert_eq!(game.level_index, 1, "con el trabajo hecho, el nivel se entrega");
}

#[test]
fn la_tuberia_find_wc_cuenta_los_logs() {
    let Some(mut game) = fresh_game() else { return; };
    run_line(&mut game, "find log | wc -l");
    // 4 ficheros .log en el VFS del nivel 1 (access, app, worker, y el access de
    // enero) — el conteo exacto lo fija el dataset; comprobamos que sale un número.
    let last = game.core.logs.last().unwrap();
    let n: i32 = last.split_whitespace().last().unwrap().parse().unwrap();
    assert!(n >= 3, "la tubería debe contar los .log encontrados, fue {n}");
}

#[test]
fn el_nivel_3_exige_una_redireccion_real() {
    let Some(mut game) = fresh_game() else { return; };
    // Avanza hasta el nivel 3 jugando los guiones de los niveles 1 y 2.
    for line in game.campaign.missions[0].autoplay.clone() {
        run_line(&mut game, &line);
    }
    for line in game.campaign.missions[1].autoplay.clone() {
        run_line(&mut game, &line);
    }
    assert_eq!(game.level_index, 2, "debe estar en el nivel 3");

    // Leer pista + env, pero SIN redirigir: el gate FileRead('/tmp/errores.txt')
    // no se cumple (el fichero no existe hasta que lo crea la redirección).
    run_line(&mut game, "cat /pistas/scripts.md");
    run_line(&mut game, "env");
    run_line(&mut game, "entregar-scripts");
    assert_eq!(game.level_index, 2, "sin redirección real no se entrega");

    // Redirigir de verdad y leer el fichero producido: ahora el gate se abre.
    run_line(&mut game, "grep ERROR /datos/logs/app.log > /tmp/errores.txt");
    run_line(&mut game, "cat /tmp/errores.txt");
    assert!(game.has_read("/tmp/errores.txt"));
    run_line(&mut game, "entregar-scripts");
    assert_eq!(game.level_index, 3, "con la redirección hecha, se entrega");
}

#[test]
fn recorrido_completo_por_autoplay_llega_a_la_victoria() {
    let Some(mut game) = fresh_game() else { return; };
    // Reproduce el guion de cada misión en orden. Cada guion termina con su
    // 'entregar-*', que solo pasa si el trabajo real quedó registrado.
    let scripts: Vec<Vec<String>> = game
        .campaign
        .missions
        .iter()
        .map(|m| m.autoplay.clone())
        .collect();
    for (i, script) in scripts.iter().enumerate() {
        let before = game.level_index;
        for line in script {
            run_line(&mut game, line);
        }
        // Salvo la última misión (que abre la elección de final), el nivel avanza.
        if i + 1 < scripts.len() {
            assert!(
                game.level_index > before,
                "el nivel {} debió completarse siguiendo su guion",
                i + 1
            );
        }
    }

    // La última misión abre el final con elección; elige un desenlace.
    assert!(game.core.awaiting_choice, "debe esperar la elección final");
    run_line(&mut game, "choose 1");
    assert_eq!(game.core.outcome, Some(GameOutcome::Victory));
}
