//! Un **medidor** genérico del runtime: un valor acumulable con suelo en 0.
//!
//! Es un primitivo neutral del núcleo, sin semántica de dominio. El pentesting lo
//! usa para la "traza"/detección (sube con el ruido, se pierde al llegar a un
//! umbral), pero cualquier otro dominio puede usarlo para lo suyo: combustible u
//! oxígeno de una nave, integridad de la cadena de custodia en forense, carga de
//! una batería... El *umbral* y qué pasa al alcanzarlo NO viven aquí: son
//! definición de cada misión/dominio.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meter {
    /// Valor acumulado actual (nunca baja de 0).
    pub value: f32,
    /// Total "activo" acumulado a lo largo del nivel (informativo). No incluye los
    /// incrementos pasivos (ver [`Meter::add_passive`]).
    pub total: f32,
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            value: 0.0,
            total: 0.0,
        }
    }
}

impl Meter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Crea un medidor con un valor inicial dado (nunca negativo).
    pub fn starting(value: f32) -> Self {
        Self {
            value: value.max(0.0),
            total: 0.0,
        }
    }

    /// Suma un incremento *activo* (cuenta para `total`). El pentesting lo usa
    /// para el ruido generado por una acción.
    pub fn add(&mut self, amount: f32) {
        let amount = amount.max(0.0);
        self.total += amount;
        self.value = (self.value + amount).max(0.0);
    }

    /// Sube el valor sin contarlo como incremento "activo" (no toca `total`). El
    /// pentesting lo usa para la exposición por permanencia (dwell).
    pub fn add_passive(&mut self, amount: f32) {
        self.value = (self.value + amount.max(0.0)).max(0.0);
    }

    /// Reduce el valor. Nunca baja de 0.
    pub fn reduce(&mut self, amount: f32) {
        self.value = (self.value - amount.max(0.0)).max(0.0);
    }

    /// ¿Se ha alcanzado el umbral dado?
    pub fn reached(&self, limit: f32) -> bool {
        self.value >= limit
    }

    /// Fracción 0.0..=1.0 respecto al umbral dado (para la barra de la UI).
    pub fn ratio(&self, limit: f32) -> f32 {
        (self.value / limit.max(1.0)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Meter;

    #[test]
    fn pasivo_no_cuenta_como_total_y_reduce_no_baja_de_cero() {
        let mut m = Meter::new();
        m.add_passive(5.0);
        assert_eq!(m.value, 5.0);
        assert_eq!(m.total, 0.0); // el incremento pasivo no cuenta en `total`
        m.reduce(3.0);
        assert_eq!(m.value, 2.0);
        m.reduce(100.0);
        assert_eq!(m.value, 0.0); // nunca baja de 0
    }
}
