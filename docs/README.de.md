# Claude Tank

> Ihr Claude-Plan-Nutzungsmonitor für Windows

**🌐 Sprache:** [English](../README.md) | [日本語](README.ja.md) | Deutsch | [한국어](README.ko.md) | [Français](README.fr.md)

---

**Claude Tank** ist eine leichtgewichtige Windows-Systemtray-App, die Ihre Claude Pro/Max/Team-Nutzungslimits in Echtzeit überwacht.

Keine überraschenden Rate-Limits mehr — sehen Sie Ihre 5-Stunden- und 7-Tage-Restkapazität auf einen Blick.

## Funktionen

- **Doppel-Anzeige im Tray-Icon** — 5h- und 7d-Rest als farbkodierte Balken
- **Cyber-Theme-Dashboard** — Klick auf das Tray-Icon für Tank-Anzeigen
- **Auto-Login** — Melden Sie sich bei claude.ai im integrierten Browser an, Cookies werden automatisch erkannt
- **Hintergrund-Polling** — Nutzungsaktualisierung alle 1/3/5 Minuten (konfigurierbar)
- **Schwellenwert-Alarme** — Unabhängige 5h- und 7d-Alarme
- **Verschlüsselte Anmeldedaten** — Session-Key mit Windows DPAPI gespeichert
- **Autostart** — Optionaler Start beim Windows-Login
- **Mehrsprachig** — Englisch, Japanisch, Deutsch, Koreanisch, Französisch (automatische OS-Erkennung)
- **Klein** — ~2MB exe, ~20MB RAM, kein Installer nötig

## Schnellstart

1. `claude-tank.exe` von [Releases](https://github.com/QuatrexEX/claude-tank/releases) herunterladen
2. Ausführen — ein Browserfenster öffnet sich für die claude.ai-Anmeldung
3. Normal anmelden (Google, E-Mail, SSO)
4. Fertig! Das Tray-Icon zeigt Ihre Nutzungsanzeigen

## Aus Quellcode bauen

Voraussetzung: [Rust](https://rustup.rs/) (stable)

```bash
cargo build --release
```

## Lizenz

[MIT](../LICENSE)

## Autor

**Quatrex** — [@QuatrexEX](https://github.com/QuatrexEX)
