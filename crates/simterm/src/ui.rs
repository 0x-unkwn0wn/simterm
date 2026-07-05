//! Renderizado con ratatui. Estética CRT de fósforo ámbar (por defecto).
//!
//! Dos colores y nada más: un matiz sobre negro. El acento crítico se reserva en
//! exclusiva para las alertas (traza al límite). Sin verde parásito, sin cian,
//! sin gris, sin neón. Bordes de línea fina, denso y funcional.
//!
//! Los cinco colores concretos ya no viven aquí: son una `Palette` (preferencia
//! del jugador, ver `palette.rs`) que `draw` toma de `App` en cada frame y pasa
//! a cada función. El jugador la cambia por CLI (`--appearance`) o en vivo (F2).
//!
//! Los textos de marca (título, títulos de overlay) los aporta la campaña vía
//! su `theme`; este módulo solo decide la presentación.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap},
    Frame,
};

use simterm_engine::{GameOutcome, Theme};

use crate::app::App;
use crate::effects::{Effect, EffectKind};
use crate::palette::Palette;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let p = app.palette;

    // Fondo negro uniforme bajo toda la interfaz.
    frame.render_widget(Block::default().style(Style::default().bg(p.bg)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // cabecera
            Constraint::Min(5),    // cuerpo (terminal + barra lateral)
        ])
        .split(area);

    draw_title(frame, chunks[0], app, p);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(38), Constraint::Min(20)])
        .split(chunks[1]);

    draw_sidebar(frame, body[0], app, p);
    draw_console(frame, body[1], app, p);

    // Animación (typewriter / alerta) superpuesta, si está activa.
    if let Some(eff) = app.effect.as_ref() {
        draw_effect(frame, area, eff, &app.game.campaign.theme, p);
    }
}

fn draw_title(frame: &mut Frame, area: Rect, app: &App, p: Palette) {
    let (status_text, status_color) = match app.game.core.outcome {
        Some(GameOutcome::Victory) => (" [ OPERACIÓN CONCLUIDA ] ", p.amber_hi),
        Some(GameOutcome::Defeat) => (" [ ENLACE CORTADO ] ", p.brick),
        None => (" [ EN CURSO ] ", p.amber_dim),
    };

    let g = &app.game;
    let title = Line::from(vec![
        Span::styled(
            format!(" {} ", g.campaign.theme.app_title),
            Style::default()
                .fg(p.bg)
                .bg(p.amber)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " :: OP {}/{} — {} ",
                g.level_number(),
                g.level_count(),
                g.level_name()
            ),
            Style::default().fg(p.amber),
        ),
        Span::styled(
            format!("[etapa {}] ", g.stage_label()),
            Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            status_text,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let par = Paragraph::new(title)
        .alignment(Alignment::Left)
        .block(border_block("", p));
    frame.render_widget(par, area);
}

fn draw_sidebar(frame: &mut Frame, area: Rect, app: &App, p: Palette) {
    // Indicadores del dominio: la "traza" (si se usa) y/o los medidores de la
    // campaña (batería, enlace, oxígeno...). Un dominio sin ninguno deja todo el
    // lateral para los datos del nodo.
    let gauges = collect_gauges(app);
    if gauges.is_empty() {
        draw_target(frame, area, app, p);
        return;
    }
    let gauge_h = 3u16; // cada indicador: borde + barra + borde
    let bottom = (gauges.len() as u16 * gauge_h).min(area.height.saturating_sub(3));
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(bottom)])
        .split(area);

    draw_target(frame, parts[0], app, p);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(gauge_h); gauges.len()])
        .split(parts[1]);
    for (spec, row) in gauges.iter().zip(rows.iter()) {
        draw_gauge(frame, *row, spec, p);
    }
}

fn draw_target(frame: &mut Frame, area: Rect, app: &App, p: Palette) {
    let t = &app.game.pentest().target;
    // Los datos de intrusión (servicios, hallazgos, ventana, defensa, red) solo
    // tienen sentido en el dominio pentest. En un dominio sin kill chain el
    // lateral se reduce a la identidad neutral del nodo + fase/op genéricas.
    let kill_chain = app.game.campaign.kill_chain();
    let mut lines = Vec::new();

    if kill_chain {
        lines.push(kv("host", &t.hostname, p));
        lines.push(kv("ip", &t.ip, p));
        lines.push(kv("os", &t.os, p));
        lines.push(Line::from(Span::styled(
            "servicios:",
            Style::default().fg(p.amber_dim).add_modifier(Modifier::BOLD),
        )));
        let mut any = false;
        for s in &t.services {
            if !app.game.pentest().discovered_ports.contains(&s.port) {
                continue;
            }
            any = true;
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:>5}/tcp ", s.port),
                    Style::default().fg(p.amber_hi),
                ),
                Span::styled(format!("{:<5} ", s.name), Style::default().fg(p.amber)),
                Span::styled(s.version.clone(), Style::default().fg(p.amber_dim)),
            ]));
        }
        if !any {
            lines.push(Line::from(Span::styled(
                "  ??? — ejecuta 'nmap'",
                Style::default().fg(p.amber_dim),
            )));
        }
    } else {
        lines.push(kv("nodo", &t.hostname, p));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("fase    ", Style::default().fg(p.amber_dim)),
        Span::styled(
            app.game.stage_label(),
            Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        format!(
            "op      {}/{}",
            app.game.level_number(),
            app.game.level_count()
        ),
        Style::default().fg(p.amber_dim),
    )));

    if kill_chain {
        match app.game.time_remaining() {
            Some(rem) => {
                // Ventana operativa: muestra ticks usados/restantes. No es tiempo real.
                let limit = app.game.pentest().time_limit.unwrap_or(0);
                let low = limit > 0 && rem * 4 <= limit;
                let style = if low {
                    Style::default().fg(p.brick).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(p.amber_dim)
                };
                lines.push(Line::from(Span::styled(
                    format!("ventana {rem} restantes"),
                    style,
                )));
                lines.push(Line::from(Span::styled(
                    format!("tiempo  t={}/{}", app.game.core.clock, limit),
                    style,
                )));
            }
            None => {
                lines.push(Line::from(Span::styled(
                    format!("tiempo  t={} sin ventana", app.game.core.clock),
                    Style::default().fg(p.amber_dim),
                )));
            }
        }
        lines.push(Line::from(Span::styled(
            format!("hallazgos: {}", app.game.pentest().intel.len()),
            Style::default().fg(p.amber_dim),
        )));

        // Defensa activa (solo en hosts reactivos): muestra la etapa de respuesta
        // del equipo azul. En rojo en cuanto se ha disparado alguna contramedida.
        if app.game.pentest().reactive {
            let stage = app.game.pentest().defense_stage;
            let (txt, style) = match stage {
                0 => (
                    String::from("defensa ACTIVA (en espera)"),
                    Style::default().fg(p.amber_hi),
                ),
                1 => (
                    String::from("defensa: RASTREO (-8% éxito)"),
                    Style::default().fg(p.brick),
                ),
                2 => (
                    String::from("defensa: CONTRAMEDIDAS (-18% éxito)"),
                    Style::default().fg(p.brick).add_modifier(Modifier::BOLD),
                ),
                _ => (
                    String::from("defensa: PURGA (-30% éxito)"),
                    Style::default().fg(p.brick).add_modifier(Modifier::BOLD),
                ),
            };
            lines.push(Line::from(Span::styled(txt, style)));
        }

        // Mapa de la red interna (solo en operaciones multi-host).
        let net = app.game.network_overview();
        if net.len() > 1 {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "red interna:",
                Style::default().fg(p.amber_dim).add_modifier(Modifier::BOLD),
            )));
            for (name, mark, active) in net {
                let style = if active {
                    Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD)
                } else if mark == '#' {
                    Style::default().fg(p.amber)
                } else if mark == '+' {
                    Style::default().fg(p.amber_dim)
                } else {
                    Style::default().fg(p.amber_dim)
                };
                lines.push(Line::from(Span::styled(format!("  {mark} {name}"), style)));
            }
        }
    }

    let par = Paragraph::new(Text::from(lines))
        .block(border_block(" objetivo ", p))
        .style(Style::default().bg(p.bg))
        .wrap(Wrap { trim: false });
    frame.render_widget(par, area);
}

/// Naturaleza de un indicador: define qué extremo es "malo" (color de alerta).
enum GaugeKind {
    /// Rojo cuando SUBE: la traza del pentesting.
    Trace,
    /// Rojo cuando BAJA: un recurso que se agota (batería, oxígeno...).
    Resource,
    /// Solo ámbar, más brillante al llenarse: una barra de progreso.
    Progress,
}

struct GaugeSpec {
    title: String,
    ratio: f64,
    label: String,
    kind: GaugeKind,
}

/// Indicadores del lateral: la "traza" (si el dominio la usa) y los medidores de
/// campaña del nivel activo (batería, enlace, oxígeno...).
fn collect_gauges(app: &App) -> Vec<GaugeSpec> {
    let g = &app.game;
    let mut gauges = Vec::new();

    if g.campaign.uses_trace() {
        let limit = g.pentest().detection_limit;
        gauges.push(GaugeSpec {
            title: String::from(" traza "),
            ratio: g.pentest().detection.ratio(limit) as f64,
            label: format!("{:.0} / {:.0}", g.pentest().detection.value, limit),
            kind: GaugeKind::Trace,
        });
    }

    for def in &g.core.meter_defs {
        if let Some(m) = g.meter(&def.id) {
            let (ratio, kind) = match def.trigger {
                simterm_engine::MeterTrigger::AtLeast => (
                    (m.value / def.limit.max(1.0)).clamp(0.0, 1.0) as f64,
                    GaugeKind::Progress,
                ),
                simterm_engine::MeterTrigger::AtMost => {
                    let span = (def.start - def.limit).abs().max(1.0);
                    (
                        ((m.value - def.limit) / span).clamp(0.0, 1.0) as f64,
                        GaugeKind::Resource,
                    )
                }
            };
            gauges.push(GaugeSpec {
                title: format!(" {} ", def.label()),
                ratio,
                label: format!("{:.0} / {:.0}", m.value, def.limit),
                kind,
            });
        }
    }

    gauges
}

fn draw_gauge(frame: &mut Frame, area: Rect, spec: &GaugeSpec, p: Palette) {
    // El acento crítico se reserva a la zona peligrosa de cada tipo de indicador.
    let color = match spec.kind {
        GaugeKind::Trace => {
            if spec.ratio >= 0.75 {
                p.brick
            } else if spec.ratio >= 0.4 {
                p.amber_hi
            } else {
                p.amber_dim
            }
        }
        GaugeKind::Resource => {
            if spec.ratio <= 0.25 {
                p.brick
            } else if spec.ratio <= 0.6 {
                p.amber_hi
            } else {
                p.amber_dim
            }
        }
        GaugeKind::Progress => {
            if spec.ratio >= 0.6 {
                p.amber_hi
            } else {
                p.amber_dim
            }
        }
    };

    let gauge = Gauge::default()
        .block(border_block(&spec.title, p))
        .gauge_style(Style::default().fg(color).bg(p.bg))
        .ratio(spec.ratio)
        .label(spec.label.clone());
    frame.render_widget(gauge, area);
}

/// Terminal: el flujo de salida y, como última línea viva, el prompt de entrada
/// con el cursor (no hay caja de comando separada).
///
/// El ajuste de línea se hace a mano para que el nº de filas renderizadas
/// coincida exactamente con el cálculo de scroll y la posición del cursor.
fn draw_console(frame: &mut Frame, area: Rect, app: &mut App, p: Palette) {
    let block = border_block(" terminal ", p);
    let inner = block.inner(area);
    let width = inner.width.max(1) as usize;

    let mut lines: Vec<Line> = Vec::new();

    // Salida previa: cada línea lógica se envuelve a `width` columnas.
    for raw in &app.game.core.logs {
        let style = log_style(raw, p);
        for piece in wrap_str(raw, width) {
            lines.push(Line::from(Span::styled(piece, style)));
        }
    }

    // Línea viva del prompt (también envuelta), y fila/columna del cursor.
    let prompt = app.game.prompt();
    let prompt_first_row = lines.len();
    let (cursor_row_off, cursor_col) =
        push_prompt_lines(&mut lines, &prompt, &app.input, app.cursor, width, p);

    let total = lines.len() as u16;

    // La UI informa al estado del tamaño visible para gestionar el scroll y la
    // disposición en columnas del autocompletado.
    app.log_view_height = inner.height;
    app.log_view_width = inner.width;
    app.log_total_lines = total;

    let max_scroll = total.saturating_sub(inner.height);
    if app.follow {
        app.scroll = max_scroll;
    } else if app.scroll > max_scroll {
        app.scroll = max_scroll;
    }

    let par = Paragraph::new(Text::from(lines))
        .block(block)
        .style(Style::default().bg(p.bg))
        .scroll((app.scroll, 0));
    frame.render_widget(par, area);

    // El cursor se sitúa en el prompt vivo, sólo si la fila es visible y no
    // hay una animación tapando la pantalla.
    let cursor_abs = prompt_first_row + cursor_row_off;
    if !app.animating() && cursor_abs >= app.scroll as usize {
        let row = cursor_abs - app.scroll as usize;
        if (row as u16) < inner.height {
            let x = inner.x + (cursor_col as u16).min(inner.width.saturating_sub(1));
            let y = inner.y + row as u16;
            frame.set_cursor_position((x, y));
        }
    }
}

/// Añade las filas envueltas del prompt vivo y devuelve `(fila, columna)`
/// relativas al inicio del prompt donde debe quedar el cursor, situado según
/// `cursor` (índice de carácter dentro de `input`).
fn push_prompt_lines(
    lines: &mut Vec<Line<'static>>,
    prompt: &str,
    input: &str,
    cursor: usize,
    width: usize,
    p: Palette,
) -> (usize, usize) {
    let prompt_len = prompt.chars().count();
    let full = format!("{prompt}{input}");
    let pieces = wrap_str(&full, width);

    let mut start = 0usize;
    for piece in &pieces {
        let plen = piece.chars().count();
        let line = if start + plen <= prompt_len {
            // Trozo enteramente dentro del prompt.
            Line::from(Span::styled(
                piece.clone(),
                Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD),
            ))
        } else if start >= prompt_len {
            // Trozo enteramente del texto introducido.
            Line::from(Span::styled(piece.clone(), Style::default().fg(p.amber)))
        } else {
            // Trozo a caballo entre prompt y texto.
            let (a, b) = split_at_char(piece, prompt_len - start);
            Line::from(vec![
                Span::styled(
                    a,
                    Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD),
                ),
                Span::styled(b, Style::default().fg(p.amber)),
            ])
        };
        lines.push(line);
        start += plen;
    }

    // El cursor cae en el offset absoluto (prompt + posición de edición). Si
    // queda en una fila más allá de las renderizadas (p. ej. al final justo en
    // un salto de línea), se añade una fila vacía para alojarlo.
    let cur_abs = prompt_len + cursor;
    let (crow, ccol) = (cur_abs / width, cur_abs % width);
    let mut rows = pieces.len();
    while rows <= crow {
        lines.push(Line::from(""));
        rows += 1;
    }
    (crow, ccol)
}

/// Estilo (color + énfasis) de una línea de registro según su contenido.
/// El acento crítico se reserva a lo crítico; todo lo demás es ámbar en sus tonos.
fn log_style(raw: &str, p: Palette) -> Style {
    // El eco de un comando del jugador se resalta como su prompt.
    let content = raw.splitn(2, "] ").nth(1).unwrap_or(raw);
    if looks_like_prompt(content) {
        return Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD);
    }

    let lower = raw.to_lowercase();

    // Crítico (acento): solo traza al límite / operación abortada.
    if raw.contains("!!")
        || lower.contains("abortada")
        || lower.contains("al límite")
        || lower.contains("traza tu conexión")
    {
        return Style::default().fg(p.brick).add_modifier(Modifier::BOLD);
    }

    // Datos clave / hitos (ámbar claro): cabeceras, éxito, exfiltración.
    if raw.contains("##")
        || raw.starts_with("===")
        || raw.contains("--- ")
        || lower.contains("éxito")
        || lower.contains("exfil")
        || lower.contains("root conseguido")
        || lower.contains("completad")
    {
        return Style::default().fg(p.amber_hi);
    }

    // Resto: texto principal en ámbar.
    Style::default().fg(p.amber)
}

/// Parte una cadena en trozos de como mucho `width` caracteres (hard-wrap).
fn wrap_str(s: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![s.to_string()];
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let end = (i + width).min(chars.len());
        out.push(chars[i..end].iter().collect());
        i = end;
    }
    out
}

/// Heurística: ¿la línea parece el eco de un prompt "usuario@host:ruta$ "?
fn looks_like_prompt(s: &str) -> bool {
    let s = s.trim_start();
    if s.starts_with('[') {
        return false; // líneas de herramienta: "[nmap] ..."
    }
    let mut seen = false;
    for ch in s.chars() {
        if ch == '@' {
            return seen;
        }
        if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            seen = true;
        } else {
            return false;
        }
    }
    false
}

/// Divide una cadena en el índice de carácter `n` (no de byte).
fn split_at_char(s: &str, n: usize) -> (String, String) {
    let idx = s
        .char_indices()
        .nth(n)
        .map(|(i, _)| i)
        .unwrap_or_else(|| s.len());
    (s[..idx].to_string(), s[idx..].to_string())
}

fn kv(key: &str, value: &str, p: Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<8}"), Style::default().fg(p.amber_dim)),
        Span::styled(value.to_string(), Style::default().fg(p.amber)),
    ])
}

/// Bloque con borde de línea fina en ámbar oscuro (UI secundaria).
fn border_block(title: &str, p: Palette) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.amber_dim))
        .style(Style::default().bg(p.bg))
        .title(Span::styled(
            title.to_string(),
            Style::default().fg(p.amber_dim).add_modifier(Modifier::BOLD),
        ))
}

// ===================== Overlay: typewriter y alertas ======================

/// Dibuja el panel del efecto activo por encima de la interfaz: un recuadro
/// sobrio centrado, con el cuerpo revelándose carácter a carácter (typewriter).
/// Las alertas críticas usan el acento y parpadean lentamente.
fn draw_effect(frame: &mut Frame, area: Rect, eff: &Effect, theme: &Theme, p: Palette) {
    // Los créditos no son un panel centrado, sino un rollo a pantalla completa.
    if let EffectKind::Credits { lines } = &eff.kind {
        draw_credits(frame, area, eff, lines, theme, p);
        return;
    }

    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(Style::default().bg(p.bg)), area);

    let critical = eff.kind.is_critical();
    // Parpadeo lento: solo en alertas críticas, y solo una vez revelado el texto.
    // Un periodo de 0 en la paleta desactiva el parpadeo (fotosensibilidad).
    let blink_on = if critical && !eff.typing() && p.blink_period > 0 {
        (eff.frame / p.blink_period) % 2 == 0
    } else {
        true
    };

    let accent = if critical { p.brick } else { p.amber_hi };
    let border_color = if critical {
        if blink_on {
            p.brick
        } else {
            p.amber_dim
        }
    } else {
        p.amber_dim
    };

    let header = eff.kind.header();
    let body = eff.kind.body();

    // --- Geometría FIJA: se calcula con el texto completo, no con el revelado,
    // para que el recuadro no crezca mientras se mecanografía dentro. ---
    let full_w = header
        .chars()
        .count()
        .max(body.iter().map(|l| l.chars().count()).max().unwrap_or(0));
    // Filas siempre: cabecera + línea en blanco + una por cada línea del cuerpo.
    let full_h = 2 + body.len();
    let panel_w = ((full_w + 4) as u16)
        .min(area.width.saturating_sub(2))
        .max(22);
    let panel_h = ((full_h + 2) as u16)
        .min(area.height.saturating_sub(2))
        .max(3);
    let panel = centered_rect(panel_w, panel_h, area);

    // --- Composición de las líneas (revelando el cuerpo carácter a carácter) ---
    let mut content: Vec<Line> = Vec::new();
    content.push(Line::from(Span::styled(
        header,
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));
    content.push(Line::from(""));
    typed_body(&body, eff.revealed(), eff.typing(), &mut content, critical, p);

    let title = if critical {
        theme.alert_title.clone()
    } else {
        theme.overlay_title.clone()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(p.bg))
        .title(Span::styled(
            title,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

    let par = Paragraph::new(Text::from(content))
        .block(block)
        .style(Style::default().bg(p.bg))
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, panel);
    frame.render_widget(par, panel);
}

/// Vuelca el cuerpo revelando `revealed` caracteres en total a lo largo de las
/// líneas; en la frontera (si aún se está mecanografiando) coloca un cursor.
fn typed_body(
    body: &[String],
    revealed: usize,
    typing: bool,
    out: &mut Vec<Line<'static>>,
    critical: bool,
    p: Palette,
) {
    let text_color = if critical { p.brick } else { p.amber };
    let mut remaining = revealed;

    for (i, raw) in body.iter().enumerate() {
        let n = raw.chars().count();
        if remaining >= n {
            // Línea completa ya revelada.
            out.push(Line::from(Span::styled(
                raw.clone(),
                Style::default().fg(text_color),
            )));
            remaining -= n;
        } else {
            // Línea en curso: parte revelada + cursor + líneas pendientes vacías.
            let shown: String = raw.chars().take(remaining).collect();
            let mut spans = vec![Span::styled(shown, Style::default().fg(text_color))];
            if typing {
                spans.push(Span::styled(
                    "▌",
                    Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD),
                ));
            }
            out.push(Line::from(spans));
            for _ in (i + 1)..body.len() {
                out.push(Line::from(""));
            }
            return;
        }
    }
}

/// Dibuja el rollo de créditos de fin de campaña: un desplazamiento vertical
/// lento, centrado y a pantalla completa. El texto entra desde abajo, sube
/// deliberadamente y se funde en negro por arriba. Cualquier tecla lo salta.
fn draw_credits(
    frame: &mut Frame,
    area: Rect,
    eff: &Effect,
    lines: &[String],
    theme: &Theme,
    p: Palette,
) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(Style::default().bg(p.bg)), area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.amber_dim))
        .style(Style::default().bg(p.bg))
        .title(Span::styled(
            format!(" {} ", theme.app_title),
            Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let view_h = inner.height as usize;

    // Relleno superior = alto del viewport: a scroll 0 la pantalla está vacía y
    // el texto aguarda justo por debajo, listo para emerger desde abajo.
    let mut content: Vec<Line> = Vec::with_capacity(view_h + lines.len());
    for _ in 0..view_h {
        content.push(Line::from(""));
    }
    for raw in lines {
        content.push(credit_line(raw, p));
    }

    // El rollo sube y se detiene al asentar los créditos: si caben, el bloque
    // queda centrado en vertical (con el contacto legible); si no, sube hasta
    // dejar la cola hacia el centro. Después reposa sobre esa tarjeta final.
    let text_len = lines.len();
    let settle = if view_h > text_len {
        (view_h - (view_h - text_len) / 2) as u16
    } else {
        (view_h + text_len) as u16
    };
    let scroll = eff.credit_scroll().min(settle);

    let par = Paragraph::new(Text::from(content))
        .alignment(Alignment::Center)
        .style(Style::default().bg(p.bg))
        .scroll((scroll, 0));
    frame.render_widget(par, inner);
}

/// Estiliza una línea de los créditos: títulos en mayúsculas y contacto en
/// ámbar claro; las líneas de roles, atenuadas; el resto, ámbar principal.
fn credit_line(raw: &str, p: Palette) -> Line<'static> {
    let t = raw.trim();
    if t.is_empty() {
        return Line::from("");
    }
    let is_title = t.chars().any(|c| c.is_alphabetic())
        && t.chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase());
    let style = if is_title {
        Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD)
    } else if t.starts_with("Created by") {
        Style::default().fg(p.amber_hi).add_modifier(Modifier::BOLD)
    } else if t.contains('@') || t.starts_with("Contact") {
        Style::default().fg(p.amber_hi)
    } else if t.contains('/') {
        // Líneas de disciplinas / sistemas.
        Style::default().fg(p.amber_dim)
    } else {
        Style::default().fg(p.amber)
    };
    Line::from(Span::styled(raw.to_string(), style))
}

/// Calcula un `Rect` de tamaño `w`×`h` centrado dentro de `area`.
fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
