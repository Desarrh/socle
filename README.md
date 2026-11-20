# Socle

Bibliothèque Rust commune aux applis egui de Desarrh ([explorateur](https://github.com/Desarrh/explorateur), [lecteur universel](https://github.com/Desarrh/lecteur-universel), Vigie, Tube) : thème, icônes vectorielles, composants, recherche floue et autocomplétion, palette Ctrl+K, décodage d'images étendu, réglages, journal de plantage, instance unique, mise à jour.

Compilée dans chaque exe : aucune dépendance à l'exécution. Chaque partie est une feature Cargo.

```toml
socle = { git = "https://github.com/Desarrh/socle", tag = "v0.1.1", default-features = false, features = ["ui", "settings"] }
```

- [STACK.md](STACK.md) : architecture, features, tailles, mesures
- [MIGRATION.md](MIGRATION.md) : guide de migration par app

```sh
cargo test --release --features full
cargo bench --features full
cargo run --release --example demo --features full
```

Licence MIT.
