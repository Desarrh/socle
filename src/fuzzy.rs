//! Recherche floue (QoL T4) : tolère les **accents**, la **casse**, les **initiales**
//! (`ec` → « Explorateur Cache », `ul` → `UltimateLauncher`), les **lettres sautées**
//! (`explr` → « explorateur ») et les **fautes de frappe** (`exploarteur` : 1 faute dès 5
//! lettres, 2 dès 9).
//!
//! Objectif : < 5 ms pour 100 000 entrées (voir `benches/perf.rs`). Comment :
//! - les entrées sont **repliées une seule fois** (minuscules, sans accents) dans un seul tampon
//!   contigu, avec leurs débuts de mots et un « sac de lettres » de 64 bits ;
//! - une requête teste d'abord la sous-chaîne (`memchr::memmem`, SIMD), puis la sous-séquence
//!   (initiales, lettres sautées), puis, seulement pour les entrées qui contiennent presque toutes
//!   les lettres de la requête (filtre par `popcount`), la distance d'édition par l'algorithme
//!   bit-parallèle de Myers (O(n) par entrée) ;
//! - au-delà de 40 000 entrées, le travail est réparti sur les cœurs (`std::thread::scope`).
//!
//! Aucune dépendance à egui : utilisable dans un thread, un outil en ligne de commande ou un test.

use memchr::memmem;
use std::cmp::Reverse;

// ---------------------------------------------------------------------------------------------
// Repliement : minuscules, sans accents.
// ---------------------------------------------------------------------------------------------

/// Replie un caractère (minuscule, sans accent) et l'ajoute à `out`.
#[inline]
fn fold_char(c: char, out: &mut Vec<u8>) {
    if c.is_ascii() {
        out.push((c as u8).to_ascii_lowercase());
        return;
    }
    let r: &[u8] = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => b"a",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' | 'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => b"c",
        'ď' | 'đ' | 'Ď' | 'Đ' => b"d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' | 'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => b"e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' | 'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => b"g",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' | 'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => b"i",
        'ñ' | 'ń' | 'ņ' | 'ň' | 'Ñ' | 'Ń' | 'Ņ' | 'Ň' => b"n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => b"o",
        'ŕ' | 'ř' | 'Ŕ' | 'Ř' => b"r",
        'ś' | 'ŝ' | 'ş' | 'š' | 'Ś' | 'Ŝ' | 'Ş' | 'Š' => b"s",
        'ţ' | 'ť' | 'Ţ' | 'Ť' => b"t",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => b"u",
        'ý' | 'ÿ' | 'Ý' | 'Ÿ' => b"y",
        'ź' | 'ż' | 'ž' | 'Ź' | 'Ż' | 'Ž' => b"z",
        'œ' | 'Œ' => b"oe",
        'æ' | 'Æ' => b"ae",
        'ß' => b"ss",
        '’' | '‘' => b"'",
        '«' | '»' | '“' | '”' => b"\"",
        '–' | '—' => b"-",
        '\u{a0}' | '\u{202f}' => b" ",
        _ => {
            let mut buf = [0u8; 4];
            for l in c.to_lowercase() {
                out.extend_from_slice(l.encode_utf8(&mut buf).as_bytes());
            }
            return;
        }
    };
    out.extend_from_slice(r);
}

/// Texte replié : minuscules, sans accents (`Élève Œuvre` → `eleve oeuvre`).
pub fn fold(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        fold_char(c, &mut out);
    }
    // fold_char n'écrit que de l'UTF-8 valide
    String::from_utf8(out).unwrap_or_default()
}

#[inline]
fn is_sep(c: char) -> bool {
    !c.is_alphanumeric()
}

/// Replie `s` dans `out` et renvoie le masque des débuts de mots (64 premiers octets repliés).
/// Début de mot : 1er caractère, après un séparateur, majuscule après minuscule (camelCase),
/// chiffre après lettre.
fn fold_with_words(s: &str, out: &mut Vec<u8>) -> u64 {
    let base = out.len();
    let mut words = 0u64;
    let mut prev: Option<char> = None;
    for c in s.chars() {
        let start = match prev {
            None => !is_sep(c),
            Some(p) => {
                !is_sep(c) && (is_sep(p) || (p.is_lowercase() && c.is_uppercase()) || (p.is_alphabetic() && c.is_numeric()))
            }
        };
        let pos = out.len() - base;
        if start && pos < 64 {
            words |= 1 << pos;
        }
        fold_char(c, out);
        prev = Some(c);
    }
    words
}

/// Sac de lettres sur 64 bits : a–z, 0–9, puis le reste haché.
#[inline]
fn bag(bytes: &[u8]) -> u64 {
    let mut m = 0u64;
    for &b in bytes {
        m |= 1 << bag_bit(b);
    }
    m
}

#[inline]
fn bag_bit(b: u8) -> u32 {
    match b {
        b'a'..=b'z' => (b - b'a') as u32,
        b'0'..=b'9' => 26 + (b - b'0') as u32,
        b' ' | b'_' | b'-' | b'.' | b'/' | b'\\' => 63, // séparateurs : ignorés par le filtre
        _ => 36 + (b as u32 * 7) % 27,
    }
}

// ---------------------------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------------------------

/// Entrées repliées une fois, prêtes à être cherchées. ~ 24 octets + longueur du texte par entrée.
#[derive(Clone, Default)]
pub struct Index {
    text: Vec<u8>,
    /// Début de chaque entrée dans `text` (n + 1 valeurs).
    starts: Vec<u32>,
    words: Vec<u64>,
    bags: Vec<u64>,
    boost: Vec<i32>,
}

/// Un résultat : position de l'entrée dans l'index (ordre d'ajout) et score (plus grand = mieux).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub index: usize,
    pub score: i32,
}

/// Niveau de correspondance (dans l'ordre de préférence).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Typo,
    Subsequence,
    Initials,
    Substring,
    Prefix,
    Exact,
}

impl Index {
    pub fn new() -> Self {
        Index { starts: vec![0], ..Default::default() }
    }

    /// Construit l'index à partir d'une liste de textes.
    pub fn from_iter<I, S>(items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut ix = Index::new();
        for s in items {
            ix.push(s.as_ref(), 0);
        }
        ix
    }

    /// Ajoute une entrée. `boost` s'ajoute au score (fréquence d'usage, épinglé…), ~0–300.
    pub fn push(&mut self, s: &str, boost: i32) {
        if self.starts.is_empty() {
            self.starts.push(0);
        }
        let b = self.text.len();
        let w = fold_with_words(s, &mut self.text);
        self.words.push(w);
        self.bags.push(bag(&self.text[b..]));
        self.boost.push(boost);
        self.starts.push(self.text.len() as u32);
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Index::new();
    }

    /// Change le bonus d'une entrée (ex. : après un usage).
    pub fn set_boost(&mut self, i: usize, boost: i32) {
        if let Some(b) = self.boost.get_mut(i) {
            *b = boost;
        }
    }

    /// Texte replié d'une entrée.
    pub fn folded(&self, i: usize) -> &str {
        let (a, b) = (self.starts[i] as usize, self.starts[i + 1] as usize);
        std::str::from_utf8(&self.text[a..b]).unwrap_or("")
    }

    /// Les `limit` meilleures entrées pour `query`, triées du meilleur au moins bon.
    /// Requête vide : les entrées avec le plus gros bonus, puis dans l'ordre d'ajout.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let n = self.len();
        if limit == 0 || n == 0 {
            return vec![];
        }
        let q = Query::new(query);
        if q.needle.is_empty() {
            let mut all: Vec<Hit> = (0..n).map(|i| Hit { index: i, score: self.boost[i] }).collect();
            top(&mut all, limit);
            return all;
        }
        let threads = std::thread::available_parallelism().map(|t| t.get()).unwrap_or(1).min(8);
        let mut hits = if n >= 40_000 && threads > 1 {
            let chunk = n.div_ceil(threads);
            std::thread::scope(|sc| {
                let q = &q;
                let handles: Vec<_> = (0..threads)
                    .map(|t| sc.spawn(move || self.scan(q, t * chunk, ((t + 1) * chunk).min(n), limit)))
                    .collect();
                handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect::<Vec<_>>()
            })
        } else {
            self.scan(&q, 0, n, limit)
        };
        top(&mut hits, limit);
        hits
    }

    fn scan(&self, q: &Query, from: usize, to: usize, limit: usize) -> Vec<Hit> {
        let mut out = Vec::new();
        for i in from..to {
            let (a, b) = (self.starts[i] as usize, self.starts[i + 1] as usize);
            if let Some((_, s)) = q.score(&self.text[a..b], self.words[i], self.bags[i]) {
                out.push(Hit { index: i, score: s + self.boost[i] });
            }
            // Garde la mémoire bornée pour les requêtes très larges (1 lettre).
            if out.len() >= limit.max(64) * 64 {
                top(&mut out, limit);
            }
        }
        out
    }
}

/// Garde les `limit` meilleurs résultats, triés (score décroissant, puis ordre d'ajout).
fn top(v: &mut Vec<Hit>, limit: usize) {
    let key = |h: &Hit| (Reverse(h.score), h.index);
    if v.len() > limit {
        v.select_nth_unstable_by_key(limit - 1, key);
        v.truncate(limit);
    }
    v.sort_unstable_by_key(key);
}

/// Score d'un texte isolé (sans index). `None` = ne correspond pas.
pub fn score(text: &str, query: &str) -> Option<i32> {
    let q = Query::new(query);
    let mut f = Vec::with_capacity(text.len());
    let w = fold_with_words(text, &mut f);
    if q.needle.is_empty() {
        return Some(0);
    }
    q.score(&f, w, bag(&f)).map(|x| x.1)
}

/// Type de correspondance d'un texte isolé (pour les tests et l'affichage).
pub fn kind(text: &str, query: &str) -> Option<Kind> {
    let q = Query::new(query);
    let mut f = Vec::with_capacity(text.len());
    let w = fold_with_words(text, &mut f);
    q.score(&f, w, bag(&f)).map(|x| x.0)
}

// ---------------------------------------------------------------------------------------------
// Requête
// ---------------------------------------------------------------------------------------------

/// Requête : un ou plusieurs mots séparés par des espaces, tous requis (« nouv dos »).
struct Query {
    needle: Vec<u8>,
    parts: Vec<Part>,
}

impl Query {
    fn new(q: &str) -> Query {
        let needle = fold(q.trim()).into_bytes();
        let words: Vec<&str> = q.split_whitespace().collect();
        let parts = if words.len() <= 1 { vec![Part::new(q)] } else { words.iter().map(|w| Part::new(w)).collect() };
        Query { needle, parts }
    }

    #[inline]
    fn score(&self, hay: &[u8], words: u64, hbag: u64) -> Option<(Kind, i32)> {
        if self.parts.len() == 1 {
            return self.parts[0].score(hay, words, hbag);
        }
        let mut kind = Kind::Exact;
        let mut total = 0;
        for p in &self.parts {
            let (k, s) = p.score(hay, words, hbag)?;
            kind = kind.min(k);
            total += s;
        }
        Some((kind, total / self.parts.len() as i32))
    }
}

struct Part {
    needle: Vec<u8>,
    finder: memmem::Finder<'static>,
    bag: u64,
    bag_n: u32,
    /// Fautes tolérées (0 si la requête est trop courte).
    k: u32,
    /// Table de Myers : bits des positions de chaque octet dans la requête.
    peq: Box<[u64; 256]>,
}

impl Part {
    fn new(q: &str) -> Part {
        let needle = fold(q.trim()).into_bytes();
        let finder = memmem::Finder::new(&needle).into_owned();
        let bag = bag(&needle) & !(1 << 63);
        let m = needle.len();
        // 1 faute dès 5 lettres, 2 dès 9 (en dessous, trop de faux positifs)
        let k = if m < 5 { 0 } else if m < 9 { 1 } else { 2 };
        let mut peq = Box::new([0u64; 256]);
        for (i, &c) in needle.iter().take(64).enumerate() {
            peq[c as usize] |= 1 << i;
        }
        Part { finder, bag, bag_n: bag.count_ones(), k, peq, needle }
    }

    /// (type, score) ou `None`.
    #[inline]
    fn score(&self, hay: &[u8], words: u64, hbag: u64) -> Option<(Kind, i32)> {
        let n = self.needle.len();
        let len = hay.len() as i32;
        // Filtre rapide : trop de lettres de la requête absentes de l'entrée.
        let missing = (self.bag & !hbag).count_ones();
        if missing > self.k {
            return None;
        }
        // 1. Sous-chaîne (le cas le plus fréquent et le plus fiable).
        if missing == 0 && hay.len() >= n {
            if let Some(pos) = self.finder.find(hay) {
                let at_word = pos < 64 && words & (1 << pos) != 0;
                let (kind, base) = if hay.len() == n {
                    (Kind::Exact, 3000)
                } else if pos == 0 {
                    (Kind::Prefix, 2400)
                } else {
                    (Kind::Substring, if at_word { 2000 } else { 1700 })
                };
                return Some((kind, base - (pos as i32).min(200) - len.min(300) / 3));
            }
            // 2. Sous-séquence : initiales et lettres sautées.
            if let Some((initials, s)) = subsequence(&self.needle, hay, words) {
                let kind = if initials { Kind::Initials } else { Kind::Subsequence };
                let base = if initials { 1500 } else { 900 };
                return Some((kind, base + s - len.min(300) / 4));
            }
        }
        // 3. Fautes de frappe (distance d'édition sur la meilleure sous-chaîne).
        if self.k > 0 && n <= 64 && self.bag_n >= 3 {
            let d = myers(&self.peq, n, hay, self.k);
            if d <= self.k {
                return Some((Kind::Typo, 500 - d as i32 * 150 - len.min(300) / 4));
            }
        }
        None
    }
}

/// Sous-séquence : chaque lettre de la requête trouvée dans l'ordre. Préfère les débuts de mots.
/// Renvoie (toutes aux débuts de mots ?, bonus).
#[inline]
fn subsequence(needle: &[u8], hay: &[u8], words: u64) -> Option<(bool, i32)> {
    // Initiales d'abord : chaque lettre sur un début de mot, ou juste après une lettre déjà
    // prise dans le même mot (« exca » → EXplorateur CAche, « ula » → Ultimate LAuncher).
    if words != 0 {
        let mut budget = 256u32;
        if initials(needle, hay, words, 0, 0, false, &mut budget) {
            return Some((true, 40 * needle.len() as i32));
        }
    }
    // Sinon, glouton de gauche à droite avec bonus.
    let mut i = 0;
    let mut bonus = 0i32;
    let mut last: Option<usize> = None;
    for &c in needle {
        let off = memchr::memchr(c, &hay[i..])?;
        let pos = i + off;
        if pos < 64 && words & (1 << pos) != 0 {
            bonus += 12;
        }
        match last {
            Some(l) if l + 1 == pos => bonus += 10,
            Some(l) => bonus -= ((pos - l - 1) as i32).min(8),
            None => bonus -= (pos as i32).min(10),
        }
        last = Some(pos);
        i = pos + 1;
    }
    Some((false, bonus))
}

/// Correspondance par initiales avec retour arrière borné (`budget` essais) : à chaque lettre,
/// on essaie de prolonger le mot en cours, sinon le prochain début de mot qui convient.
fn initials(needle: &[u8], hay: &[u8], words: u64, j: usize, pos: usize, cont: bool, budget: &mut u32) -> bool {
    if j == needle.len() {
        return true;
    }
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    let c = needle[j];
    // prolonger le mot en cours
    if cont && pos < hay.len() && hay[pos] == c && initials(needle, hay, words, j + 1, pos + 1, true, budget) {
        return true;
    }
    // sauter au prochain début de mot portant la lettre
    let mut w = if pos >= 64 { 0 } else { words & (!0u64 << pos) };
    while w != 0 {
        let p = w.trailing_zeros() as usize;
        w &= w - 1;
        if p < hay.len() && hay[p] == c && initials(needle, hay, words, j + 1, p + 1, true, budget) {
            return true;
        }
    }
    false
}

/// Distance d'édition minimale entre `needle` (≤ 64 octets) et une sous-chaîne quelconque de
/// `hay` : algorithme bit-parallèle de Myers (1999), une passe, quelques opérations par octet.
/// S'arrête dès que la distance tombe à 0.
#[inline]
fn myers(peq: &[u64; 256], m: usize, hay: &[u8], _k: u32) -> u32 {
    let mask_last = 1u64 << (m - 1);
    let mut pv: u64 = if m == 64 { !0 } else { (1u64 << m) - 1 };
    let mut mv: u64 = 0;
    let mut score = m as u32;
    let mut best = score;
    for &c in hay {
        let eq = peq[c as usize];
        let xv = eq | mv;
        let xh = ((eq & pv).wrapping_add(pv) ^ pv) | eq;
        let mut ph = mv | !(xh | pv);
        let mut mh = pv & xh;
        if ph & mask_last != 0 {
            score += 1;
        } else if mh & mask_last != 0 {
            score -= 1;
        }
        ph <<= 1;
        mh <<= 1;
        pv = mh | !(xv | ph);
        mv = ph & xv;
        if score < best {
            best = score;
            if best == 0 {
                break;
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------------------------
// Surlignage
// ---------------------------------------------------------------------------------------------

/// Indices (en caractères de `text`) à surligner pour `query`. À n'appeler que sur les quelques
/// résultats affichés : plus lent que `search` (pas de pré-calcul).
pub fn highlight(text: &str, query: &str) -> Vec<usize> {
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.len() <= 1 {
        return highlight_one(text, query);
    }
    let mut v: Vec<usize> = words.iter().flat_map(|w| highlight_one(text, w)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn highlight_one(text: &str, query: &str) -> Vec<usize> {
    let needle = fold(query.trim());
    if needle.is_empty() {
        return vec![];
    }
    // Repli caractère par caractère, en gardant la correspondance octet replié → caractère.
    let mut folded = Vec::with_capacity(text.len());
    let mut owner = Vec::with_capacity(text.len());
    let mut words = Vec::new();
    let mut prev: Option<char> = None;
    for (ci, c) in text.chars().enumerate() {
        let start = match prev {
            None => !is_sep(c),
            Some(p) => !is_sep(c) && (is_sep(p) || (p.is_lowercase() && c.is_uppercase()) || (p.is_alphabetic() && c.is_numeric())),
        };
        let b = folded.len();
        fold_char(c, &mut folded);
        for k in b..folded.len() {
            owner.push(ci);
            words.push(start && k == b);
        }
        prev = Some(c);
    }
    let nb = needle.as_bytes();
    let mut out: Vec<usize> = Vec::new();
    if let Some(pos) = memmem::find(&folded, nb) {
        out.extend(owner[pos..pos + nb.len()].iter().copied());
    } else {
        // initiales, puis glouton
        let mut j = 0;
        let mut picked = Vec::new();
        for (i, &b) in folded.iter().enumerate() {
            if j < nb.len() && words[i] && b == nb[j] {
                picked.push(i);
                j += 1;
            }
        }
        if j < nb.len() {
            picked.clear();
            let mut i = 0;
            for &c in nb {
                match folded[i..].iter().position(|&x| x == c) {
                    Some(o) => {
                        picked.push(i + o);
                        i += o + 1;
                    }
                    None => return vec![],
                }
            }
        }
        out.extend(picked.into_iter().map(|i| owner[i]));
    }
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repli_accents_casse() {
        assert_eq!(fold("Élève Œuvre À l'ÉTÉ"), "eleve oeuvre a l'ete");
        assert_eq!(fold("Straße"), "strasse");
        assert_eq!(fold("Ünïcödé"), "unicode");
    }

    #[test]
    fn types_de_correspondance() {
        assert_eq!(kind("explorateur", "explorateur"), Some(Kind::Exact));
        assert_eq!(kind("Explorateur.exe", "explo"), Some(Kind::Prefix));
        assert_eq!(kind("mon explorateur", "explo"), Some(Kind::Substring));
        assert_eq!(kind("Explorateur Cache", "ec"), Some(Kind::Initials));
        assert_eq!(kind("UltimateLauncher", "ula"), Some(Kind::Initials));
        assert_eq!(kind("Explorateur Cache", "exca"), Some(Kind::Initials));
        assert_eq!(kind("explorateur", "explr"), Some(Kind::Subsequence));
        assert_eq!(kind("explorateur", "exploarteur"), Some(Kind::Typo));
        assert_eq!(kind("téléchargements", "telechargemnts"), Some(Kind::Subsequence));
        assert_eq!(kind("téléchargements", "telecharjements"), Some(Kind::Typo));
        assert_eq!(kind("Vidéos", "videos"), Some(Kind::Exact));
        assert_eq!(kind("explorateur", "zzz"), None);
        assert_eq!(kind("Nouveau dossier", "nouv dos"), Some(Kind::Substring));
        assert_eq!(kind("Nouveau dossier", "dos nouv"), Some(Kind::Substring));
        assert_eq!(kind("abc", "abd"), None, "requête trop courte pour tolérer une faute");
    }

    #[test]
    fn ordre_des_resultats() {
        let ix = Index::from_iter(["Le grand explorateur", "explorateur", "Explorateur.exe", "exemple lourd", "xplorateur"]);
        let r = ix.search("explorateur", 10);
        let names: Vec<&str> = r.iter().map(|h| ix.folded(h.index)).collect();
        assert_eq!(names[0], "explorateur");
        assert_eq!(names[1], "explorateur.exe");
        assert_eq!(names[2], "le grand explorateur");
        assert!(names.contains(&"xplorateur"), "1 faute tolérée : {names:?}");
    }

    #[test]
    fn bonus_et_requete_vide() {
        let mut ix = Index::from_iter(["alpha", "beta", "gamma"]);
        ix.set_boost(2, 50);
        assert_eq!(ix.search("", 2)[0].index, 2);
        assert_eq!(ix.search("a", 3).len(), 3);
    }

    #[test]
    fn myers_distance() {
        let q = Part::new("bonjour");
        assert_eq!(myers(&q.peq, 7, b"xx bonjour xx", 1), 0);
        assert_eq!(myers(&q.peq, 7, b"bonjuor", 2), 2);
        assert_eq!(myers(&q.peq, 7, b"bonour", 1), 1);
    }

    #[test]
    fn surlignage() {
        assert_eq!(highlight("Mon Élève", "eleve"), vec![4, 5, 6, 7, 8]);
        assert_eq!(highlight("Explorateur Cache", "ec"), vec![0, 12]);
        assert!(highlight("abc", "z").is_empty());
    }

    #[test]
    fn gros_index_parallele() {
        let ix = Index::from_iter((0..50_000).map(|i| format!("fichier_{i:05}.txt")));
        let r = ix.search("fichier_04242", 5);
        assert_eq!(ix.folded(r[0].index), "fichier_04242.txt");
    }
}
