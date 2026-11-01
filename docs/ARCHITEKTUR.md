# Womo-Wetter „Sonnen-Radar“ – Architekturvorschlag

Ziel: Auf iPhone/iPad in Sekunden sehen, **in welchen Regionen (GAFOR-ähnlich) jetzt, in +2 h, +4 h und +6 h die Sonne scheint**, um spontan mit dem Wohnmobil loszufahren. Gebiet: Deutschland, Niederlande, Belgien, Luxemburg und ein grenznaher Streifen Frankreichs (Nord, Ardennen, Lothringen, Elsass/Vogesen).

---

## 1. Empfehlung in einem Satz

**Rust-Rechenkern, der stündlich per GitHub Actions läuft und fertige JSON-Dateien erzeugt + eine installierbare Web-App (PWA) für iPhone/iPad, gehostet auf GitHub Pages.**
Kein eigener Server, keine laufenden Kosten, kein App-Store nötig. Später optional als „echte“ App via Tauri 2 (Rust) verpackbar.

```
            stündlich (cron)                           auf Abruf
┌──────────────────────────────┐            ┌────────────────────────────┐
│ GitHub Actions               │            │ iPhone / iPad (PWA)         │
│  └─ womo-core (Rust CLI)     │            │  - Karte (SVG / MapLibre)   │
│     1. Wetterdaten holen     │  JSON/     │  - Zeit-Slots Jetzt…+6 h    │
│        (Open-Meteo, ICON-D2) │  GeoJSON   │  - Ranking „Beste Ziele“    │
│     2. Gitterpunkte → Region │ ─────────► │  - Detail-Zoom je Region    │
│     3. Klassen S/H/W/B/R/N   │  GitHub    │  - Offline-Cache (Service   │
│     4. JSON schreiben        │  Pages     │    Worker)                  │
└──────────────────────────────┘            └────────────────────────────┘
```

## 2. Varianten im Vergleich

| Variante | Rust-Anteil | iPhone/iPad | Aufwand | Kosten | Bewertung |
|---|---|---|---|---|---|
| **A. Rust-Batch + PWA (empfohlen)** | Datenaufbereitung, Bewertung, Regionen-Geometrie | PWA über „Zum Home-Bildschirm“ | gering–mittel | 0 € | schnell, robust, offline-fähig |
| B. Rust-Server (Axum) + PWA | API-Server | PWA | mittel | Server ~5 €/Monat | nur nötig für Push-Nachrichten/Benutzerdaten |
| C. Komplett Rust im Browser (Leptos/WASM) | alles | PWA | hoch | 0 € | geht, UI-Entwicklung aber zäher als mit TS |
| D. Tauri 2 Mobile (Rust + Web-UI) | Kern | echte iOS-App | hoch | Apple-Dev 99 €/Jahr, Mac+Xcode nötig | sinnvoller **2. Schritt**, gleiche UI wiederverwendbar |
| E. SwiftUI nativ | keiner | beste iOS-Integration | hoch | 99 €/Jahr | nur Apple, kein Rust |

**Warum A:** Die eigentliche „Intelligenz“ (Wetter je Gitterpunkt → Regionsklasse) ist ein klassischer Batch-Job – ideal für Rust (schnell, typsicher, gut testbar). Die Oberfläche als PWA läuft ohne App-Store auf iPhone und iPad, inklusive Vollbild und Home-Icon. Wird später eine echte App gewünscht, wird dieselbe Web-UI mit Tauri 2 verpackt und der Rust-Kern direkt eingebunden.

## 3. Datenquellen

| Quelle | Inhalt | Nutzung |
|---|---|---|
| **Open-Meteo API** (`models=icon_d2`, Fallback `icon_eu`) | stündlich `sunshine_duration`, `cloud_cover` (low/mid/high), `precipitation_probability`, `temperature_2m`, `wind_speed_10m`, `is_day` | Hauptquelle. ICON-D2 (DWD, 2,2 km) deckt DE/NL/BE/LU und Ostfrankreich ab, ~48 h Vorhersage, Update alle 3 h. Viele Koordinaten pro Anfrage möglich. Kostenlos für private Nutzung. |
| DWD Open Data / Bright Sky | Stationsmessungen Sonnenscheindauer der letzten Stunde | optional: Abgleich „Jetzt“ mit Messwerten (nur DE) |
| EUMETSAT / DWD Satellit (Bewölkung) | Ist-Zustand Wolken | optional Phase 3: „Jetzt“-Slot als Nowcast statt Modell |
| Sonnenauf-/-untergang | berechnet im Rust-Kern | Slots nach Sonnenuntergang = „Nacht“ |

## 4. Regionen („GAFOR-ähnlich“)

- **Deutschland:** Anlehnung an die DWD-GAFOR-Gebietseinteilung (Landschaftsräume wie Eifel, Sauerland, Schwarzwald …). Die Polygone werden einmalig digitalisiert (GeoJSON) – Vorlage: DWD-GAFOR-Gebietskarte. *Hinweis:* Die exakten amtlichen Grenzen sind nicht als freie GeoJSON verfügbar; wir zeichnen sie nach bzw. nähern sie an.
- **NL / BE / LU / FR-Grenzraum:** dort gibt es keine GAFOR-Gebiete im deutschen Sinn → eigene Regionen im gleichen Maßstab (z. B. Zeeland, Veluwe, Ardennen, Hohes Venn, Lothringen, Vogesen).
- Im Prototyp sind ~75 Regionen als Näherung automatisch erzeugt (Voronoi um Regionsmittelpunkte, an Ländergrenzen abgeschnitten).
- Pro Region **5–25 Stützpunkte** (Gitter im Polygon) → robuste Aussage, auch bei gemischtem Wetter.

## 5. Bewertungslogik (Rust-Kern)

Pro Stützpunkt und Stunde: Sonnenanteil `s = sunshine_duration / 3600`, Regenrisiko `r`.
Pro Region: Mittelwert + Anteil sonniger Punkte. Klassen wie bei GAFOR als Buchstaben (farbenblind-tauglich):

| Code | Bedeutung | Regel (Startwert, kalibrierbar) |
|---|---|---|
| **S** | Sonnig | s ≥ 75 % |
| **H** | Heiter | 50–75 % |
| **W** | Wolkig | 25–50 % |
| **B** | Bedeckt | < 25 % |
| **R** | Regen | Regenrisiko > 45 % |
| **N** | Nacht | nach Sonnenuntergang |

**„Beste Ziele“-Ranking:** Sonnenstunden ab gewähltem Zeitpunkt bis Sonnenuntergang, abzüglich Fahrzeit-Malus vom Startort (Womo-Tempo einstellbar, z. B. 75 km/h). Optional: Wochenend-Score Sa+So.

## 6. Datenformat (Ausgabe des Rust-Kerns)

```
/data/regions.geojson            # Regionsgrenzen (selten geändert)
/data/latest.json                # Übersicht: Klasse je Region × Slot (~20 KB)
/data/detail/<region-id>.json    # Feingitter + Stundenverlauf 48 h je Region
```
`latest.json` (Auszug):
```json
{ "run": "2026-11-07T09:00Z", "model": "icon_d2",
  "slots": ["2026-11-07T10:00+01:00", "...+2h", "...+4h", "...+6h"],
  "sunset": {"eifel": "17:03"},
  "regions": { "eifel": { "cls": ["S","H","R","S"], "sun": [0.94,0.6,0.1,0.8], "sun_h_today": 6.7 } } }
```

## 7. Projektstruktur (Monorepo)

```
womo-core/         Rust: Datenabruf, Geometrie, Bewertung, JSON-Export (+ Tests)
  src/fetch.rs     Open-Meteo-Client (reqwest, Retry, Batch)
  src/regions.rs   GeoJSON laden, Stützpunkte erzeugen (geo-Crate)
  src/score.rs     Klassen, Ranking, Sonnenstand
  src/export.rs    JSON-Ausgabe
web/               PWA: TypeScript + Vite, SVG-Karte (später MapLibre für Feinzoom)
  manifest.webmanifest, service-worker
data/regions/      handgepflegte Regions-GeoJSON
.github/workflows/ forecast.yml (stündlich), deploy-pages.yml
docs/              Architektur, Mockups
```

## 8. Oberfläche (siehe `docs/mockup/`)

1. **Übersicht:** Karte mit farbigen Regionen + Buchstaben-Code, darüber Segment-Schalter *Jetzt / +2 h / +4 h / +6 h* (mit Uhrzeit und Sonnenuntergangs-Hinweis), darunter Liste **„Beste Ziele“** mit Fahrzeit, Mini-Verlauf und Sonnenstunden.
2. **Detail (Tippen auf Region):** Karte zoomt animiert hinein, Feinraster der Sonne innerhalb der Region, Orte, Stundenverlauf, Kennzahlen (Sonne %, Sonnenstunden, Temperatur, Regenrisiko), Wochenend-Ausblick, Knopf **„Route starten“** (Apple Karten).
3. **iPhone:** Karte oben, Liste als Bottom-Sheet. **iPad:** Karte links, Seitenleiste rechts.
4. Hell/Dunkel-Modus automatisch, große Touch-Flächen, Buchstaben-Codes zusätzlich zur Farbe.

## 9. Umsetzungsphasen & Agenten-Einsatz

| Phase | Inhalt | Agenten (parallel) |
|---|---|---|
| 1 | Regions-GeoJSON (DE GAFOR-ähnlich + NL/BE/LU/FR) | Agent „Geo“ |
| 1 | Rust-Kern: Abruf, Bewertung, Export, Tests | Agent „Rust-Core“ |
| 1 | PWA-Grundgerüst aus Prototyp, liest `latest.json` | Agent „Web-UI“ |
| 2 | GitHub Actions (stündlich) + Pages-Deployment | Agent „CI/Deploy“ |
| 2 | Detail-Zoom mit echtem Feingitter, Startort per GPS | Web-UI |
| 3 | Optional: Push „Sonne in deiner Lieblingsregion“, Stellplatz-Overlay, Tauri-iOS-App | nach Bedarf |

Abschließend Review-Agent (Code-Review) und Test auf iPhone-/iPad-Viewport per Playwright.

## 10. Risiken / offene Punkte

- **GAFOR-Geometrie** muss nachgezeichnet werden (keine freie Quelle) → im Prototyp Näherung.
- **Open-Meteo**: frei nur für nicht-kommerzielle Nutzung; ~10 000 Aufrufe/Tag – reicht bei Batch-Abruf locker.
- **GitHub Pages** ist bei privaten Repos nur mit GitHub Pro verfügbar (Alternative: Cloudflare Pages, kostenlos).
- **GitHub-Actions-Cron** kann sich um einige Minuten verzögern – für stündliche Daten unkritisch.
- „Jetzt“ ist zunächst Modellwert, nicht Satellitenbild (Phase 3 möglich).
