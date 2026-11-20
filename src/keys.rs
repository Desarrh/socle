//! Lecture des raccourcis clavier, compatible **AZERTY**.
//!
//! egui-winit 0.37 transmet la touche **logique** (`key = logical_key.or(physical_key)`). En
//! AZERTY, la rangée des chiffres donne `&`, `é`, `"`, `'`… : Ctrl+3 arrive comme `Key::Quote`
//! et les raccourcis Ctrl+chiffre ne marchent pas. Règle (reprise de `tube/src/ui.rs`,
//! `handle_keys`) : pour la rangée des chiffres, on prend la touche **physique** quand Ctrl est
//! enfoncé, ou quand la touche logique n'est pas déjà un chiffre (sauf `,` et `.` du pavé
//! numérique). Avec ces helpers, les apps peuvent garder Ctrl+1…9.

use egui::{Event, Key, Modifiers};

fn is_digit(k: Key) -> bool {
    matches!(k, Key::Num0 | Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 | Key::Num5 | Key::Num6 | Key::Num7 | Key::Num8 | Key::Num9)
}

/// Touche à utiliser pour un raccourci, d'après la touche logique, la touche physique et les
/// modificateurs d'un `Event::Key`.
pub fn effective(key: Key, physical: Option<Key>, mods: Modifiers) -> Key {
    match physical {
        Some(pk) if is_digit(pk) && (mods.ctrl || mods.command || !is_digit(key)) && !matches!(key, Key::Comma | Key::Period) => pk,
        _ => key,
    }
}

/// Chiffre 0–9 d'une touche (`Key::Num3` → 3).
pub fn digit(k: Key) -> Option<u8> {
    const D: [Key; 10] = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
    D.iter().position(|d| *d == k).map(|n| n as u8)
}

/// Touches enfoncées pendant cette image, corrigées pour l'AZERTY, avec leurs modificateurs.
pub fn pressed(ctx: &egui::Context) -> Vec<(Key, Modifiers)> {
    ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                Event::Key { key, physical_key, pressed: true, modifiers, .. } => Some((effective(*key, *physical_key, *modifiers), *modifiers)),
                _ => None,
            })
            .collect()
    })
}

/// Comme `InputState::consume_key`, mais sur la touche corrigée : vrai (et l'événement est
/// retiré) si `key` a été enfoncée avec exactement `mods` (`Modifiers::matches_logically`).
pub fn consume(ctx: &egui::Context, mods: Modifiers, key: Key) -> bool {
    ctx.input_mut(|i| {
        let pos = i.events.iter().position(|e| match e {
            Event::Key { key: k, physical_key, pressed: true, modifiers, .. } => effective(*k, *physical_key, *modifiers) == key && modifiers.matches_logically(mods),
            _ => false,
        });
        match pos {
            Some(p) => {
                i.events.remove(p);
                true
            }
            None => false,
        }
    })
}

/// Ctrl+chiffre enfoncé pendant cette image (consommé), quelle que soit la disposition.
pub fn consume_ctrl_digit(ctx: &egui::Context) -> Option<u8> {
    for n in 0..10u8 {
        let k = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9][n as usize];
        if consume(ctx, Modifiers::COMMAND, k) {
            return Some(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azerty() {
        let ctrl = Modifiers::COMMAND;
        // Ctrl+3 en AZERTY : logique « " » (Quote), physique Num3
        assert_eq!(effective(Key::Quote, Some(Key::Num3), ctrl), Key::Num3);
        // sans Ctrl, chiffre physique mais logique non chiffre → chiffre (saut à 30 % dans Tube)
        assert_eq!(effective(Key::Quote, Some(Key::Num3), Modifiers::NONE), Key::Num3);
        // QWERTY : rien ne change
        assert_eq!(effective(Key::Num3, Some(Key::Num3), ctrl), Key::Num3);
        // lettres : touche logique (Ctrl+A en AZERTY reste A)
        assert_eq!(effective(Key::A, Some(Key::Q), ctrl), Key::A);
        // pavé numérique : virgule et point gardés
        assert_eq!(effective(Key::Comma, Some(Key::Num5), Modifiers::NONE), Key::Comma);
        assert_eq!(digit(Key::Num7), Some(7));
        assert_eq!(digit(Key::A), None);
    }

    #[test]
    fn consommation() {
        let ctx = egui::Context::default();
        let ev = Event::Key { key: Key::Quote, physical_key: Some(Key::Num4), pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
        let input = egui::RawInput { events: vec![ev], ..Default::default() };
        let mut got = None;
        let _ = ctx.run_ui(input, |ui| got = consume_ctrl_digit(ui.ctx()));
        assert_eq!(got, Some(4));
    }
}
