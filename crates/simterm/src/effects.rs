//! Efectos visuales superpuestos a la interfaz: contenidos y justificados.
//!
//! Estética sobria (CRT ámbar): nada de glitch, ruido ni scanlines. Solo dos
//! recursos:
//!   - **Typewriter**: los briefings y mensajes narrativos se revelan carácter a
//!     carácter, lentos y deliberados, sobre un panel sobrio.
//!   - **Parpadeo lento**: exclusivo de las alertas críticas (fin de operación
//!     por traza), en rojo ladrillo.
//!
//! Son puramente estéticos: el estado del juego ya ha cambiado por debajo. Todo
//! el TEXTO mostrado aquí (boot, créditos, alertas) procede de la campaña: este
//! módulo solo define la mecánica de animación, no el contenido.

/// Frames de "reposo" tras revelar todo el texto, para que el panel no
/// desaparezca en el instante en que se teclea el último carácter.
const HOLD_FRAMES: u16 = 36;
/// Cota inferior de duración para mensajes muy cortos.
const MIN_FRAMES: u16 = 24;

/// Frames por cada fila que sube el rollo de créditos (a ~45 ms/frame, una fila
/// cada ~315 ms: un desplazamiento lento y deliberado, de cine).
pub const CREDIT_ROW_FRAMES: u16 = 7;
/// Alto de viewport asumido al dimensionar la duración del rollo (sin conocer
/// aún el tamaño real del terminal). Cota superior holgada: el rollo se detiene
/// al asentar los créditos centrados (ver `ui::draw_credits`), así que si el
/// terminal real es más bajo, simplemente reposa más tiempo en esa tarjeta
/// final; si es más alto, la alcanza por los pelos.
const CREDIT_ASSUMED_H: u16 = 55;
/// Frames de reposo sobre la tarjeta final asentada, antes de devolver el control.
const CREDIT_HOLD: u16 = 40;

#[derive(Debug, Clone)]
pub enum EffectKind {
    /// Secuencia de arranque al iniciar la sesión (texto de la campaña).
    Boot { header: String, lines: Vec<String> },
    /// Documento de misión al abrir una operación (typewriter).
    Briefing {
        number: usize,
        total: usize,
        name: String,
        lines: Vec<String>,
    },
    /// Cierre de operación / fin de campaña (typewriter).
    Debrief {
        name: String,
        lines: Vec<String>,
        /// Si es la última operación de la campaña.
        final_op: bool,
    },
    /// Traza al límite: operación abortada. Alerta crítica (parpadeo, ladrillo).
    Aborted { lines: Vec<String> },
    /// Créditos de fin de campaña: rollo vertical cinemático (ver `ui::draw_credits`).
    Credits { lines: Vec<String> },
}

impl EffectKind {
    /// Encabezado fijo del panel (no se mecanografía: se muestra de inmediato).
    pub fn header(&self) -> String {
        match self {
            EffectKind::Boot { header, .. } => header.clone(),
            EffectKind::Briefing {
                number,
                total,
                name,
                ..
            } => {
                format!("BRIEFING · OPERACIÓN {number}/{total} · {name}")
            }
            EffectKind::Debrief { name, final_op, .. } => {
                if *final_op {
                    String::from("FIN DE CAMPAÑA")
                } else {
                    format!("DEBRIEF · {name}")
                }
            }
            EffectKind::Aborted { .. } => String::from("** OPERACIÓN ABORTADA **"),
            EffectKind::Credits { .. } => String::from("CRÉDITOS"),
        }
    }

    /// Cuerpo del panel: las líneas que se revelan con el typewriter.
    pub fn body(&self) -> Vec<String> {
        match self {
            EffectKind::Boot { lines, .. } => lines.clone(),
            EffectKind::Briefing { lines, .. } => lines.clone(),
            EffectKind::Debrief { lines, .. } => lines.clone(),
            EffectKind::Aborted { lines } => lines.clone(),
            EffectKind::Credits { lines } => lines.clone(),
        }
    }

    /// ¿Es una alerta crítica? (parpadeo lento + rojo ladrillo).
    pub fn is_critical(&self) -> bool {
        matches!(self, EffectKind::Aborted { .. })
    }
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub kind: EffectKind,
    pub frame: u16,
    pub max: u16,
    /// Nº total de caracteres mecanografiables (cuerpo del panel).
    pub body_chars: u16,
}

impl Effect {
    /// Construye un efecto cuya duración se ajusta a la longitud del texto, de
    /// modo que el typewriter avance ~1 carácter por frame y luego repose.
    pub fn typed(kind: EffectKind) -> Self {
        let body_chars: usize = kind.body().iter().map(|l| l.chars().count()).sum();
        let body_chars = body_chars as u16;
        let max = (body_chars + HOLD_FRAMES).max(MIN_FRAMES);
        Self {
            kind,
            frame: 0,
            max,
            body_chars,
        }
    }

    /// Construye el rollo de créditos: un desplazamiento vertical lento cuyo
    /// número de frames se calcula para que el texto entre desde abajo, suba
    /// hasta salir por arriba y repose unos instantes en negro.
    pub fn credits(lines: Vec<String>) -> Self {
        // Filas hasta asentar los créditos centrados ≈ medio viewport + el texto.
        let travel = CREDIT_ASSUMED_H / 2 + lines.len() as u16;
        let max = travel
            .saturating_mul(CREDIT_ROW_FRAMES)
            .saturating_add(CREDIT_HOLD);
        Self {
            kind: EffectKind::Credits { lines },
            frame: 0,
            max,
            body_chars: 0,
        }
    }

    /// Fila (offset de scroll) por la que va el rollo de créditos según el frame.
    pub fn credit_scroll(&self) -> u16 {
        self.frame / CREDIT_ROW_FRAMES
    }

    pub fn done(&self) -> bool {
        self.frame >= self.max
    }

    /// Nº de caracteres del cuerpo ya revelados según el frame actual.
    pub fn revealed(&self) -> usize {
        self.frame.min(self.body_chars) as usize
    }

    /// ¿Está aún mecanografiando (no ha revelado todo el cuerpo)?
    pub fn typing(&self) -> bool {
        self.frame < self.body_chars
    }
}
