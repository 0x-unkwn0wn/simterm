//! Sistema de archivos ficticio (VFS) de un nodo objetivo.
//!
//! Capa de **definición** (un árbol de `FsNode` que se carga desde RON dentro de
//! cada `TargetNode`) + **navegación pura** (funciones sin efectos que resuelven
//! rutas y devuelven datos en propiedad). Los efectos de juego (recoger botín,
//! completar el objetivo) los orquesta `runtime::actions` con lo que devuelven.
//!
//! Aquí también vive el soporte de **lore**: cada fichero puede llevar líneas de
//! `content` (texto narrativo) y, opcionalmente, `loot` (botín con efecto).

use serde::{Deserialize, Serialize};

/// Un nodo del árbol: directorio o fichero.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FsNode {
    Dir {
        name: String,
        #[serde(default)]
        children: Vec<FsNode>,
    },
    File {
        name: String,
        /// Contenido del fichero (lore / pistas). Una entrada por línea.
        #[serde(default)]
        content: Vec<String>,
        /// Si es true, hace falta root para leerlo.
        #[serde(default)]
        root: bool,
        /// Botín que otorga la primera vez que se lee.
        #[serde(default)]
        loot: Option<Loot>,
        /// Si está presente, el fichero es un **binario reversible**: `cat` no
        /// sirve; hay que usar `strings`/`disasm` y extraer el secreto con `solve`.
        #[serde(default)]
        binary: Option<Binary>,
        /// Si está presente, el `content` está **codificado**: `cat` muestra el
        /// blob; hay que decodificarlo (`base64`/`xor`) para leer el claro.
        #[serde(default)]
        encoding: Option<Encoding>,
    },
}

/// Botín de un fichero: efecto mecánico (skill) + información (credencial/nota).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loot {
    /// Bonus de habilidad que persiste durante el resto de la campaña.
    #[serde(default)]
    pub skill: f32,
    /// Credencial saqueada (se guarda en el inventario; sabor/narrativa).
    #[serde(default)]
    pub credential: Option<String>,
    /// Nota o pista mostrada al recogerlo.
    #[serde(default)]
    pub note: Option<String>,
    /// Si es `true`, este botín es la llave/credencial local del nivel: al
    /// recogerlo, `privesc` pasa a ser DETERMINISTA (ruta segura, sin RNG).
    /// Los ficheros con esta marca deben ser legibles sin root (son el vector
    /// que CONCEDE root). Por defecto `false`: el botín no altera la escalada.
    #[serde(default)]
    pub privesc_key: bool,
    /// Token de credencial reutilizable: si un nivel posterior la acepta
    /// (`TargetNode.accepts_token`), permite un foothold determinista con
    /// `login` (sin pasar por el `exploit` probabilístico).
    #[serde(default)]
    pub foothold_token: Option<String>,
    /// Hash saqueado, crackeable offline con `john` para obtener su `yields`.
    #[serde(default)]
    pub hash: Option<LootHash>,
    /// Si es `true`, leer este fichero aporta un **wordlist** (tipo rockyou):
    /// habilita romper hashes que lo requieren. Persiste en la campaña.
    #[serde(default)]
    pub wordlist: bool,
}

/// Recompensa mecánica que otorga romper un hash (`john`) o resolver un binario
/// (`solve`). Unifica los efectos posibles con los del botín normal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Reward {
    /// Bonus de habilidad permanente.
    Skill(f32),
    /// Credencial saqueada (inventario/sabor).
    Credential(String),
    /// Token de credencial reutilizable (como `foothold_token`).
    Token(String),
    /// Habilita la escalada determinista (como `privesc_key`).
    PrivescKey,
}

fn default_strength() -> u8 {
    5
}

/// Hash saqueado, **crackeable offline** con `john`/`hashcat`. El ataque gasta
/// reloj pero no ruido de red (es local). `strength` alto + `needs_wordlist`
/// modela hashes inviables sin un diccionario adecuado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootHash {
    /// Algoritmo (sabor): "sha512crypt", "ntlm", "bcrypt"...
    pub algo: String,
    /// Dureza 1..=10 (a mayor dureza, menor probabilidad por intento).
    #[serde(default = "default_strength")]
    pub strength: u8,
    /// Si es `true`, sin un wordlist saqueado el ataque es inviable.
    #[serde(default)]
    pub needs_wordlist: bool,
    /// Qué se obtiene al romperlo.
    pub yields: Reward,
}

/// Binario **reversible**: el reto es leer las cadenas/desensamblado (autorados
/// por la campaña) y extraer el secreto embebido, que se entrega con `solve`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binary {
    /// Salida de `strings` (cadenas imprimibles; algunas señuelo).
    #[serde(default)]
    pub strings: Vec<String>,
    /// Salida de `disasm` (pseudo-desensamblado/decompilado).
    #[serde(default)]
    pub disasm: Vec<String>,
    /// Secreto a extraer (se compara sin distinguir mayúsculas).
    pub secret: String,
    /// Recompensa al resolverlo.
    pub yields: Reward,
    /// Pista opcional (se muestra con `strings`).
    #[serde(default)]
    pub hint: Option<String>,
}

/// Codificación del `content` de un fichero. El claro lo escribe la campaña; el
/// motor lo muestra codificado en `cat` y revela el claro al decodificar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Encoding {
    /// Base64: se decodifica con `base64 <fichero>`.
    Base64,
    /// XOR con clave (texto): se decodifica con `xor <fichero> <clave>`.
    Xor(String),
}

impl FsNode {
    pub fn name(&self) -> &str {
        match self {
            FsNode::Dir { name, .. } => name,
            FsNode::File { name, .. } => name,
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, FsNode::Dir { .. })
    }

    pub fn children(&self) -> Option<&[FsNode]> {
        match self {
            FsNode::Dir { children, .. } => Some(children),
            FsNode::File { .. } => None,
        }
    }
}

// --------------------------- Rutas y resolución ---------------------------

/// Normaliza una ruta (absoluta o relativa a `cwd`) a una lista de componentes.
pub fn normalize(cwd: &[String], input: &str) -> Vec<String> {
    let mut comps: Vec<String> = if input.starts_with('/') {
        Vec::new()
    } else {
        cwd.to_vec()
    };
    for seg in input.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                comps.pop();
            }
            s => comps.push(s.to_string()),
        }
    }
    comps
}

/// Representación textual de una ruta a partir de sus componentes.
pub fn path_string(comps: &[String]) -> String {
    if comps.is_empty() {
        String::from("/")
    } else {
        format!("/{}", comps.join("/"))
    }
}

/// Localiza el nodo correspondiente a `comps` partiendo de la raíz `root`.
/// `comps` vacío representa la raíz (devuelve `None`, tratada aparte).
fn find_node<'a>(root: &'a [FsNode], comps: &[String]) -> Option<&'a FsNode> {
    let mut children = root;
    let mut node = None;
    for (i, c) in comps.iter().enumerate() {
        let found = children.iter().find(|n| n.name() == c)?;
        node = Some(found);
        match found.children() {
            Some(ch) => children = ch,
            None => {
                // Es un fichero: solo válido si es el último componente.
                if i + 1 != comps.len() {
                    return None;
                }
            }
        }
    }
    node
}

/// Lista las entradas `(nombre, es_dir)` del directorio en `comps`
/// (`comps` vacío = raíz). Devuelve `None` si la ruta no es un directorio.
/// Pensado para el autocompletado de rutas.
pub fn dir_children(root: &[FsNode], comps: &[String]) -> Option<Vec<(String, bool)>> {
    let children = if comps.is_empty() {
        root
    } else {
        match find_node(root, comps) {
            Some(FsNode::Dir { children, .. }) => children.as_slice(),
            _ => return None,
        }
    };
    Some(
        children
            .iter()
            .map(|n| (n.name().to_string(), n.is_dir()))
            .collect(),
    )
}

// --------------------------- Operaciones (puras) ---------------------------

/// Resultado de listar una ruta.
pub enum ListOutcome {
    /// Entradas de un directorio (ya formateadas con marcadores).
    Dir(Vec<String>),
    /// La ruta era un fichero (se muestra su nombre).
    File(String),
    NotFound,
}

pub fn list_entries(root: &[FsNode], comps: &[String]) -> ListOutcome {
    if comps.is_empty() {
        return ListOutcome::Dir(entries(root));
    }
    match find_node(root, comps) {
        Some(FsNode::Dir { children, .. }) => ListOutcome::Dir(entries(children)),
        Some(FsNode::File { name, root: r, .. }) => {
            ListOutcome::File(format_entry(name, false, *r))
        }
        None => ListOutcome::NotFound,
    }
}

fn entries(children: &[FsNode]) -> Vec<String> {
    children
        .iter()
        .map(|n| match n {
            FsNode::Dir { name, .. } => format_entry(name, true, false),
            FsNode::File { name, root, .. } => format_entry(name, false, *root),
        })
        .collect()
}

fn format_entry(name: &str, is_dir: bool, root: bool) -> String {
    if is_dir {
        format!("{name}/")
    } else if root {
        format!("{name}   [root]")
    } else {
        name.to_string()
    }
}

/// Resultado de leer un fichero.
pub enum ReadOutcome {
    File {
        content: Vec<String>,
        root: bool,
        loot: Option<Loot>,
        /// Codificación del contenido (si la hay): `cat` muestra el blob.
        encoding: Option<Encoding>,
        /// Si es `true`, es un binario: `cat` no sirve (usa `strings`/`disasm`).
        is_binary: bool,
    },
    IsDir,
    NotFound,
}

pub fn read_file(root: &[FsNode], comps: &[String]) -> ReadOutcome {
    if comps.is_empty() {
        return ReadOutcome::IsDir;
    }
    match find_node(root, comps) {
        Some(FsNode::File {
            content,
            root: r,
            loot,
            binary,
            encoding,
            ..
        }) => ReadOutcome::File {
            content: content.clone(),
            root: *r,
            loot: loot.clone(),
            encoding: encoding.clone(),
            is_binary: binary.is_some(),
        },
        Some(FsNode::Dir { .. }) => ReadOutcome::IsDir,
        None => ReadOutcome::NotFound,
    }
}

/// Devuelve el binario reversible del fichero en `comps`, si lo es.
pub fn file_binary(root: &[FsNode], comps: &[String]) -> Option<Binary> {
    match find_node(root, comps) {
        Some(FsNode::File { binary, .. }) => binary.clone(),
        _ => None,
    }
}

/// Devuelve el hash crackeable del botín del fichero en `comps`, si lo tiene.
pub fn file_hash(root: &[FsNode], comps: &[String]) -> Option<LootHash> {
    match find_node(root, comps) {
        Some(FsNode::File { loot: Some(l), .. }) => l.hash.clone(),
        _ => None,
    }
}

// ------------------------------ Codificación ------------------------------

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Codifica bytes en Base64 (estándar, con relleno).
fn base64_encode(input: &[u8]) -> String {
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            B64[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// XOR de unos bytes con una clave (repetida), devuelto en hex.
fn xor_hex(input: &[u8], key: &[u8]) -> String {
    if key.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(input.len() * 2);
    for (i, b) in input.iter().enumerate() {
        let x = b ^ key[i % key.len()];
        out.push_str(&format!("{x:02x}"));
    }
    out
}

/// Representación **codificada** del contenido (lo que muestra `cat`). El claro
/// lo escribe la campaña; aquí se ofusca para que haga falta `base64`/`xor`.
pub fn encode_display(content: &[String], enc: &Encoding) -> Vec<String> {
    let joined = content.join("\n");
    match enc {
        Encoding::Base64 => {
            // Troceado en líneas de 64 para que parezca un volcado real.
            let b64 = base64_encode(joined.as_bytes());
            b64.as_bytes()
                .chunks(64)
                .map(|c| String::from_utf8_lossy(c).into_owned())
                .collect()
        }
        Encoding::Xor(key) => vec![xor_hex(joined.as_bytes(), key.as_bytes())],
    }
}

/// Resultado de intentar decodificar un fichero con `base64`/`xor`.
pub enum DecodeOutcome {
    /// Decodificado: contenido en claro.
    Ok(Vec<String>),
    /// La herramienta no corresponde a la codificación del fichero.
    WrongTool,
    /// XOR con clave equivocada.
    WrongKey,
    /// El fichero no está codificado.
    NotEncoded,
}

/// Intenta decodificar el fichero en `comps`. `tool` es "base64" o "xor";
/// `key` es la clave para XOR (ignorada en base64).
pub fn decode_file(
    root: &[FsNode],
    comps: &[String],
    tool: &str,
    key: Option<&str>,
) -> DecodeOutcome {
    let (content, enc) = match find_node(root, comps) {
        Some(FsNode::File {
            content,
            encoding: Some(e),
            ..
        }) => (content.clone(), e.clone()),
        Some(FsNode::File { .. }) => return DecodeOutcome::NotEncoded,
        _ => return DecodeOutcome::NotEncoded,
    };
    match (tool, &enc) {
        ("base64", Encoding::Base64) => DecodeOutcome::Ok(content),
        ("xor", Encoding::Xor(k)) => {
            if key == Some(k.as_str()) {
                DecodeOutcome::Ok(content)
            } else {
                DecodeOutcome::WrongKey
            }
        }
        _ => DecodeOutcome::WrongTool,
    }
}

/// ¿Es la ruta un directorio válido? (para `cd`).
pub enum DirCheck {
    Ok,
    NotADir,
    NotFound,
}

pub fn is_dir(root: &[FsNode], comps: &[String]) -> DirCheck {
    if comps.is_empty() {
        return DirCheck::Ok;
    }
    match find_node(root, comps) {
        Some(n) if n.is_dir() => DirCheck::Ok,
        Some(_) => DirCheck::NotADir,
        None => DirCheck::NotFound,
    }
}

/// Busca rutas cuyo nombre contenga `needle` (todas si es `None`).
pub fn search(root: &[FsNode], needle: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut path = Vec::new();
    walk(root, &mut path, needle, &mut out);
    out
}

fn walk(children: &[FsNode], path: &mut Vec<String>, needle: Option<&str>, out: &mut Vec<String>) {
    for n in children {
        path.push(n.name().to_string());
        let hit = match needle {
            Some(s) => n.name().to_lowercase().contains(&s.to_lowercase()),
            None => true,
        };
        if hit {
            let mark = if n.is_dir() { "/" } else { "" };
            out.push(format!("{}{}", path_string(path), mark));
        }
        if let Some(ch) = n.children() {
            walk(ch, path, needle, out);
        }
        path.pop();
    }
}
