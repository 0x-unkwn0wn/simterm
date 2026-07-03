use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use flate2::write::ZlibEncoder;
use flate2::Compression;

fn main() {
    println!("cargo:rerun-if-env-changed=SIMTERM_EMBED_CAMPAIGN");
    println!("cargo:rerun-if-env-changed=SIMTERM_EMBED_KEY");
    println!("cargo:rerun-if-env-changed=SIMTERM_DISABLE_AUTOPLAY");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set"));
    let generated = out_dir.join("embedded_campaign.rs");

    // El empaquetado puede desactivar el autoplay para que el jugador no pueda
    // auto-resolver (spoilear) la campaña. Se hornea como constante en el binario.
    let autoplay_disabled = env::var_os("SIMTERM_DISABLE_AUTOPLAY").is_some();

    if env::var_os("CARGO_FEATURE_EMBED_CAMPAIGN").is_none() {
        fs::write(
            generated,
            "pub const AVAILABLE: bool = false;\npub const AUTOPLAY_DISABLED: bool = false;\npub const KEY: [u8; 32] = [0; 32];\npub const MANIFEST_NONCE: [u8; 12] = [0; 12];\npub const MANIFEST: &[u8] = &[];\npub const ASSETS: &[super::AssetBlob] = &[];\n",
        )
        .expect("write embedded stub");
        return;
    }

    let Some(root) = env::var_os("SIMTERM_EMBED_CAMPAIGN").map(PathBuf::from) else {
        panic!("feature 'embed-campaign' requires SIMTERM_EMBED_CAMPAIGN=<campaign directory>");
    };
    if !root.is_dir() {
        panic!(
            "SIMTERM_EMBED_CAMPAIGN is not a directory: {}",
            root.display()
        );
    }
    println!("cargo:rerun-if-changed={}", root.display());

    let mut files = collect_campaign_files(&root).expect("collect embedded campaign");
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let manifest_index = files
        .iter()
        .position(|(path, _)| path == "campaign.ron")
        .expect("campaign.ron not found in embedded campaign");
    let manifest_data = files.remove(manifest_index).1;

    let key = derive_or_make_key(
        env::var("SIMTERM_EMBED_KEY").ok().as_deref(),
        &manifest_data,
    );
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));

    let manifest_compressed = compress(&manifest_data).expect("compress embedded manifest");
    let manifest_nonce = make_nonce(&manifest_compressed, 0);
    let manifest_ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&manifest_nonce),
            manifest_compressed.as_slice(),
        )
        .expect("encrypt embedded manifest");
    let manifest_path = out_dir.join("campaign.ron.enc");
    fs::write(&manifest_path, manifest_ciphertext).expect("write encrypted manifest blob");

    let mut asset_entries = String::new();
    for (i, (path, data)) in files.into_iter().enumerate() {
        let compressed = compress(&data).expect("compress embedded asset");
        let nonce = make_nonce(&compressed, i as u64 + 1);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), compressed.as_slice())
            .expect("encrypt embedded asset");
        let blob_path = out_dir.join(format!("asset_{i}.enc"));
        fs::write(&blob_path, ciphertext).expect("write encrypted asset blob");
        asset_entries.push_str(&format!(
            "    super::AssetBlob {{ path: {:?}, nonce: {:?}, ciphertext: include_bytes!(r#\"{}\"#) }},\n",
            path,
            nonce,
            blob_path.display()
        ));
    }

    fs::write(
        generated,
        format!(
            "pub const AVAILABLE: bool = true;\npub const AUTOPLAY_DISABLED: bool = {};\npub const KEY: [u8; 32] = {:?};\npub const MANIFEST_NONCE: [u8; 12] = {:?};\npub const MANIFEST: &[u8] = include_bytes!(r#\"{}\"#);\npub const ASSETS: &[super::AssetBlob] = &[\n{}];\n",
            autoplay_disabled,
            key,
            manifest_nonce,
            manifest_path.display(),
            asset_entries
        ),
    )
    .expect("write embedded campaign module");
}

fn collect_campaign_files(root: &Path) -> io::Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            collect_files(root, &path, out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .expect("child path")
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, fs::read(path)?));
        }
    }
    Ok(())
}

fn compress(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    io::Write::write_all(&mut encoder, bytes)?;
    encoder.finish()
}

fn derive_or_make_key(configured: Option<&str>, package: &[u8]) -> [u8; 32] {
    let seed = match configured {
        Some(s) if !s.is_empty() => fnv64(s.as_bytes()),
        _ => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            now ^ fnv64(package)
        }
    };
    expand_seed(seed)
}

fn make_nonce(package: &[u8], index: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    let a = fnv64(package) ^ index.rotate_left(17);
    let b = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nonce[..8].copy_from_slice(&a.to_le_bytes());
    nonce[8..].copy_from_slice(&(b as u32).to_le_bytes());
    nonce
}

fn expand_seed(mut x: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    for chunk in out.chunks_mut(8) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        chunk.copy_from_slice(&x.to_le_bytes());
    }
    out
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
