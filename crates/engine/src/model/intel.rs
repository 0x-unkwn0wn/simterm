//! Hallazgos de inteligencia (IntelFinding) generados por el reconocimiento.

use serde::{Deserialize, Serialize};

use crate::model::language::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingStatus {
    Unverified,
    VerifiedTrue,
    VerifiedFalse,
    Exploited,
    Failed,
}

impl FindingStatus {
    pub fn label_in(&self, language: Language) -> &'static str {
        match (language, self) {
            (Language::Es, FindingStatus::Unverified) => "SIN VERIFICAR",
            (Language::Es, FindingStatus::VerifiedTrue) => "VERIFICADO+",
            (Language::Es, FindingStatus::VerifiedFalse) => "VERIFICADO-",
            (Language::Es, FindingStatus::Exploited) => "EXPLOTADO",
            (Language::Es, FindingStatus::Failed) => "FALLIDO",
            (Language::En, FindingStatus::Unverified) => "UNVERIFIED",
            (Language::En, FindingStatus::VerifiedTrue) => "VERIFIED+",
            (Language::En, FindingStatus::VerifiedFalse) => "VERIFIED-",
            (Language::En, FindingStatus::Exploited) => "EXPLOITED",
            (Language::En, FindingStatus::Failed) => "FAILED",
        }
    }

    pub fn label(&self) -> &'static str {
        self.label_in(Language::Es)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingSource {
    PortScan,
    DeepScan,
    Guess,
}

impl FindingSource {
    pub fn label_in(&self, language: Language) -> &'static str {
        match (language, self) {
            (Language::Es, FindingSource::PortScan) => "scan",
            (Language::Es, FindingSource::DeepScan) => "deep-scan",
            (Language::Es, FindingSource::Guess) => "intuición",
            (Language::En, FindingSource::PortScan) => "scan",
            (Language::En, FindingSource::DeepScan) => "deep-scan",
            (Language::En, FindingSource::Guess) => "hunch",
        }
    }

    pub fn label(&self) -> &'static str {
        self.label_in(Language::Es)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelFinding {
    /// Id numérico visible para el jugador.
    pub public_id: usize,
    pub title: String,
    pub target_node: String,
    /// Confianza estimada entre 0.0 y 1.0.
    pub confidence: f32,
    pub status: FindingStatus,
    pub source: FindingSource,
    /// Si está presente, el hallazgo corresponde a una vulnerabilidad real.
    /// Campo interno: nunca se muestra al jugador.
    pub real_vuln_id: Option<String>,
    /// Lecturas de `searchsploit` a favor de que es real (consenso).
    #[serde(default)]
    pub verify_pos: u8,
    /// Lecturas de `searchsploit` a favor de que es falso positivo.
    #[serde(default)]
    pub verify_neg: u8,
}

impl IntelFinding {
    /// Indica si el hallazgo apunta a una vulnerabilidad real (uso interno).
    pub fn is_real(&self) -> bool {
        self.real_vuln_id.is_some()
    }

    pub fn confidence_pct(&self) -> u32 {
        (self.confidence.clamp(0.0, 1.0) * 100.0).round() as u32
    }
}
