//! Estado de detección/ruido (runtime). La detección sube con el ruido generado.
//!
//! El *umbral* a partir del cual se pierde es definición de cada misión, así que
//! vive fuera de aquí; este tipo solo acumula el ruido de la partida en curso.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionState {
    /// Nivel de detección acumulado.
    pub detection: f32,
    /// Ruido total generado a lo largo del nivel (informativo).
    pub total_noise: f32,
}

impl Default for DetectionState {
    fn default() -> Self {
        Self {
            detection: 0.0,
            total_noise: 0.0,
        }
    }
}

impl DetectionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade ruido. La detección sube directamente con el ruido generado.
    pub fn add_noise(&mut self, noise: f32) {
        let noise = noise.max(0.0);
        self.total_noise += noise;
        self.detection = (self.detection + noise).max(0.0);
    }

    /// Sube la detección por permanencia (dwell): no cuenta como "ruido
    /// generado", solo como exposición por el tiempo en el sistema.
    pub fn add_dwell(&mut self, amount: f32) {
        self.detection = (self.detection + amount.max(0.0)).max(0.0);
    }

    /// Reduce la detección (encubrimiento activo). Nunca baja de 0.
    pub fn reduce(&mut self, amount: f32) {
        self.detection = (self.detection - amount.max(0.0)).max(0.0);
    }

    /// ¿Se ha alcanzado el umbral dado?
    pub fn reached(&self, limit: f32) -> bool {
        self.detection >= limit
    }

    /// Fracción 0.0..=1.0 respecto al umbral dado (para la barra de la UI).
    pub fn ratio(&self, limit: f32) -> f32 {
        (self.detection / limit.max(1.0)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::DetectionState;

    #[test]
    fn dwell_no_cuenta_como_ruido_y_reduce_no_baja_de_cero() {
        let mut d = DetectionState::new();
        d.add_dwell(5.0);
        assert_eq!(d.detection, 5.0);
        assert_eq!(d.total_noise, 0.0); // el dwell no es "ruido generado"
        d.reduce(3.0);
        assert_eq!(d.detection, 2.0);
        d.reduce(100.0);
        assert_eq!(d.detection, 0.0); // nunca baja de 0
    }
}
