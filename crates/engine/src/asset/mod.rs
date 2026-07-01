//! Acceso a los **assets** de una campaña (música, textos sueltos, cualquier
//! binario) por una **ruta lógica**, sin que el frontend sepa de dónde salen.
//!
//! Una campaña abierta (directorio) sirve sus assets desde disco
//! ([`DirAssetSource`]); una campaña empaquetada (`.rtpack`) los sirve desde
//! memoria tras descifrar ([`MemAssetSource`]). El frontend solo ve
//! [`AssetSource`].
//!
//! La ruta lógica es siempre relativa al directorio de la campaña y usa `/`
//! como separador (p.ej. `"audio/intro.ogg"`).

use std::collections::HashMap;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Fuente de assets de una campaña: resuelve una ruta lógica a sus bytes.
pub trait AssetSource: Send + Sync {
    /// Lee el asset en `path` (ruta lógica con `/`). Error si no existe.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;

    /// `true` si existe un asset en `path`.
    fn contains(&self, path: &str) -> bool {
        self.read(path).is_ok()
    }
}

/// Sin assets. Útil como default para campañas que no traen contenido suelto.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoAssets;

impl AssetSource for NoAssets {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("esta campaña no trae assets (se pidió '{path}')"),
        ))
    }
    fn contains(&self, _path: &str) -> bool {
        false
    }
}

/// Assets servidos desde un directorio en disco (campañas abiertas / desarrollo).
#[derive(Debug, Clone)]
pub struct DirAssetSource {
    root: PathBuf,
}

impl DirAssetSource {
    /// Sirve assets desde `root` (el directorio de la campaña).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl AssetSource for DirAssetSource {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let safe = sanitize(path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("ruta de asset no permitida: '{path}'"),
            )
        })?;
        std::fs::read(self.root.join(safe))
    }
}

/// Assets en memoria (descifrados desde un `.rtpack`).
#[derive(Debug, Clone, Default)]
pub struct MemAssetSource {
    files: HashMap<String, Vec<u8>>,
}

impl MemAssetSource {
    /// Crea una fuente vacía.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserta (o reemplaza) un asset bajo su ruta lógica.
    pub fn insert(&mut self, path: impl Into<String>, data: Vec<u8>) {
        self.files.insert(path.into(), data);
    }

    /// Número de assets almacenados.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// `true` si no hay ningún asset.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl AssetSource for MemAssetSource {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.files.get(path).cloned().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("asset no encontrado: '{path}'"),
            )
        })
    }
    fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }
}

/// Convierte una ruta lógica en una ruta relativa segura (sin escapar del root
/// con `..`, sin raíz absoluta). Devuelve `None` si es sospechosa.
fn sanitize(path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            // Cualquier intento de subir de directorio o ruta absoluta se rechaza.
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_source_roundtrip() {
        let mut s = MemAssetSource::new();
        s.insert("audio/intro.ogg", vec![1, 2, 3]);
        assert!(s.contains("audio/intro.ogg"));
        assert_eq!(s.read("audio/intro.ogg").unwrap(), vec![1, 2, 3]);
        assert!(s.read("nope").is_err());
    }

    #[test]
    fn sanitize_rejects_traversal() {
        assert!(sanitize("../secret").is_none());
        assert!(sanitize("/etc/passwd").is_none());
        assert!(sanitize("audio/intro.ogg").is_some());
        assert!(
            sanitize("./a/./b").map(|p| p.to_string_lossy().replace('\\', "/"))
                == Some("a/b".into())
        );
    }
}
