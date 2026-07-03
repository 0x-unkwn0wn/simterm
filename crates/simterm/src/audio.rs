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
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::Source;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use simterm_engine::AssetSource;

/// Duración del fundido de entrada al arrancar la pista de una misión.
const FADE_IN: Duration = Duration::from_millis(1500);

pub struct Audio {
    /// El stream debe seguir vivo mientras suene algo: se conserva aunque no se
    /// use directamente.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    /// Sink de la pista actual; al reemplazarlo se detiene la anterior.
    sink: Option<Sink>,
    /// Fuente de pistas de la campaña (disco para campañas abiertas, memoria
    /// para campañas embebidas).
    source: AudioSource,
    /// Índice (0-based) de la misión cuya pista suena (evita reiniciarla).
    current: Option<usize>,
}

enum AudioSource {
    Dir(PathBuf),
    Assets(Arc<dyn AssetSource>),
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
            source: AudioSource::Dir(root),
            current: None,
        })
    }

    /// Prepara audio desde assets en memoria. Se usa para campañas embebidas:
    /// los WAV se descifran con el paquete y no se escriben a disco.
    pub fn try_new_assets(assets: Arc<dyn AssetSource>) -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        Some(Self {
            _stream: stream,
            handle,
            sink: None,
            source: AudioSource::Assets(assets),
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
        let logical = match track {
            Some(rel) => rel.to_string(),
            None => format!("music/mission_{}_theme.wav", level + 1),
        };
        self.sink = self.start(&logical);
    }

    /// Carga y arranca la reproducción en bucle de un fichero WAV.
    fn start(&self, path: &str) -> Option<Sink> {
        match &self.source {
            AudioSource::Dir(root) => {
                let file = File::open(root.join(Path::new(path))).ok()?;
                let decoder = Decoder::new(BufReader::new(file)).ok()?;
                self.start_decoded(decoder)
            }
            AudioSource::Assets(assets) => {
                let bytes = assets.read(path).ok()?;
                let decoder = Decoder::new(BufReader::new(Cursor::new(bytes))).ok()?;
                self.start_decoded(decoder)
            }
        }
    }

    fn start_decoded<R>(&self, decoder: Decoder<R>) -> Option<Sink>
    where
        R: std::io::Read + std::io::Seek + Send + Sync + 'static,
    {
        let sink = Sink::try_new(&self.handle).ok()?;
        // `buffered()` permite clonar la fuente para repetirla sin releer disco.
        // El `fade_in` va tras el bucle: solo funde el arranque, no cada vuelta.
        sink.append(decoder.buffered().repeat_infinite().fade_in(FADE_IN));
        Some(sink)
    }
}
