//! Constantes de balance del juego, centralizadas para tuneo fácil.
//!
//! Toda la "física" del juego (ruido, tiempo, probabilidades) vive aquí, en vez
//! de dispersa como números mágicos por `actions.rs`/`state.rs`. Ajustar la
//! dificultad es cambiar una constante, sin tocar la lógica.

// ----------------------------- Reconocimiento -----------------------------

/// `nmap`: tiempo y ruido de un escaneo activo.
pub const NMAP_TIME: u32 = 5;
pub const NMAP_NOISE: f32 = 4.0;
/// Ruido EXTRA al escanear con `nmap` un objetivo de entrada pasiva (sigilosa).
pub const PASSIVE_NMAP_PENALTY: f32 = 6.0;

/// `sniff`: interceptación pasiva (lenta pero casi muda).
pub const SNIFF_TIME: u32 = 8;
pub const SNIFF_NOISE: f32 = 1.0;

/// `connect`: pivote a través de un bastión de entrada.
pub const CONNECT_TIME: u32 = 5;
pub const CONNECT_NOISE: f32 = 2.0;

/// `netmap`: descubrimiento de hosts internos.
pub const NETMAP_TIME: u32 = 4;
pub const NETMAP_NOISE: f32 = 2.0;

/// `pivot`: salto entre hosts de la red interna.
pub const PIVOT_TIME: u32 = 3;

// ------------------------------ Investigación -----------------------------

/// `searchsploit`: tiempo, ruido y precisión de una lectura.
pub const RESEARCH_TIME: u32 = 8;
pub const RESEARCH_NOISE: f32 = 2.0;
pub const RESEARCH_ACCURACY: f32 = 0.78;
/// Cotas de la confianza por consenso (suavizado de Laplace).
pub const CONF_MIN: f32 = 0.10;
pub const CONF_MAX: f32 = 0.90;

// -------------------------------- Explotación -----------------------------

/// `exploit`: tiempo y pesos del cálculo de probabilidad de éxito.
pub const EXPLOIT_TIME: u32 = 15;
pub const EXPLOIT_BASE: f32 = 0.15;
pub const EXPLOIT_W_CONF: f32 = 0.45;
pub const EXPLOIT_W_SKILL: f32 = 0.30;
pub const EXPLOIT_W_DIFF: f32 = 0.55;
/// Ruido extra de un exploit fallido.
pub const EXPLOIT_FAIL_NOISE: f32 = 18.0;
/// Ruido de explotar un falso positivo (rebota y dispara alarmas).
pub const EXPLOIT_FALSEPOS_NOISE: f32 = 25.0;

/// `login`: foothold determinista por credencial reutilizada.
pub const LOGIN_TIME: u32 = 8;
pub const LOGIN_NOISE: f32 = 6.0;

// --------------------------------- Post-exploit ---------------------------

/// `privesc`: tiempo, ruido base y pesos de la escalada probabilística.
pub const PRIVESC_TIME: u32 = 10;
pub const PRIVESC_NOISE: f32 = 3.0;
pub const PRIVESC_BASE: f32 = 0.35;
pub const PRIVESC_W_SKILL: f32 = 0.40;
pub const PRIVESC_W_DIFF: f32 = 0.50;
/// Ruido extra de una escalada fallida.
pub const PRIVESC_FAIL_NOISE: f32 = 8.0;

/// `cleanup`: encubrimiento activo.
pub const CLEANUP_TIME: u32 = 8;
pub const CLEANUP_REDUCTION: f32 = 14.0;
pub const CLEANUP_BACKFIRE: f32 = 5.0;
/// Probabilidad de éxito del primer `cleanup` del nivel, y su caída por uso.
pub const CLEANUP_BASE_P: f32 = 0.85;
pub const CLEANUP_P_DECAY: f32 = 0.15;
pub const CLEANUP_MIN_P: f32 = 0.30;

// ------------------------- Trabajo offline (avanzado) ---------------------
//
// Cracking de hashes, reversing y decodificación son trabajo LOCAL: gastan
// reloj pero no ruido de red (no tocas el objetivo). Modelan análisis offline.

/// `john`/`hashcat`: cracking offline de un hash saqueado.
pub const JOHN_TIME: u32 = 20;
pub const JOHN_BASE: f32 = 0.35;
pub const JOHN_W_SKILL: f32 = 0.40;
pub const JOHN_W_STRENGTH: f32 = 0.70;
/// Bonus de probabilidad por tener un wordlist saqueado.
pub const JOHN_WORDLIST_BONUS: f32 = 0.30;

/// `strings`/`disasm`/`solve`: reversing de un binario (rápido, offline).
pub const REV_TIME: u32 = 6;

/// `base64`/`xor`: decodificación de un fichero (trivial, offline).
pub const DECODE_TIME: u32 = 2;

/// `linpeas`/`sudo -l`/`suid`/`sysinfo`: enumeración local en POST. Poco ruido
/// (estás dentro), revela el vector de escalada del host.
pub const LOCALENUM_TIME: u32 = 6;
pub const LOCALENUM_NOISE: f32 = 1.5;

// ----------------------------------- Traza --------------------------------

/// Traza por permanencia (dwell) por cada tick de reloj en fases activas.
pub const DWELL_RATE: f32 = 0.1;

// ------------------------- Defensa activa (blue team) ---------------------
//
// En hosts `reactive`, el equipo de seguridad responde por etapas según la
// fracción de traza alcanzada. Cada etapa, al cruzarse por primera vez:
//   - suma una penalización permanente a la prob. de `exploit`/`privesc`
//     (han endurecido el sistema y rotado credenciales), y
//   - puede inyectar un golpe de ruido inmediato (te están rastreando).
// La ruta SEGURA (llave de privesc) sigue siendo inmune: es acceso legítimo.

/// Etapas de respuesta: (umbral de traza, penalización a la prob., ruido extra).
pub const DEFENSE_STAGES: [(f32, f32, f32); 3] = [
    (0.35, 0.08, 0.0),  // RASTREO: te correlacionan; cuesta más explotar
    (0.60, 0.10, 8.0),  // CONTRAMEDIDAS: endurecen y aceleran la traza
    (0.82, 0.12, 14.0), // PURGA: cierran el cerco
];
