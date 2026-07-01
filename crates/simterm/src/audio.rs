//! Subsistema de audio opcional del frontend.
//!
//! Reproduce la **pista** (música/ambiente) asociada a cada misión de la
//! campaña. Es deliberadamente tolerante a fallos: si no hay dispositivo de
//! sonido, falta el fichero o el formato no decodifica, el juego sigue **sin
//! audio** y nunca aborta por ello.
//!
//! Cada misión puede declarar su pista con el campo `music:` del `campaign.ron`
//! (ruta relativa a la campaña). Si no lo hace, se recurre a la convención por
//! nombre: `music/mission_{N}_theme.wav`. Las misiones sin ninguna de las dos
//! quedan en silencio. El motor no sabe nada de esto: el audio es una
//! preocupación exclusiva del frontend, igual que la TUI.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio::source::Source;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

/// Duración del fundido de entrada al arrancar la pista de una misión.
const FADE_IN: Duration = Duration::from_millis(1500);

pub struct Audio {
    /// El stream debe seguir vivo mientras suene algo: se conserva aunque no se
    /// use directamente.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    /// Sink de la pista actual; al reemplazarlo se detiene la anterior.
    sink: Option<Sink>,
    /// Directorio raíz de la campaña (las rutas de pista son relativas a él).
    root: PathBuf,
    /// Índice (0-based) de la misión cuya pista suena (evita reiniciarla).
    current: Option<usize>,
}

impl Audio {
    /// Intenta preparar el audio con `root` como directorio de la campaña (las
    /// rutas `music:` de cada misión son relativas a él). Devuelve `None` (y el
    /// juego va en silencio) si `root` no existe o no hay dispositivo de sonido.
    pub fn try_new(root: impl Into<PathBuf>) -> Option<Self> {
        let root = root.into();
        if !root.is_dir() {
            return None;
        }
        let (stream, handle) = OutputStream::try_default().ok()?;
        Some(Self {
            _stream: stream,
            handle,
            sink: None,
            root,
            current: None,
        })
    }

    /// Sincroniza la música con la misión indicada (0-based). `track` es la ruta
    /// declarada por la misión (`Mission.music`, relativa a la campaña); si es
    /// `None`, se usa la convención `music/mission_{N}_theme.wav`. Si ya suena la
    /// pista de esa misión no hace nada. Cualquier error se ignora en silencio.
    pub fn set_level(&mut self, level: usize, track: Option<&str>) {
        if self.current == Some(level) {
            return;
        }
        self.current = Some(level);
        let path = match track {
            Some(rel) => self.root.join(rel),
            None => self
                .root
                .join("music")
                .join(format!("mission_{}_theme.wav", level + 1)),
        };
        self.sink = self.start(&path);
    }

    /// Carga y arranca la reproducción en bucle de un fichero WAV.
    fn start(&self, path: &Path) -> Option<Sink> {
        let file = File::open(path).ok()?;
        let decoder = Decoder::new(BufReader::new(file)).ok()?;
        let sink = Sink::try_new(&self.handle).ok()?;
        // `buffered()` permite clonar la fuente para repetirla sin releer disco.
        // El `fade_in` va tras el bucle: solo funde el arranque, no cada vuelta.
        sink.append(decoder.buffered().repeat_infinite().fade_in(FADE_IN));
        Some(sink)
    }
}
