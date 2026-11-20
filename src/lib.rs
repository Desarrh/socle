//! # Socle
//!
//! Bibliothèque commune aux applis egui (`explorateur`, `lecteur-universel`, `vigie`, `tube`).
//! Elle est **compilée dans chaque exe** : aucune dépendance à l'exécution, chaque appli marche
//! seule. Chaque module est derrière une feature Cargo pour que chaque app n'embarque que ce
//! qu'elle utilise (voir `Cargo.toml` et `STACK.md`).
//!
//! | Module | Feature | Contenu |
//! |---|---|---|
//! | [`theme`] | `theme` | palettes crème / bleu nuit, 19 rôles, `apply`, polices |
//! | [`icons`] | `icons` | ~110 icônes vectorielles (grille 24, trait 1,8) |
//! | [`keys`] | `keys` | raccourcis compatibles AZERTY (Ctrl+chiffre par la touche physique) |
//! | [`ui`] | `ui` | toasts, texte tronqué, segmenté, modale, recherche, autocomplétion, palette Ctrl+K |
//! | [`fuzzy`], [`autocomplete`] | `fuzzy` | recherche floue < 5 ms / 100 000 entrées, historique, Tab |
//! | [`images`] | `images`, `img-*` | décodage étendu, miniature rapide, cache disque, pool annulable |
//! | [`settings`] | `settings` | `%APPDATA%\<App>\settings.json`, écriture atomique |
//! | [`crash`] | `crash` | `crash.log` + signalement au lancement suivant |
//! | [`instance`] | `instance` | instance unique (mutex + tube nommé) |
//! | [`update`] | `update` | mise à jour depuis GitHub Releases |

#[cfg(feature = "theme")]
pub mod theme;

#[cfg(feature = "icons")]
pub mod icons;

#[cfg(feature = "keys")]
pub mod keys;

#[cfg(feature = "fuzzy")]
pub mod fuzzy;

#[cfg(feature = "fuzzy")]
pub mod autocomplete;

#[cfg(feature = "ui")]
pub mod ui;

#[cfg(feature = "images")]
pub mod images;

pub mod paths;

#[cfg(feature = "settings")]
pub mod settings;

#[cfg(feature = "crash")]
pub mod crash;

#[cfg(all(feature = "instance", windows))]
pub mod instance;

#[cfg(feature = "update")]
pub mod update;

/// Jeton d'annulation partagé entre l'interface et un thread de travail.
/// Cloner = même jeton. `cancel()` est vu par tous les clones.
#[derive(Clone, Default, Debug)]
pub struct Cancel(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}
