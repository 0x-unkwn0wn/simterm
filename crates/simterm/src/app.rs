//! Estado de la aplicación: une el GameState con la entrada de comandos y el scroll.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use simterm_engine::actions;
use simterm_engine::{Campaign, GameOutcome, GameState};

use crate::audio::Audio;
use crate::autoplay::{Autoplay, AutoplayConfig, AutoplayMode};
use crate::command::{self, Command};
use crate::completion::{self, Completion};
use crate::effects::{Effect, EffectKind};

mod info;
mod minigames;

/// Duración de cada frame de animación.
const FRAME_DUR: Duration = Duration::from_millis(45);

pub struct App {
    pub game: GameState,
    pub input: String,
    /// Posición del cursor de edición dentro de `input` (en caracteres).
    pub cursor: usize,
    /// Desplazamiento vertical del panel de logs (líneas desde arriba).
    pub scroll: u16,
    /// Si true, el panel de logs sigue automáticamente la última línea.
    pub follow: bool,
    /// Altura visible del panel de logs (la rellena la UI en cada frame).
    pub log_view_height: u16,
    /// Ancho visible del panel de logs (la rellena la UI en cada frame). Se usa
    /// para disponer en columnas la lista de autocompletado.
    pub log_view_width: u16,
    /// Nº total de líneas del panel de logs en el último frame.
    pub log_total_lines: u16,
    /// Historial de comandos introducidos (el más reciente al final).
    pub history: Vec<String>,
    /// Posición actual al recorrer el historial con las flechas.
    pub hist_pos: Option<usize>,
    /// Animación activa (typewriter / alerta), si la hay.
    pub effect: Option<Effect>,
    /// Animaciones encoladas tras la actual (p. ej. debrief -> briefing).
    pub effect_queue: VecDeque<Effect>,
    /// Minijuego de cifrado: texto claro de la señal interceptada activa.
    signal_answer: Option<String>,
    /// Minijuego de teclado numérico: (PIN secreto, intentos realizados).
    keypad: Option<(u16, u32)>,
    /// Minijuego mastermind: (código secreto de 4 dígitos únicos, intentos).
    mastermind: Option<([u8; 4], u32)>,
    /// Marca de tiempo del último tick (para avanzar las animaciones).
    last_tick: Instant,
    /// Tiempo acumulado desde el último frame de animación.
    tick_accum: Duration,
    /// Subsistema de audio (música por misión). `None` si va en silencio.
    audio: Option<Audio>,
    /// Autoplayer visible: si está activo, juega la campaña solo, paso a paso.
    autoplay: Option<Autoplay>,
}

impl App {
    /// Construye la aplicación a partir de una campaña ya cargada y un subsistema
    /// de audio opcional (la música por misión; `None` = silencio).
    pub fn new(campaign: Campaign, audio: Option<Audio>) -> Self {
        // Si hay progreso guardado, reanuda la campaña en su nivel antes de
        // construir el briefing de arranque.
        let mut game = GameState::new(campaign);
        game.try_resume();
        let start_level = game.level_index;

        // Overlay de arranque: su texto lo define la campaña (theme).
        let boot = Effect::typed(EffectKind::Boot {
            header: game.campaign.theme.boot_header.clone(),
            lines: game.campaign.theme.boot_lines.clone(),
        });

        let mut app = App {
            game,
            input: String::new(),
            cursor: 0,
            scroll: 0,
            follow: true,
            log_view_height: 0,
            log_view_width: 0,
            log_total_lines: 0,
            history: Vec::new(),
            hist_pos: None,
            effect: Some(boot),
            effect_queue: VecDeque::new(),
            signal_answer: None,
            keypad: None,
            mastermind: None,
            last_tick: Instant::now(),
            tick_accum: Duration::ZERO,
            audio,
            autoplay: None,
        };
        // Tras el arranque, el briefing de la operación activa (1ª o reanudada).
        let brief = app.briefing_effect(start_level);
        app.effect_queue.push_back(brief);
        // Arranca la pista de la misión activa (si hay audio).
        app.sync_audio();
        app
    }

    /// Activa el autoplayer visible: la campaña se jugará sola, paso a paso,
    /// inyectando comandos reales por el mismo dispatcher que el jugador.
    pub fn enable_autoplay(&mut self, config: AutoplayConfig) {
        let label = match config.mode {
            AutoplayMode::Normal => "normal",
            AutoplayMode::Strict => "determinista",
        };
        self.autoplay = Some(Autoplay::new(config));
        self.game.log(format!(
            "[autoplay] Activado ({label}): la campaña se jugará automáticamente paso a paso."
        ));
    }

    /// Sincroniza la música con la misión activa. No hace nada si no hay audio o
    /// si ya suena la pista correcta. Usa la pista declarada por la misión
    /// (`Mission.music`) si existe; si no, la convención por nombre.
    fn sync_audio(&mut self) {
        if let Some(audio) = self.audio.as_mut() {
            let level = self.game.level_index;
            let track = self.game.campaign.missions[level].music.clone();
            audio.set_level(level, track.as_deref());
        }
    }

    /// Construye el efecto de briefing (typewriter) de una operación.
    fn briefing_effect(&self, idx: usize) -> Effect {
        let m = &self.game.campaign.missions[idx];
        Effect::typed(EffectKind::Briefing {
            number: idx + 1,
            total: self.game.level_count(),
            name: m.name.clone(),
            lines: m.briefing.clone(),
        })
    }

    /// Construye el efecto de debrief (cierre de operación), con el resumen de
    /// sigilo del nivel anexado al final.
    fn debrief_effect(&self, idx: usize, final_op: bool) -> Effect {
        let m = &self.game.campaign.missions[idx];
        // En el cierre de campaña con elección, el overlay muestra el epílogo
        // escogido en vez del debrief estándar.
        let mut lines = match (final_op, &self.game.core.epilogue) {
            (true, Some(epi)) => epi.clone(),
            _ => m.debrief.clone(),
        };
        if let Some(summary) = &self.game.core.last_summary {
            lines.push(String::new());
            lines.push(summary.clone());
        }
        Effect::typed(EffectKind::Debrief {
            name: m.name.clone(),
            lines,
            final_op,
        })
    }

    /// Reproduce una secuencia de efectos: el primero entra en juego, el resto
    /// se encola (la entrada queda bloqueada hasta que se vacía la cola).
    fn show_effects(&mut self, effects: Vec<Effect>) {
        let mut it = effects.into_iter();
        if let Some(first) = it.next() {
            self.effect = Some(first);
            self.tick_accum = Duration::ZERO;
        }
        for e in it {
            self.effect_queue.push_back(e);
        }
    }

    /// Termina el efecto actual y saca el siguiente de la cola (o ninguno).
    fn advance_effect(&mut self) {
        self.effect = self.effect_queue.pop_front();
        self.tick_accum = Duration::ZERO;
    }

    pub fn animating(&self) -> bool {
        self.effect.is_some()
    }

    /// Avanza la animación activa según el tiempo real transcurrido.
    pub fn on_tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick);
        self.last_tick = now;

        if self.effect.is_some() {
            self.tick_accum += dt;
            let mut done = false;
            if let Some(eff) = self.effect.as_mut() {
                while self.tick_accum >= FRAME_DUR {
                    self.tick_accum -= FRAME_DUR;
                    eff.frame += 1;
                }
                done = eff.done();
            }
            if done {
                self.advance_effect();
            }
        } else {
            self.tick_accum = Duration::ZERO;
        }

        // El autoplayer solo actúa cuando no hay animación tapando la pantalla.
        if self.effect.is_none() {
            self.autoplay_tick(now);
        }
    }

    /// Pide al autoplayer el siguiente comando (si toca por tiempo) y lo ejecuta
    /// por el dispatcher normal, como si lo tecleara el jugador.
    fn autoplay_tick(&mut self, now: Instant) {
        let Some(mut autoplay) = self.autoplay.take() else {
            return;
        };
        if let Some(line) = autoplay.next_command(&self.game, now) {
            self.submit_line(line);
        }
        self.autoplay = Some(autoplay);
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        // Ctrl-C siempre sale.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.game.running = false;
            return;
        }

        // Durante una animación, cualquier tecla la salta (y pasa a la siguiente).
        if self.effect.is_some() {
            self.advance_effect();
            return;
        }

        // Atajos estilo readline con Ctrl (edición de la línea de comando).
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') => self.cursor = 0,
                KeyCode::Char('e') => self.cursor = self.input_len(),
                KeyCode::Char('u') => self.kill_to_start(),
                KeyCode::Char('k') => self.kill_to_end(),
                KeyCode::Char('w') => self.delete_word_back(),
                _ => {}
            }
            self.follow = true;
            return;
        }

        match key.code {
            KeyCode::Enter => self.submit(),
            KeyCode::Tab => self.complete(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete_forward(),
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Esc => {
                self.input.clear();
                self.cursor = 0;
                self.hist_pos = None;
            }
            // Flechas izda/dcha: mover el cursor de edición.
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.input_len()),
            // Inicio/fin de la línea de comando.
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input_len(),
            // Flechas arriba/abajo: historial de comandos.
            KeyCode::Up => self.history_prev(),
            KeyCode::Down => self.history_next(),
            // Scroll del registro.
            KeyCode::PageUp => self.scroll_up(10),
            KeyCode::PageDown => self.scroll_down(10),
            _ => {}
        }
    }

    // -------------------- Edición de la línea de comando --------------------

    fn input_len(&self) -> usize {
        self.input.chars().count()
    }

    /// Índice de byte del carácter nº `pos` (o el final si se pasa).
    fn byte_at(&self, pos: usize) -> usize {
        self.input
            .char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len())
    }

    fn insert_char(&mut self, c: char) {
        let bi = self.byte_at(self.cursor);
        self.input.insert(bi, c);
        self.cursor += 1;
        self.follow = true;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            let from = self.byte_at(self.cursor - 1);
            let to = self.byte_at(self.cursor);
            self.input.replace_range(from..to, "");
            self.cursor -= 1;
        }
        self.follow = true;
    }

    fn delete_forward(&mut self) {
        if self.cursor < self.input_len() {
            let from = self.byte_at(self.cursor);
            let to = self.byte_at(self.cursor + 1);
            self.input.replace_range(from..to, "");
        }
        self.follow = true;
    }

    fn kill_to_start(&mut self) {
        let from = self.byte_at(self.cursor);
        self.input.replace_range(0..from, "");
        self.cursor = 0;
    }

    fn kill_to_end(&mut self) {
        let from = self.byte_at(self.cursor);
        self.input.truncate(from);
    }

    fn delete_word_back(&mut self) {
        let chars: Vec<char> = self.input.chars().collect();
        let mut start = self.cursor;
        while start > 0 && chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        let from = self.byte_at(start);
        let to = self.byte_at(self.cursor);
        self.input.replace_range(from..to, "");
        self.cursor = start;
    }

    /// Autocompletado con Tab, al estilo de una shell real.
    fn complete(&mut self) {
        self.follow = true;
        match completion::complete(&self.game, &self.input) {
            Completion::None => {}
            Completion::Replace(line) => {
                self.input = line;
                self.cursor = self.input_len();
            }
            Completion::List { options } => {
                // Como en una shell: se reimprime la línea y debajo, los candidatos
                // dispuestos en columnas alineadas al ancho visible del panel.
                let prompt = self.game.prompt();
                self.game.log(format!("{prompt}{}", self.input));
                for line in completion::format_columns(&options, self.log_view_width) {
                    self.game.log(line);
                }
            }
        }
    }

    /// Scroll del registro por rueda de ratón (una muesca ≈ 3 líneas).
    pub fn scroll_wheel(&mut self, up: bool) {
        if up {
            self.scroll_up(3);
        } else {
            self.scroll_down(3);
        }
    }

    fn scroll_up(&mut self, n: u16) {
        self.follow = false;
        self.scroll = self.scroll.saturating_sub(n);
    }

    fn scroll_down(&mut self, n: u16) {
        let max = self.max_scroll();
        self.scroll = (self.scroll + n).min(max);
        if self.scroll >= max {
            self.follow = true;
        }
    }

    pub fn max_scroll(&self) -> u16 {
        self.log_total_lines.saturating_sub(self.log_view_height)
    }

    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let pos = match self.hist_pos {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(p) => p - 1,
        };
        self.hist_pos = Some(pos);
        self.input = self.history[pos].clone();
        self.cursor = self.input_len();
        self.follow = true;
    }

    fn history_next(&mut self) {
        match self.hist_pos {
            Some(p) if p + 1 < self.history.len() => {
                self.hist_pos = Some(p + 1);
                self.input = self.history[p + 1].clone();
            }
            Some(_) => {
                // Pasada la entrada más reciente: línea nueva en blanco.
                self.hist_pos = None;
                self.input.clear();
            }
            None => {}
        }
        self.cursor = self.input_len();
        self.follow = true;
    }

    fn submit(&mut self) {
        let raw = std::mem::take(&mut self.input);
        self.cursor = 0;
        self.submit_line(raw);
    }

    /// Ejecuta una línea de comando concreta (la tecleada por el jugador o la
    /// inyectada por el autoplayer). El eco, el historial y las transiciones son
    /// idénticos en ambos casos.
    fn submit_line(&mut self, raw: String) {
        let cmd = command::parse(&raw, self.game.campaign.kill_chain());
        self.hist_pos = None;

        // El eco del comando se registra con el prompt (excepto entrada vacía).
        if cmd != Command::Empty {
            let trimmed = raw.trim().to_string();
            let prompt = self.game.prompt();
            self.game.log(format!("{prompt}{trimmed}"));
            // Se guarda en el historial evitando duplicar el último.
            if self.history.last().map(|s| s.as_str()) != Some(trimmed.as_str()) {
                self.history.push(trimmed);
            }
        }

        // El scroll vuelve a seguir el final tras ejecutar un comando.
        self.follow = true;

        // Capturamos estado previo para detectar transiciones de nivel/partida.
        let prev_level = self.game.level_index;
        let prev_outcome = self.game.outcome;

        self.dispatch(cmd);

        self.trigger_transition(prev_level, prev_outcome);

        // Si el comando cambió de misión (o reinició), ajusta la música.
        self.sync_audio();
    }

    /// Dispara la animación adecuada si el comando cambió de operación o terminó
    /// la campaña (el estado ya ha cambiado; el efecto es solo visual). El texto
    /// de cada overlay procede de la campaña (theme).
    fn trigger_transition(&mut self, prev_level: usize, prev_outcome: Option<GameOutcome>) {
        if self.game.outcome != prev_outcome {
            match self.game.outcome {
                // Última operación cerrada: debrief final y, tras él, el rollo
                // de créditos cinemático que cierra la campaña.
                Some(GameOutcome::Victory) => {
                    let d = self.debrief_effect(self.game.level_index, true);
                    let credits = Effect::credits(self.game.campaign.theme.credits.clone());
                    self.show_effects(vec![d, credits]);
                }
                // Traza al límite: alerta crítica (parpadeo en ladrillo).
                Some(GameOutcome::Defeat) => {
                    let lines = self.game.campaign.theme.aborted_lines.clone();
                    self.show_effects(vec![Effect::typed(EffectKind::Aborted { lines })]);
                }
                None => {}
            }
        } else if self.game.level_index != prev_level {
            // Operación previa cerrada: debrief y, a continuación, el nuevo briefing.
            let d = self.debrief_effect(prev_level, false);
            let b = self.briefing_effect(self.game.level_index);
            self.show_effects(vec![d, b]);
        }
    }

    fn dispatch(&mut self, cmd: Command) {
        // Si la partida terminó, solo se admite salir (y limpiar la pantalla).
        if self.game.is_over()
            && cmd != Command::Quit
            && cmd != Command::Empty
            && cmd != Command::Clear
            && cmd != Command::Reset
        {
            self.game.log(String::from(
                "La partida ha terminado. Escribe 'quit' para salir.",
            ));
            return;
        }

        match cmd {
            Command::Empty => {}
            Command::Clear => {
                self.game.logs.clear();
                self.follow = true;
            }
            Command::Help { all } => self.cmd_help(all),
            Command::Target => self.cmd_target(),
            Command::Recon => actions::recon(&mut self.game),
            Command::Sniff => actions::sniff(&mut self.game),
            Command::Connect(host) => actions::connect(&mut self.game, host),
            Command::Netmap => actions::netmap(&mut self.game),
            Command::Pivot(host) => actions::pivot(&mut self.game, host),
            Command::Enumerate(tool, port) => actions::enumerate(&mut self.game, &tool, port),
            Command::Research(id) => actions::research(&mut self.game, id),
            Command::Intel => self.cmd_intel(),
            Command::Exploit(id) => actions::exploit(&mut self.game, id),
            Command::Login => actions::login(&mut self.game),
            Command::Privesc => actions::privesc(&mut self.game),
            Command::Loot => actions::loot(&mut self.game),
            Command::John(path) => actions::john(&mut self.game, path),
            Command::Strings(path) => actions::strings(&mut self.game, path),
            Command::Disasm(path) => actions::disasm(&mut self.game, path),
            Command::Solve(path, secret) => actions::solve(&mut self.game, path, secret),
            Command::DecodeFile { tool, path, key } => {
                actions::decode_cmd(&mut self.game, &tool, path, key)
            }
            Command::LocalEnum(tool) => actions::local_enum(&mut self.game, &tool),
            Command::Ls(path) => actions::fs_ls(&mut self.game, path),
            Command::Cat(path) => actions::fs_cat(&mut self.game, path),
            Command::Exfil(path) => actions::fs_exfil(&mut self.game, path),
            Command::Cd(path) => actions::fs_cd(&mut self.game, path),
            Command::Pwd => actions::fs_pwd(&mut self.game),
            Command::Find(needle) => actions::fs_find(&mut self.game, needle),
            Command::Cleanup => actions::cleanup(&mut self.game),
            Command::Reset => self.game.reset_campaign(),
            Command::Choose(n) => match n {
                Some(c) if c >= 1 => self.game.resolve_ending(c - 1),
                _ => self
                    .game
                    .log(String::from("uso: choose <n> (el número de la opción)")),
            },
            Command::Status => self.cmd_status(),
            Command::Logs => self.cmd_logs(),
            Command::Achievements => self.cmd_achievements(),
            Command::Quit => self.game.running = false,
            Command::Shell { verb, args } => self.cmd_shell(verb, args),
            Command::Echo(text) => {
                // `echo` expande variables de entorno ($VAR, ${VAR}, $?).
                let line = simterm_engine::sysemu::expand_vars(&self.game, &text);
                self.game.log(line);
            }
            Command::Fortune => self.cmd_fortune(),
            Command::Signal => self.cmd_signal(),
            Command::Decode(text) => self.cmd_decode(text),
            Command::Crack(guess) => self.cmd_crack(guess),
            Command::History => self.cmd_history(),
            Command::Mastermind(arg) => self.cmd_mastermind(arg),
            Command::BadPort(raw) => self.game.log(format!(
                "Puerto inválido: '{raw}'. Uso: <herramienta> <puerto> (ej. nikto 80)."
            )),
            Command::BadId(raw) => self.game.log(format!(
                "Id inválido: '{raw}'. Uso: searchsploit <id> | exploit <id>."
            )),
            // Verbo no reconocido: se prueba (en orden) comando declarativo con
            // efectos, comando de terminal autorado, easter egg de sabor y, si
            // nada casa, se responde con el `command not found` de una shell real.
            Command::Unknown { verb, args } => self.cmd_unknown(verb, args),
        }
    }

    /// Comando de sistema emulado: delega en el motor (`sysemu`) y refleja su
    /// salida y código de retorno (`$?`).
    fn cmd_shell(&mut self, verb: String, args: Vec<String>) {
        match simterm_engine::sysemu::run(&mut self.game, &verb, &args) {
            Some(out) => {
                for line in out.lines {
                    self.game.log(line);
                }
                self.game.core.last_exit = out.exit;
            }
            None => {
                self.game.log(format!("bash: {verb}: command not found"));
                self.game.core.last_exit = 127;
            }
        }
    }

    /// Verbo no reconocido: comando declarativo → comando de terminal autorado →
    /// easter egg → `command not found` (tono de shell real).
    fn cmd_unknown(&mut self, verb: String, args: Vec<String>) {
        if actions::campaign_command(&mut self.game, &verb) {
            return;
        }
        let arg_line = args.join(" ");
        if actions::terminal_command(&mut self.game, &verb, &arg_line) {
            return;
        }
        if self.try_easter(&verb) {
            return;
        }
        self.game.log(format!("bash: {verb}: command not found"));
        self.game.core.last_exit = 127;
    }
}
