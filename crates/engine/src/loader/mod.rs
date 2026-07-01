//! Carga de campañas desde disco.
//!
//! El motor interpreta cualquier campaña que siga el formato de datos
//! (ver `docs/CAMPAIGN_FORMAT.md`). Una campaña es:
//!
//!   - un **fichero** `.ron` con la estructura `Campaign`, o
//!   - un **directorio** que contiene `campaign.ron`.
//!
//! No hay nada de contenido incrustado: si la ruta no existe o el RON es
//! inválido, se devuelve un error explicativo (el frontend decide qué hacer).

use std::fmt;
use std::path::{Path, PathBuf};

use crate::asset::{AssetSource, DirAssetSource};
use crate::model::campaign::Campaign;

/// Nombre del manifiesto dentro de un directorio de campaña.
pub const MANIFEST: &str = "campaign.ron";

/// Error de carga de una campaña (con contexto para el usuario).
#[derive(Debug)]
pub enum LoadError {
    /// La ruta no existe o no se pudo leer.
    NotFound {
        path: PathBuf,
        source: std::io::Error,
    },
    /// El RON no se pudo interpretar como una `Campaign`.
    Parse { path: PathBuf, message: String },
    /// La campaña se cargó pero no contiene misiones.
    Empty { path: PathBuf },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound { path, source } => {
                write!(
                    f,
                    "no pude leer la campaña en '{}': {source}",
                    path.display()
                )
            }
            LoadError::Parse { path, message } => {
                write!(f, "no pude interpretar '{}': {message}", path.display())
            }
            LoadError::Empty { path } => {
                write!(f, "la campaña '{}' no contiene misiones", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// Resuelve la ruta `input` al fichero de manifiesto a cargar: si es un
/// directorio, devuelve `<dir>/campaign.ron`; si es un fichero, lo devuelve tal
/// cual.
pub fn resolve_manifest(input: &Path) -> PathBuf {
    if input.is_dir() {
        input.join(MANIFEST)
    } else {
        input.to_path_buf()
    }
}

/// Carga e interpreta una campaña desde una ruta (directorio o fichero `.ron`).
pub fn load_campaign(input: impl AsRef<Path>) -> Result<Campaign, LoadError> {
    let manifest = resolve_manifest(input.as_ref());

    let text = std::fs::read_to_string(&manifest).map_err(|source| LoadError::NotFound {
        path: manifest.clone(),
        source,
    })?;

    let campaign: Campaign = ron::de::from_str(&text).map_err(|e| LoadError::Parse {
        path: manifest.clone(),
        message: e.to_string(),
    })?;

    if campaign.missions.is_empty() {
        return Err(LoadError::Empty { path: manifest });
    }

    Ok(campaign)
}

/// Una campaña cargada **junto con su fuente de assets** (música, etc.).
///
/// Es lo que el frontend necesita para reproducir audio u otros recursos sueltos
/// con independencia de si la campaña venía de un directorio abierto o de un
/// `.rtpack` cifrado.
pub struct OpenCampaign {
    /// La campaña interpretada.
    pub campaign: Campaign,
    /// De dónde leer sus assets.
    pub assets: Box<dyn AssetSource>,
}

/// Carga una campaña **abierta** (directorio o fichero `.ron`) y expone sus
/// assets desde el propio directorio.
pub fn load_open_campaign(input: impl AsRef<Path>) -> Result<OpenCampaign, LoadError> {
    let input = input.as_ref();
    let campaign = load_campaign(input)?;
    // Los assets cuelgan del directorio de la campaña (o del que contiene el .ron).
    let root = if input.is_dir() {
        input.to_path_buf()
    } else {
        input
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    };
    Ok(OpenCampaign {
        campaign,
        assets: Box::new(DirAssetSource::new(root)),
    })
}
