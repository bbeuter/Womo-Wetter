# Regionen-Generator (`data/regions.geojson`)

Erzeugt die Regionsgeometrie für Womo-Wetter gemäß `docs/DATENFORMAT.md` §1.

**Gebiet:** Deutschland, Niederlande, Belgien, Luxemburg und ein ~90 km tiefer
Grenzstreifen Frankreichs (Côte d'Opale/Flandre, Ardennen, Champagne-Nord,
Lothringen, Elsass, Vogesen; Südgrenze Sundgau, 47,45° N).

> **Hinweis:** Die deutschen Regionen sind eine **Näherung** an die
> DWD-GAFOR-Gebietseinteilung (Namen/Granularität nach Landschaftsräumen),
> **keine amtliche Geometrie**. Die DWD-Gebietsgrenzen sind nicht als freie
> Geodaten verfügbar. NL/BE/LU/FR haben eigene Regionen im gleichen Maßstab.

## Verfahren

1. Ländergrenzen aus Natural Earth 10m `admin_0_countries` (DE, NL, BE, LU, FR).
2. Frankreich wird auf einen Puffer von 90 km um DE/BE/LU beschnitten und geglättet.
3. Pro Land Voronoi-Zellen um benannte Saatpunkte (`SEEDS` in `make_regions.py`),
   geschnitten mit dem Landesumriss → keine Region überschreitet eine Staatsgrenze.
   Gerechnet wird in einer lokalen km-Ebene (äquirektangulär um 51° N).
4. Abgetrennte Kleinteile (< 400 km²) gehen an die Nachbarregion mit der längsten
   gemeinsamen Grenze; Inseln (Sylt, Föhr, Fehmarn, Rügen, Watteninseln …) bleiben
   als `MultiPolygon` bei ihrer Region, Inseln < 8 km² entfallen.
5. Topologieerhaltende Vereinfachung (`shapely.coverage_simplify`, ~0,6 km),
   Koordinaten auf 4 Nachkommastellen, `center` per polylabel (liegt garantiert in der Region).

Regionen ändern = Saatpunkte in `SEEDS` verschieben/ergänzen/entfernen und neu erzeugen.
Die `id`s sind stabil, solange die Saatpunkt-ids gleich bleiben (der Rust-Kern und
`stellplaetze.json` referenzieren sie).

## Ausführen

```sh
# Abhängigkeiten (einmalig)
python3 -m venv /tmp/rvenv && /tmp/rvenv/bin/pip install "shapely>=2.1" matplotlib

# Natural Earth holen (nicht ins Repo einchecken)
mkdir -p /tmp/regions-dl
curl -sSL -o /tmp/regions-dl/ne_10m_admin_0_countries.geojson \
  https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_admin_0_countries.geojson

# erzeugen
/tmp/rvenv/bin/python -I tools/regions/make_regions.py \
  /tmp/regions-dl/ne_10m_admin_0_countries.geojson data/regions.geojson

# prüfen (+ optionale PNG-Vorschau)
/tmp/rvenv/bin/python -I tools/regions/check_regions.py \
  data/regions.geojson /tmp/regions-dl/ne_10m_admin_0_countries.geojson /tmp/regions-preview.png
```

`check_regions.py` prüft: eindeutige ids (`[a-z0-9-]+`), gültige Polygone,
`center` innerhalb, keine Überlappungen (> 0,05 km²), Abdeckung DE/NL/BE/LU ≥ 99 %,
und gibt Anzahl/Flächen je Land aus. Exit-Code ≠ 0 bei Fehlern.

Kartendaten: Natural Earth (gemeinfrei).
