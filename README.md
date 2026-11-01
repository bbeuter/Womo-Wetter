# Womo-Wetter – Sonnen-Radar ☀︎

Zeigt GAFOR-ähnlich, in welchen Regionen (DE, NL, BE, LU, grenznahes FR) jetzt, in +2 h, +4 h und +6 h die Sonne scheint – für spontane Wohnmobil-Wochenendtouren. Läuft als Web-App auf iPhone/iPad („Zum Home-Bildschirm“).

## Aufbau
| Teil | Ordner | Aufgabe |
|---|---|---|
| Regionen | `tools/regions/`, `data/regions.geojson` | 118 Regionen (81 DE GAFOR-ähnlich, NL/BE/LU/FR) |
| Rust-Kern | `womo-core/` | holt Open-Meteo (DWD ICON), bewertet Regionen, schreibt JSON; Stellplätze aus OpenStreetMap |
| Web-App | `web/` | PWA: Karte, Zeit-Slots, „Beste Ziele“, Detail-Zoom, Wochenende, Stellplätze, offline |
| Automatik | `.github/workflows/` | alle 3 h Vorhersage + Deploy auf GitHub Pages, wöchentlich Stellplätze, CI |

Details: [docs/ARCHITEKTUR.md](docs/ARCHITEKTUR.md), [docs/DATENFORMAT.md](docs/DATENFORMAT.md).

## Einrichtung (einmalig)
1. Repository auf **öffentlich** stellen (Settings → General → Danger Zone).
2. **Settings → Pages → Source: „GitHub Actions“**.
3. Branch nach `main` mergen (geplante Läufe laufen nur auf `main`).
4. Unter **Actions** einmal „Stellplätze aktualisieren“ und danach „Vorhersage & Deploy“ manuell starten.
5. Auf dem iPhone/iPad `https://<benutzer>.github.io/Womo-Wetter/` in Safari öffnen → Teilen → **Zum Home-Bildschirm**.

## Lokal ausprobieren
```bash
cargo run --release --manifest-path womo-core/Cargo.toml -- forecast --mock --regions data/regions.geojson --out web/public/data
cp data/regions.geojson web/public/data/
cd web && npm install && npm run dev
```

Daten: Wetter © Open-Meteo.com (CC BY 4.0, DWD ICON), Stellplätze © OpenStreetMap-Mitwirkende (ODbL), Kartenumrisse Natural Earth (Public Domain).
