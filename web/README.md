# Sonnen-Radar – Web-App (PWA)

Installierbare Web-App für iPhone/iPad/Desktop: Wo scheint jetzt, in +2 h, +4 h und +6 h die Sonne?
Liest die vom Rust-Kern erzeugten Dateien (`data/regions.geojson`, `latest.json`, `grid.json`,
`stellplaetze.json`, siehe `docs/DATENFORMAT.md`). Vite + TypeScript (strict), ohne UI-Framework.

## Entwickeln

```bash
cd web
npm install
npm run sample-data   # erzeugt SYNTHETISCHE Beispieldaten nach public/data/ (nicht im Git)
npm run dev           # http://localhost:5173
```

Echte Daten: die Ausgabe von `womo-core` nach `web/public/data/` (bzw. beim Deployment nach `dist/data/`) kopieren.
Zum Testen kann die Uhrzeit überschrieben werden: `?now=2026-11-07T14`.

## Bauen

```bash
npm run build         # Typprüfung + dist/ (relativ, läuft auch unter /Womo-Wetter/ auf GitHub Pages)
npm run preview
```

`dist/sw.js` wird beim Build erzeugt (App-Shell cache-first, `data/*` network-first mit Offline-Fallback).

## Struktur

| Datei | Inhalt |
|---|---|
| `src/data.ts` | Typen laut Datenvertrag, Laden, Zeit-Helfer (Europe/Berlin) |
| `src/geo.ts` | Web-Mercator-Projektion, SVG-Pfade, Haversine/Fahrzeit |
| `src/map.ts` | SVG-Karte, Zoom-Animation, Heatmap aus `grid.json`, Orte, Stellplätze |
| `src/model.ts` | Slots, Sonnenuntergang, Ranking „Beste Ziele“ (Heute/Wochenende) |
| `src/list.ts`, `src/detail.ts`, `src/settings.ts` | Liste, Regions-Detail, Startort-/Tempo-Einstellungen |
| `src/store.ts`, `src/towns.ts` | localStorage (Startort, Tempo, Favoriten), eingebaute Ortsliste |
| `src/sw-template.js` | Service Worker (Vorlage) |
| `scripts/make-sample-data.mjs` | Beispieldaten aus `docs/mockup/geo.js` |
| `scripts/make-countries.mjs` | `src/assets/countries.json` aus Natural Earth 50m (gemeinfrei) |
| `scripts/make-icons.mjs` | PNG-Icons per Playwright |

Kartengrundlage: Natural Earth (gemeinfrei). Stellplätze: © OpenStreetMap-Mitwirkende (ODbL).
