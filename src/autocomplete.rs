//! Autocomplétion (QoL T4) : suggestions floues tirées de l'**historique** (fréquence + récence)
//! et d'une liste de **candidats** fournie par l'app (noms du dossier, sous-dossiers, titres…).
//! Tab complète. Logique pure, sans egui ; le champ visuel est dans `ui::autocomplete_field`.
//!
//! Avec la feature `settings`, `History` se sérialise (serde) : l'app la range dans ses réglages.

use crate::fuzzy::{self, Index};

/// Une saisie passée.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "settings", derive(serde::Serialize, serde::Deserialize))]
pub struct Past {
    pub text: String,
    pub count: u32,
    /// Secondes Unix du dernier usage.
    pub last: u64,
}

/// Historique des saisies, borné (les moins utiles sont oubliées).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "settings", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "settings", serde(default))]
pub struct History {
    pub items: Vec<Past>,
    pub max: usize,
}

impl Default for History {
    fn default() -> Self {
        History { items: Vec::new(), max: 200 }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl History {
    /// Enregistre une saisie validée (Entrée).
    pub fn record(&mut self, text: &str) {
        self.record_at(text, now());
    }

    pub fn record_at(&mut self, text: &str, t: u64) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let key = fuzzy::fold(text);
        if let Some(p) = self.items.iter_mut().find(|p| fuzzy::fold(&p.text) == key) {
            p.count += 1;
            p.last = t;
            p.text = text.to_string(); // garde la dernière graphie
        } else {
            self.items.push(Past { text: text.to_string(), count: 1, last: t });
        }
        if self.items.len() > self.max.max(1) {
            // oublie la saisie la moins « fréquente-récente »
            if let Some((i, _)) = self.items.iter().enumerate().min_by(|a, b| self.frecency(a.1, t).total_cmp(&self.frecency(b.1, t))) {
                self.items.remove(i);
            }
        }
    }

    pub fn remove(&mut self, text: &str) {
        let key = fuzzy::fold(text);
        self.items.retain(|p| fuzzy::fold(&p.text) != key);
    }

    /// Fréquence × récence : ln(1 + n) pondéré, + bonus décroissant sur ~1 semaine.
    pub fn frecency(&self, p: &Past, now: u64) -> f32 {
        let age_days = now.saturating_sub(p.last) as f32 / 86_400.0;
        (1.0 + p.count as f32).ln() * 60.0 + 150.0 / (1.0 + age_days)
    }
}

/// Une suggestion.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub text: String,
    /// Vient de l'historique (affichée avec l'icône horloge).
    pub past: bool,
    pub score: i32,
}

/// Moteur d'autocomplétion : historique + candidats.
#[derive(Clone, Default)]
pub struct Autocomplete {
    pub history: History,
    candidates: Vec<String>,
    index: Index,
}

impl Autocomplete {
    pub fn new(history: History) -> Self {
        Autocomplete { history, candidates: Vec::new(), index: Index::new() }
    }

    /// Remplace les candidats (ex. : noms du dossier courant). Coût : ~40 ns par entrée.
    pub fn set_candidates<I: IntoIterator<Item = String>>(&mut self, items: I) {
        self.candidates = items.into_iter().collect();
        self.index = Index::from_iter(self.candidates.iter());
    }

    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }

    /// Suggestions pour `input`, meilleures d'abord, sans doublons. L'historique passe devant à
    /// score égal. Une entrée identique à la saisie n'est pas proposée.
    pub fn suggest(&self, input: &str, limit: usize) -> Vec<Suggestion> {
        let t = now();
        let key = fuzzy::fold(input.trim());
        let mut out: Vec<Suggestion> = Vec::new();
        for p in &self.history.items {
            let s = if key.is_empty() { Some(0) } else { fuzzy::score(&p.text, input) };
            if let Some(s) = s {
                out.push(Suggestion { text: p.text.clone(), past: true, score: s + self.history.frecency(p, t) as i32 });
            }
        }
        if !key.is_empty() {
            for h in self.index.search(input, limit * 2) {
                out.push(Suggestion { text: self.candidates[h.index].clone(), past: false, score: h.score });
            }
        }
        out.sort_by(|a, b| b.score.cmp(&a.score).then(b.past.cmp(&a.past)));
        let mut seen = std::collections::HashSet::new();
        out.retain(|s| {
            let k = fuzzy::fold(&s.text);
            k != key && seen.insert(k)
        });
        out.truncate(limit);
        out
    }

    /// Ce que Tab doit écrire dans le champ :
    /// - la suggestion choisie si l'utilisateur en a sélectionné une (`selected`) ;
    /// - sinon, le plus long préfixe commun aux suggestions qui commencent par la saisie, s'il
    ///   allonge la saisie (comme un terminal) ;
    /// - sinon, la meilleure suggestion.
    pub fn complete(&self, input: &str, suggestions: &[Suggestion], selected: Option<usize>) -> Option<String> {
        if let Some(s) = selected.and_then(|i| suggestions.get(i)) {
            return Some(s.text.clone());
        }
        let key = fuzzy::fold(input);
        let starting: Vec<&str> = suggestions.iter().map(|s| s.text.as_str()).filter(|t| fuzzy::fold(t).starts_with(&key)).collect();
        if let Some(lcp) = common_prefix(&starting) {
            if lcp.chars().count() > input.chars().count() {
                // complétion entière : la graphie de la suggestion
                if starting.iter().any(|t| t.chars().count() == lcp.chars().count()) {
                    return Some(lcp);
                }
                // garde la casse tapée pour la partie déjà saisie
                let tail: String = lcp.chars().skip(input.chars().count()).collect();
                return Some(format!("{input}{tail}"));
            }
        }
        suggestions.first().map(|s| s.text.clone())
    }
}

/// Plus long préfixe commun (insensible à la casse et aux accents), dans la graphie du 1er texte.
fn common_prefix(items: &[&str]) -> Option<String> {
    let first = items.first()?;
    let mut n = first.chars().count();
    for t in &items[1..] {
        let k = first.chars().zip(t.chars()).take_while(|(a, b)| fuzzy::fold(&a.to_string()) == fuzzy::fold(&b.to_string())).count();
        n = n.min(k);
    }
    Some(first.chars().take(n).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historique_frequence_recence() {
        let mut h = History::default();
        h.record_at("cargo build", 1_000);
        h.record_at("cargo test", 1_000);
        h.record_at("Cargo Test", 2_000);
        assert_eq!(h.items.len(), 2, "même saisie à la casse près");
        assert_eq!(h.items[1].count, 2);
        assert_eq!(h.items[1].text, "Cargo Test");
        h.max = 2;
        h.record_at("git status", 3_000);
        assert_eq!(h.items.len(), 2);
        assert!(h.items.iter().all(|p| p.text != "cargo build"), "la moins utile est oubliée");
    }

    #[test]
    fn suggestions_et_tab() {
        let mut ac = Autocomplete::default();
        ac.set_candidates(["Documents", "Dev", "Développement", "Téléchargements", "Desktop"].map(String::from));
        let s = ac.suggest("de", 10);
        assert!(s.iter().any(|x| x.text == "Développement"), "accents ignorés");
        assert_eq!(ac.suggest("tele", 1)[0].text, "Téléchargements");
        assert!(ac.suggest("dev", 5).iter().all(|x| x.text != "Dev"), "la saisie elle-même n'est pas proposée");
        // Tab : préfixe commun à « Développement » et « Dev » = « Dev »… déjà tapé → meilleure suggestion
        let s = ac.suggest("dév", 5);
        assert_eq!(ac.complete("dév", &s, None).as_deref(), Some("Développement"));
        // préfixe commun qui allonge la saisie
        ac.set_candidates(["projet-alpha", "projet-beta"].map(String::from));
        let s = ac.suggest("pro", 5);
        assert_eq!(ac.complete("pro", &s, None).as_deref(), Some("projet-"));
        assert_eq!(ac.complete("pro", &s, Some(1)).as_deref(), Some(s[1].text.as_str()));
    }

    #[test]
    fn historique_devant() {
        let mut ac = Autocomplete::default();
        ac.set_candidates(["minecraft", "mine"].map(String::from));
        ac.history.record("minecraft mods");
        let s = ac.suggest("mine", 5);
        assert!(s[0].past, "{s:?}");
        assert!(s.iter().all(|x| x.text != "mine"), "la saisie elle-même n'est pas proposée");
    }
}
