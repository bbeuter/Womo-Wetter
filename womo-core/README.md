# womo-core

Rust-Rechenkern von Womo-Wetter. Holt die Wettervorhersage (Open-Meteo, Modell `icon_seamless`)
für ein regelmäßiges Gitter über allen Regionen, fasst sie je Region zusammen (Klassen
S/H/W/B/R/N, Sonnenanteil, Regen, Temperatur, Wind) und schreibt die JSON-Dateien für die
Web-App. Das Ausgabeformat ist in [`docs/DATENFORMAT.md`](../docs/DATENFORMAT.md) festgelegt.

## Befehle

```sh
cargo build --release

# Vorhersage → data/latest.json + data/grid.json
./target/release/womo-core forecast --regions ../data/regions.geojson --out ../data

# Ohne Netz: synthetisches, reproduzierbares Wetter (Kaltfront NW→SO, Donau-Nebel)
./target/release/womo-core forecast --regions ../data/regions.geojson --out ../data \
    --mock --now 2026-11-07T09:12:00+01:00

# Stellplätze aus OpenStreetMap (Overpass) → data/stellplaetze.json
./target/release/womo-core stellplaetze --regions ../data/regions.geojson --out ../data/stellplaetze.json
# … oder aus einer gespeicherten Overpass-Antwort
./target/release/womo-core stellplaetze --regions ../data/regions.geojson \
    --out ../data/stellplaetze.json --input overpass.json
```

Wichtige Optionen von `forecast`:

| Option | Bedeutung |
|---|---|
| `--step 0.25` | Gitterweite in Grad (Regionen mit < 3 Gitterpunkten bekommen Zusatzpunkte) |
| `--now <RFC 3339>` | Referenzzeit (Standard: jetzt); Stundenachse beginnt 00:00 Ortszeit dieses Tages |
| `--mock` | synthetisches Wetter, `"mock": true` in `latest.json` |
| `--pause 16` | Sekunden zwischen den Paketen à 100 Orten (Open-Meteo-Minutenlimit) |
| `--api-url`, `--apikey` | anderer Endpunkt / API-Schlüssel (auch per `OPEN_METEO_URL`, `OPEN_METEO_APIKEY`) |

## Tests

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Die Tests laufen komplett offline (Fixtures in `tests/fixtures/`, Fake-Server für den
Open-Meteo-Client in `tests/realmode.rs`).

## Einsatz in GitHub Actions

Der Echtbetrieb läuft per Cron alle 3 Stunden; die erzeugten Dateien werden mit der Web-App
auf GitHub Pages veröffentlicht. Kern des Jobs:

```yaml
- uses: dtolnay/rust-toolchain@stable
- uses: Swatinem/rust-cache@v2
  with: { workspaces: womo-core }
- run: cargo run --release --manifest-path womo-core/Cargo.toml -- forecast --regions data/regions.geojson --out web/public/data
```

Ein Lauf dauert wegen der Pausen zwischen den Paketen einige Minuten. Schlägt der Abruf fehl
(nach 3 Versuchen je Paket), endet das Programm mit Fehlercode ≠ 0 und schreibt nichts –
die zuletzt veröffentlichten Daten bleiben dann stehen. Stellplätze (`stellplaetze`) reichen
wöchentlich.

Datenquellen: Wetterdaten von [Open-Meteo](https://open-meteo.com) (CC BY 4.0),
Stellplätze © OpenStreetMap-Mitwirkende (ODbL).
