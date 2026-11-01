# Datenschnittstelle Rust-Kern → Web-App

Alle Dateien liegen nach dem Build unter `/data/` neben der Web-App (GitHub Pages).
Zeiten sind ISO-8601 mit Offset in `Europe/Berlin` (z. B. `2026-11-07T10:00+01:00`).
Zahlen sind ganzzahlig, wo möglich (kleinere Dateien).

## Datenbudget / Quelle

- Open-Meteo `https://api.open-meteo.com/v1/forecast`, `models=icon_seamless` (ICON-D2 2 km → ICON-EU → global),
  `timezone=Europe/Berlin`, `forecast_days=7`.
- Hourly-Variablen (nur diese 7): `sunshine_duration, precipitation_probability, precipitation, temperature_2m, wind_speed_10m, wind_direction_10m, is_day`
- **Keine Daily-Variablen** – alle Tageswerte werden aus den stündlichen Werten der 7 Tage abgeleitet (siehe §2 „Tageswerte“).
  Mit ≤ 10 Variablen zählt jeder Ort als **1 Aufruf** (Open-Meteo zählt Anfragen mit mehr als 10 Variablen anteilig mehrfach).
- Gitterpunkte: reguläres Gitter mit `step = 0.25°` über das Gebiet, nur Punkte, die in einer Region liegen
  (~1000 Punkte). Lauf alle 3 h → ~1000 × 8 ≈ 8 000 Abrufe/Tag < 10 000 (Open-Meteo-Freigrenze, jeder Ort zählt).
  Mehrere Orte pro Anfrage (kommagetrennt), Paketgröße 100.
- **Stunden-Zuordnung:** Bei Open-Meteo beschreiben `sunshine_duration`, `precipitation` und
  `precipitation_probability` zum Zeitstempel T die **vorangehende** Stunde (T−1 h … T).
  In den Ausgabedateien beschreibt die Stunde `h` den Zeitraum **h … h+1** (z. B. `10:00` = 10–11 Uhr).
  Der Rust-Kern nimmt daher für Stunde `h` diese Werte vom Zeitstempel `h+1` (fehlt er am Datenende, den von `h`).
  `temperature_2m`, `wind_*` und `is_day` sind Momentanwerte zum Zeitpunkt `h`.

## 1. `data/regions.geojson` (statisch, vom Geo-Werkzeug erzeugt)

FeatureCollection, Geometrie `Polygon` oder `MultiPolygon` (WGS84, lon/lat), bereits an Ländergrenzen/Küste abgeschnitten.

```json
{ "type": "Feature",
  "properties": { "id": "eifel", "name": "Eifel", "cc": "DE", "center": [6.7, 50.3] },
  "geometry": { "type": "Polygon", "coordinates": [[[6.1,50.1], ...]] } }
```
- `id`: ASCII-kleinbuchstaben, `[a-z0-9-]`, eindeutig.
- `cc`: `DE | NL | BE | LU | FR`.
- `center`: Punkt **innerhalb** der Region für Beschriftung.

## 2. `data/latest.json` (alle 3 h neu)

```json
{
  "generated": "2026-11-07T09:12+01:00",
  "source": "open-meteo icon_seamless",
  "mock": false,
  "hours": ["2026-11-07T00:00+01:00", "... 48 Einträge, stündlich ab 00:00 des aktuellen Tages"],
  "days":  ["2026-11-07", "... 7 Einträge"],
  "regions": {
    "eifel": {
      "hourly": {
        "cls":  "NNNNNNNSSHHRRWSSNN...",   // String, 1 Zeichen je Stunde (48)
        "sun":  [0, 0, 94, ...],           // Sonnenanteil 0–100 % (Mittel der Gitterpunkte)
        "sunny":[0, 0, 88, ...],           // Anteil Gitterpunkte mit Sonne ≥ 50 %, 0–100
        "rain": [0, 10, ...],              // Regenwahrscheinlichkeit 0–100 % (Mittel)
        "temp": [8, 9, ...],               // °C gerundet (Mittel)
        "wind": [12, 14, ...],             // km/h (Mittel)
        "wdir": [230, 240, ...]            // Grad (vektoriell gemittelt)
      },
      "daily": [
        { "date": "2026-11-07", "sunrise": "07:24", "sunset": "17:03",
          "sun_h": 6.7, "tmax": 14, "tmin": 4, "rain": 20, "wind": 18, "wdir": 240, "cls": "S" }
      ]
    }
  }
}
```

### Klassen (`cls`)
| Code | Bedeutung | Regel (Stunde) |
|---|---|---|
| `N` | Nacht | Mehrheit der Gitterpunkte ist Nacht. Ein Punkt gilt in Stunde h als Tag, wenn `is_day(h) == 1` **oder** `is_day(h+1) == 1` und in der Stunde Sonne > 0 (Sonnenaufgangsstunde) |
| `R` | Regen | Regenwahrscheinlichkeit ≥ 50 % **und** Niederschlag ≥ 0,2 mm im Mittel |
| `S` | Sonnig | sun ≥ 75 |
| `H` | Heiter | 50 ≤ sun < 75 |
| `W` | Wolkig | 25 ≤ sun < 50 |
| `B` | Bedeckt | sun < 25 |

`sun` pro Punkt und Stunde = `sunshine_duration / 3600 * 100`.
Tagesklasse `daily.cls` aus `sun_h` relativ zur Tageslänge: ≥ 60 % S, ≥ 40 % H, ≥ 20 % W, sonst B; R wenn `rain ≥ 60` und `sun_h < 2`.

### Tageswerte (`daily`) – aus Stundenwerten abgeleitet
Je Region und Ortstag (00–24 Uhr, an Umstellungstagen 23/25 h) aus den Regionsmitteln der Stunden:
- `sun_h`: Summe der stündlichen Sonnenanteile (Regionsmittel) in Stunden, 1 Nachkommastelle.
- `tmax` / `tmin`: Maximum / Minimum der stündlichen Regionsmittel-Temperatur.
- `rain`: Maximum der stündlichen Regenwahrscheinlichkeit (Regionsmittel) über die Tagstunden.
- `wind`: Maximum der stündlichen Windgeschwindigkeit (Regionsmittel).
- `wdir`: vektorielles Mittel der stündlichen Windrichtung über die Tagstunden.
- `sunrise` / `sunset`: berechnet (Sonnenstand, NOAA-Verfahren) am Regions-`center`, Ortszeit `HH:MM`.

## 3. `data/grid.json` (alle 3 h neu; für Detail-Zoom/Heatmap)

```json
{
  "lon0": 2.0, "lat0": 47.0, "step": 0.25, "nx": 55, "ny": 34,
  "hours": ["... gleiche 48 Stunden wie latest.json"],
  "sun": ["<Zeichenkette je Stunde>"]
}
```
- `sun[h]` ist ein String der Länge `nx*ny`, Zeile für Zeile ab Süden (`lat0`) nach Norden, innerhalb einer Zeile West → Ost.
  Zeichen: `'0'..'9'` = Sonnenanteil in Zehnerschritten (0–9 → 0 %…90 %+), `'R'` = Regen, `'N'` = Nacht, `'.'` = kein Datenpunkt.
- Zellmittelpunkt `(i, j)` = `(lon0 + i*step, lat0 + j*step)`.

## 4. `data/stellplaetze.json` (wöchentlich, OpenStreetMap / Overpass)

```json
{ "generated": "2026-11-02T03:00+01:00",
  "items": [ { "id": "n123456", "name": "Wohnmobilstellplatz am See", "lon": 6.81, "lat": 50.21,
               "region": "eifel", "type": "caravan_site", "fee": "yes", "website": "https://…" } ] }
```
- Quelle: Overpass, `tourism=caravan_site` (Knoten + Flächen-Mittelpunkte) im Gebiet.
- `region` per Punkt-in-Polygon gegen `regions.geojson`; Punkte außerhalb aller Regionen weglassen.
- Fehlende Felder weglassen (nicht `null`).
- Daten © OpenStreetMap-Mitwirkende, ODbL – Hinweis in der App anzeigen.
