//! Definición (datos) de un **medidor de campaña**: un recurso con nombre, umbral
//! y un desenlace opcional al alcanzarlo.
//!
//! Es la contraparte de *definición* del primitivo de runtime [`crate::runtime::meter::Meter`]
//! (que solo acumula un valor). Aquí se declara, desde la campaña (RON), qué
//! medidores tiene un nivel, cómo arrancan y qué pasa cuando cruzan su umbral:
//! combustible u oxígeno que se agotan (`AtMost` + `Fail`), una barra de progreso
//! que al llenarse gana el nivel (`AtLeast` + `Win`), o un simple indicador
//! (`None`). La "traza"/detección del pentesting NO se declara aquí: es un
//! medidor privilegiado con su propia mecánica.

use serde::Deserialize;

/// Definición de un medidor declarado por un nivel.
#[derive(Debug, Clone, Deserialize)]
pub struct MeterDef {
    /// Identificador estable (lo referencian los efectos `AddMeter`).
    pub id: String,
    /// Etiqueta visible (si se omite, se usa el `id`).
    #[serde(default)]
    pub label: Option<String>,
    /// Valor inicial al arrancar el nivel.
    #[serde(default)]
    pub start: f32,
    /// Umbral que dispara el desenlace `on_limit`.
    pub limit: f32,
    /// Dirección del disparo respecto al umbral.
    #[serde(default)]
    pub trigger: MeterTrigger,
    /// Qué ocurre al cruzar el umbral.
    #[serde(default)]
    pub on_limit: OnLimit,
    /// Deriva automática por tick de reloj (p. ej. `-1.0` = se agota con el
    /// tiempo). `0.0` = solo cambia por efectos declarativos.
    #[serde(default)]
    pub per_tick: f32,
}

/// Dirección en la que el medidor cruza su umbral.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
pub enum MeterTrigger {
    /// Dispara cuando `valor >= limit` (traza, progreso que sube).
    #[default]
    AtLeast,
    /// Dispara cuando `valor <= limit` (combustible, oxígeno que baja).
    AtMost,
}

/// Desenlace al alcanzar el umbral de un medidor.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
pub enum OnLimit {
    /// Solo indicador: alcanzarlo no tiene consecuencia mecánica.
    #[default]
    None,
    /// Alcanzarlo pierde el nivel (derrota).
    Fail,
    /// Alcanzarlo supera el nivel (como lograr el objetivo).
    Win,
}

impl MeterDef {
    /// Etiqueta visible (cae al `id` si no se definió una).
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }

    /// ¿El valor dado cruza el umbral en la dirección configurada?
    pub fn triggered(&self, value: f32) -> bool {
        match self.trigger {
            MeterTrigger::AtLeast => value >= self.limit,
            MeterTrigger::AtMost => value <= self.limit,
        }
    }
}
