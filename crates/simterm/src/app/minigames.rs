//! Easter eggs y minijuegos ocultos. No afectan a la partida (ni traza, ni
//! reloj, ni fases): son un "juego dentro del juego".
//!
//! La MECÁNICA (cifrado César, mastermind, teclado) vive en el motor/frontend;
//! el CONTENIDO (respuestas temáticas, palabras de señal, aforismos) lo define
//! la campaña en su `theme`/`easter_eggs`/`fortunes`/`signals`.

use rand::Rng;

use super::App;

impl App {
    /// Easter egg de campaña para `verb`: si existe, imprime sus líneas
    /// (sustituyendo `{clock}`) y devuelve `true`; si no, devuelve `false` para
    /// que el dispatcher siga probando (o responda `command not found`).
    pub(super) fn try_easter(&mut self, verb: &str) -> bool {
        let clock = self.game.core.clock;
        let lines: Option<Vec<String>> = self.game.campaign.easter_egg(verb).map(|egg| {
            egg.lines
                .iter()
                .map(|l| l.replace("{clock}", &clock.to_string()))
                .collect()
        });
        match lines {
            Some(ls) => {
                for l in ls {
                    self.game.log(l);
                }
                true
            }
            None => false,
        }
    }

    /// `fortune`: aforismo aleatorio del catálogo de la campaña.
    pub(super) fn cmd_fortune(&mut self) {
        if self.game.campaign.fortunes.is_empty() {
            self.game
                .log(String::from("(sin fortunas en esta campaña)"));
            return;
        }
        let i = rand::thread_rng().gen_range(0..self.game.campaign.fortunes.len());
        let line = self.game.campaign.fortunes[i].clone();
        self.game.log(line);
    }

    /// Minijuego: intercepta una señal y la muestra cifrada con un César de
    /// desplazamiento variable (más reto que un ROT13 fijo). Las palabras en
    /// claro las define la campaña (`signals`).
    pub(super) fn cmd_signal(&mut self) {
        if self.game.campaign.signals.is_empty() {
            self.game.log(String::from(
                "[signal] Sin señales que interceptar en esta campaña.",
            ));
            return;
        }
        let mut rng = rand::thread_rng();
        let pick =
            self.game.campaign.signals[rng.gen_range(0..self.game.campaign.signals.len())].clone();
        let shift: u8 = rng.gen_range(1..=25);
        let enc = caesar(&pick, shift);
        self.signal_answer = Some(pick);
        self.game.log(String::from("--- SEÑAL INTERCEPTADA ---"));
        self.game
            .log(format!("cifrado (César, desplazamiento {shift}): {enc}"));
        self.game
            .log(String::from("Descífrala con 'decode <texto en claro>'."));
    }

    /// Minijuego: comprueba el texto descifrado de la señal interceptada.
    pub(super) fn cmd_decode(&mut self, text: String) {
        let guess = text.trim();
        match self.signal_answer.clone() {
            None => self.game.log(String::from(
                "No hay ninguna señal interceptada. Usa 'signal' primero.",
            )),
            Some(_) if guess.is_empty() => self.game.log(String::from("uso: decode <texto>")),
            Some(ans) if guess.eq_ignore_ascii_case(&ans) => {
                self.game
                    .log(format!("[decode] Correcto. Texto en claro: {ans}."));
                self.signal_answer = None;
            }
            Some(_) => self.game.log(String::from(
                "[decode] Texto en claro incorrecto. La señal sigue cifrada.",
            )),
        }
    }

    /// Minijuego: fuerza un teclado numérico de 4 dígitos (con pistas alto/bajo).
    pub(super) fn cmd_crack(&mut self, guess: Option<u16>) {
        match guess {
            None => {
                let secret = rand::thread_rng().gen_range(0..=9999);
                self.keypad = Some((secret, 0));
                self.game
                    .log(String::from("--- TECLADO NUMÉRICO (4 dígitos) ---"));
                self.game.log(String::from(
                    "Fuerza el PIN: 'crack <0-9999>'. Te diré si el real es más alto o más bajo.",
                ));
            }
            Some(g) => match self.keypad {
                None => self.game.log(String::from(
                    "No hay teclado activo. Inícialo con 'crack' (sin número).",
                )),
                Some((secret, tries)) => {
                    let tries = tries + 1;
                    if g == secret {
                        self.game.log(format!(
                            "[crack] PIN {secret:04} CORRECTO en {tries} intento(s). Cerradura abierta."
                        ));
                        self.keypad = None;
                    } else {
                        let hint = if g < secret { "más ALTO" } else { "más BAJO" };
                        self.game.log(format!(
                            "[crack] {g:04} incorrecto. El PIN real es {hint}. (intento {tries})"
                        ));
                        self.keypad = Some((secret, tries));
                    }
                }
            },
        }
    }

    /// `history`: vuelca el historial de comandos introducidos.
    pub(super) fn cmd_history(&mut self) {
        if self.history.is_empty() {
            self.game.log(String::from("(historial vacío)"));
            return;
        }
        let hist = self.history.clone();
        self.game.log(String::from("--- HISTORIAL ---"));
        for (i, h) in hist.iter().enumerate() {
            self.game.log(format!("  {:>3}  {h}", i + 1));
        }
    }

    /// Minijuego mastermind (picos y toques): código de 4 dígitos únicos.
    pub(super) fn cmd_mastermind(&mut self, arg: Option<String>) {
        match arg {
            None => {
                // Nueva partida: 4 dígitos distintos al azar.
                let mut rng = rand::thread_rng();
                let mut pool: Vec<u8> = (0..=9).collect();
                let mut secret = [0u8; 4];
                for slot in secret.iter_mut() {
                    let j = rng.gen_range(0..pool.len());
                    *slot = pool.remove(j);
                }
                self.mastermind = Some((secret, 0));
                self.game
                    .log(String::from("--- MASTERMIND (4 dígitos, sin repetir) ---"));
                self.game.log(String::from(
                    "Adivina con 'mastermind <NNNN>'. picos = dígito y posición; toques = solo dígito.",
                ));
            }
            Some(s) => match self.mastermind {
                None => self.game.log(String::from(
                    "No hay partida activa. Inícialo con 'mastermind' (sin argumento).",
                )),
                Some((secret, tries)) => match parse_code(&s) {
                    None => self.game.log(String::from(
                        "Código inválido: exactamente 4 dígitos (0-9).",
                    )),
                    Some(guess) => {
                        let tries = tries + 1;
                        let mut picos = 0;
                        for i in 0..4 {
                            if guess[i] == secret[i] {
                                picos += 1;
                            }
                        }
                        // Toques: coincidencias de dígito (en cualquier posición) menos picos.
                        let mut shared = 0;
                        for d in 0u8..=9 {
                            let cg = guess.iter().filter(|&&x| x == d).count();
                            let cs = secret.iter().filter(|&&x| x == d).count();
                            shared += cg.min(cs);
                        }
                        let toques = shared - picos;
                        if picos == 4 {
                            self.game.log(format!(
                                "[mastermind] CÓDIGO ROTO en {tries} intento(s). Acceso concedido."
                            ));
                            self.mastermind = None;
                        } else {
                            self.game.log(format!(
                                "[mastermind] picos: {picos}   toques: {toques}   (intento {tries})"
                            ));
                            self.mastermind = Some((secret, tries));
                        }
                    }
                },
            },
        }
    }
}

/// Cifrado César con desplazamiento `shift`. Solo afecta a las letras ASCII.
fn caesar(s: &str, shift: u8) -> String {
    let k = shift % 26;
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a' + k) % 26) + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A' + k) % 26) + b'A') as char,
            other => other,
        })
        .collect()
}

/// Parsea un código de exactamente 4 dígitos (0-9) para el minijuego mastermind.
fn parse_code(s: &str) -> Option<[u8; 4]> {
    let chars: Vec<char> = s.trim().chars().collect();
    if chars.len() != 4 {
        return None;
    }
    let mut out = [0u8; 4];
    for (i, c) in chars.iter().enumerate() {
        out[i] = c.to_digit(10)? as u8;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{caesar, parse_code};

    #[test]
    fn rot13_es_involutivo() {
        // ROT13 = César con desplazamiento 13 (involutivo).
        assert_eq!(caesar("ABCDEF", 13), "NOPQRS");
        assert_eq!(caesar(&caesar("ABCDEF", 13), 13), "ABCDEF");
        // No toca dígitos ni símbolos.
        assert_eq!(caesar("PIN-42", 13), "CVA-42");
    }

    #[test]
    fn caesar_descifra_con_complemento() {
        // Cifrar con k y descifrar con 26-k recupera el original.
        let enc = caesar("SIGNAL", 5);
        assert_eq!(caesar(&enc, 26 - 5), "SIGNAL");
    }

    #[test]
    fn parse_code_valida_cuatro_digitos() {
        assert_eq!(parse_code("0427"), Some([0, 4, 2, 7]));
        assert_eq!(parse_code("42"), None);
        assert_eq!(parse_code("12ab"), None);
    }
}
