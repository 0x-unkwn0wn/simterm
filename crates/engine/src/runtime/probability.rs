//! Utilidades de probabilidad para el modelo de información imperfecta.

use rand::Rng;

/// Tira un dado entre 0.0 y 1.0 y devuelve true con probabilidad `prob`.
pub fn roll(prob: f32) -> bool {
    let p = prob.clamp(0.0, 1.0);
    rand::thread_rng().gen::<f32>() < p
}

/// Limita un valor al rango [0.0, 1.0].
pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Devuelve un f32 aleatorio uniforme en el rango dado.
pub fn range(min: f32, max: f32) -> f32 {
    if min >= max {
        return min;
    }
    rand::thread_rng().gen_range(min..max)
}

/// Elige un índice aleatorio en [0, len).
pub fn index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    rand::thread_rng().gen_range(0..len)
}
