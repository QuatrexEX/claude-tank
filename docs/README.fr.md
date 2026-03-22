# Claude Tank

<img src="img/icon.png" alt="Claude Tank Icon" width="64">

> Moniteur d'utilisation de votre plan Claude pour Windows

**🌐 Langue :** [English](../README.md) | [日本語](README.ja.md) | [Deutsch](README.de.md) | [한국어](README.ko.md) | Français

---

**Claude Tank** est une application légère pour la barre d'état Windows qui surveille en temps réel les limites d'utilisation de votre plan Claude Pro/Max/Team.

Plus de limites surprise — visualisez votre capacité restante à 5 heures et 7 jours en un coup d'œil.

## Fonctionnalités

- **Double jauge dans l'icône** — 5h et 7d restants en barres colorées
- **Tableau de bord cyber** — cliquez sur l'icône pour un popup avec jauges en forme de réservoir
- **Connexion automatique** — connectez-vous à claude.ai dans le navigateur intégré, cookies détectés automatiquement
- **Mise à jour en arrière-plan** — toutes les 1/3/5 minutes (configurable)
- **Alertes de seuil** — alertes indépendantes 5h et 7d
- **Identifiants chiffrés** — clé de session stockée avec Windows DPAPI
- **Démarrage automatique** — lancement optionnel au démarrage de Windows
- **Multilingue** — anglais, japonais, allemand, coréen, français (détection automatique)
- **Léger** — exe ~2 Mo, RAM ~20 Mo, pas d'installateur

## Démarrage rapide

1. Téléchargez `claude-tank.exe` depuis [Releases](https://github.com/QuatrexEX/claude-tank/releases)
2. Exécutez — une fenêtre s'ouvre pour la connexion à claude.ai
3. Connectez-vous normalement (Google, email, SSO)
4. C'est fait ! L'icône affiche vos jauges d'utilisation

## Compiler depuis les sources

Prérequis : [Rust](https://rustup.rs/) (stable)

```bash
cargo build --release
```

## Licence

[MIT](../LICENSE)

## Auteur

**Quatrex** — [@QuatrexEX](https://github.com/QuatrexEX)
