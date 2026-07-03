use std::collections::HashMap;
use std::fmt;
use std::io;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use flate2::read::ZlibDecoder;
use simterm_engine::{AssetSource, Campaign};

mod generated {
    include!(concat!(env!("OUT_DIR"), "/embedded_campaign.rs"));
}

pub struct EmbeddedCampaign {
    pub campaign: Campaign,
    pub assets: EmbeddedAssetSource,
}

#[derive(Debug)]
pub struct AssetBlob {
    pub path: &'static str,
    pub nonce: [u8; 12],
    pub ciphertext: &'static [u8],
}

#[derive(Debug, Clone)]
pub struct EmbeddedAssetSource {
    files: HashMap<&'static str, &'static AssetBlob>,
}

#[derive(Debug)]
pub enum EmbeddedError {
    NotAvailable,
    Decrypt,
    Format(&'static str),
    Io,
    Utf8(std::string::FromUtf8Error),
    Parse(String),
}

impl fmt::Display for EmbeddedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbeddedError::NotAvailable => write!(f, "no hay campaña embebida en este binario"),
            EmbeddedError::Decrypt => write!(f, "no pude descifrar la campaña embebida"),
            EmbeddedError::Format(msg) => write!(f, "paquete embebido inválido: {msg}"),
            EmbeddedError::Io => write!(f, "no pude descomprimir la campaña embebida"),
            EmbeddedError::Utf8(err) => write!(f, "campaign.ron embebido no es UTF-8: {err}"),
            EmbeddedError::Parse(err) => {
                write!(f, "no pude interpretar campaign.ron embebido: {err}")
            }
        }
    }
}

impl std::error::Error for EmbeddedError {}

pub fn available() -> bool {
    generated::AVAILABLE
}

/// ¿El empaquetado desactivó el autoplay? Si es `true`, el binario ignora
/// cualquier `--autoplay*` para que el jugador no pueda spoilear la campaña.
/// Siempre `false` en compilaciones normales (sin campaña embebida).
pub fn autoplay_disabled() -> bool {
    generated::AUTOPLAY_DISABLED
}

pub fn load() -> Result<EmbeddedCampaign, EmbeddedError> {
    if !generated::AVAILABLE {
        return Err(EmbeddedError::NotAvailable);
    }

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&generated::KEY));
    let compressed_manifest = cipher
        .decrypt(
            Nonce::from_slice(&generated::MANIFEST_NONCE),
            generated::MANIFEST,
        )
        .map_err(|_| EmbeddedError::Decrypt)?;

    let manifest = decompress(&compressed_manifest).map_err(|_| EmbeddedError::Io)?;
    let text = String::from_utf8(manifest).map_err(EmbeddedError::Utf8)?;
    let campaign: Campaign =
        ron::de::from_str(&text).map_err(|err| EmbeddedError::Parse(err.to_string()))?;
    if campaign.missions.is_empty() {
        return Err(EmbeddedError::Format("campaign.ron no contiene misiones"));
    }

    let assets = EmbeddedAssetSource::new(generated::ASSETS);
    Ok(EmbeddedCampaign { campaign, assets })
}

impl EmbeddedAssetSource {
    fn new(blobs: &'static [AssetBlob]) -> Self {
        let files = blobs.iter().map(|blob| (blob.path, blob)).collect();
        Self { files }
    }
}

impl AssetSource for EmbeddedAssetSource {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let blob = self.files.get(path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("asset no encontrado: '{path}'"),
            )
        })?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&generated::KEY));
        let compressed = cipher
            .decrypt(Nonce::from_slice(&blob.nonce), blob.ciphertext)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "asset cifrado inválido"))?;
        decompress(&compressed)
    }

    fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }
}

fn decompress(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut out)?;
    Ok(out)
}
