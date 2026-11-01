#!/usr/bin/env python3
"""Erzeugt data/regions.geojson (siehe docs/DATENFORMAT.md §1).

Verfahren
---------
1. Ländergrenzen (Natural Earth 10m admin_0) für DE, NL, BE, LU, FR laden und in
   eine lokale, abstandstreue Ebene (km, äquirektangulär um 51° N) projizieren.
2. Frankreich auf einen Grenzstreifen beschneiden (Puffer um DE/BE/LU, ~95 km).
3. Pro Land: Voronoi-Zellen um benannte Saatpunkte (SEEDS), geschnitten mit
   dem Landesumriss → Regionen überschreiten nie eine Staatsgrenze.
4. Kleine abgetrennte Teilflächen (Exklaven durch Buchten/Grenzverlauf) werden
   dem Nachbarn mit der längsten gemeinsamen Grenze zugeschlagen; Inseln bleiben
   bei ihrer Region (MultiPolygon), winzige Inseln < MIN_ISLAND_KM2 entfallen.
5. Topologieerhaltend vereinfachen (shapely.coverage_simplify), zurück nach
   WGS84, auf 4 Nachkommastellen runden, Beschriftungspunkt per polylabel.

Aufruf:  python -I make_regions.py <ne_10m_admin_0_countries.geojson> <out.geojson>
Benötigt: shapely >= 2.1
"""
import json
import math
import re
import sys

import shapely
from shapely.geometry import MultiPoint, MultiPolygon, Polygon, box, mapping, shape
from shapely.ops import polylabel, unary_union

# --------------------------------------------------------------------------
# Saatpunkte: (id, Name, Land, lon, lat)
# Deutschland: Anlehnung an die DWD-GAFOR-Gebietseinteilung (Landschaftsräume),
# keine amtliche Geometrie.
# --------------------------------------------------------------------------
SEEDS = [
    # Schleswig-Holstein / Hamburg
    ("nordfriesland", "Nordfriesland/Inseln", "DE", 8.85, 54.62),
    ("schleswig", "Schleswig/Angeln", "DE", 9.75, 54.55),
    ("holstein", "Holstein/Dithmarschen", "DE", 9.55, 54.05),
    ("ostholstein", "Ostholstein/Fehmarn", "DE", 10.7, 54.15),
    # Mecklenburg-Vorpommern
    ("westmecklenburg", "Westmecklenburg", "DE", 11.2, 53.55),
    ("mecklenburger-kueste", "Mecklenburger Ostseeküste", "DE", 12.1, 54.0),
    ("mecklenburgische-seenplatte", "Mecklenburgische Seenplatte", "DE", 12.85, 53.45),
    ("vorpommern", "Vorpommern/Rügen", "DE", 13.55, 54.0),
    # Niedersachsen / Bremen / Hamburg
    ("ostfriesland", "Ostfriesland/Inseln", "DE", 7.45, 53.45),
    ("emsland", "Emsland", "DE", 7.35, 52.7),
    ("oldenburger-land", "Oldenburg/Bremen", "DE", 8.45, 52.95),
    ("elbe-weser", "Elbe-Weser-Dreieck", "DE", 9.15, 53.45),
    ("lueneburger-heide", "Lüneburger Heide", "DE", 10.05, 53.0),
    ("altmark-wendland", "Altmark/Wendland", "DE", 11.4, 52.75),
    ("hannover", "Hannover/Leinetal", "DE", 9.55, 52.4),
    ("braunschweiger-land", "Braunschweiger Land", "DE", 10.55, 52.3),
    ("weserbergland", "Weserbergland/Solling", "DE", 9.55, 51.85),
    ("harz", "Harz", "DE", 10.7, 51.72),
    # Brandenburg / Berlin
    ("prignitz", "Prignitz/Ruppin", "DE", 12.3, 53.05),
    ("uckermark", "Uckermark", "DE", 13.85, 53.15),
    ("berlin-havelland", "Berlin/Havelland", "DE", 13.0, 52.55),
    ("oderland", "Oderbruch/Oderland", "DE", 14.2, 52.45),
    ("flaeming", "Fläming", "DE", 12.95, 51.95),
    ("lausitz", "Spreewald/Niederlausitz", "DE", 14.0, 51.75),
    # Sachsen-Anhalt
    ("magdeburger-boerde", "Magdeburger Börde", "DE", 11.4, 52.1),
    ("anhalt", "Anhalt/Mittelelbe", "DE", 12.1, 51.8),
    ("saale-unstrut", "Halle/Saale-Unstrut", "DE", 11.6, 51.35),
    # Sachsen
    ("leipziger-bucht", "Leipziger Bucht", "DE", 12.6, 51.3),
    ("dresden", "Dresden/Elbsandstein", "DE", 13.75, 51.05),
    ("oberlausitz", "Oberlausitz", "DE", 14.6, 51.2),
    ("erzgebirge", "Erzgebirge", "DE", 13.0, 50.72),
    ("vogtland", "Vogtland", "DE", 12.2, 50.45),
    # Thüringen
    ("thueringer-becken", "Thüringer Becken", "DE", 10.85, 51.15),
    ("ostthueringen", "Ostthüringen/Saaletal", "DE", 11.8, 50.8),
    ("thueringer-wald", "Thüringer Wald", "DE", 10.75, 50.62),
    # Hessen
    ("nordhessen", "Nordhessen", "DE", 9.3, 51.2),
    ("vogelsberg", "Vogelsberg/Wetterau", "DE", 9.0, 50.5),
    ("rhoen", "Rhön", "DE", 9.95, 50.45),
    ("taunus", "Taunus", "DE", 8.25, 50.3),
    ("rhein-main", "Rhein-Main", "DE", 8.7, 50.0),
    ("spessart", "Spessart", "DE", 9.45, 50.0),
    ("odenwald", "Odenwald", "DE", 9.0, 49.6),
    # Nordrhein-Westfalen
    ("niederrhein", "Niederrhein", "DE", 6.4, 51.6),
    ("ruhrgebiet", "Ruhrgebiet", "DE", 7.25, 51.5),
    ("muensterland", "Münsterland", "DE", 7.45, 52.0),
    ("teutoburger-wald", "Teutoburger Wald/Ostwestfalen", "DE", 8.45, 52.0),
    ("sauerland", "Sauerland", "DE", 8.2, 51.3),
    ("bergisches-land", "Bergisches Land", "DE", 7.3, 51.08),
    ("siegerland", "Siegerland/Lahn-Dill", "DE", 8.25, 50.8),
    ("koeln-bonn", "Köln/Bonn/Aachen", "DE", 6.6, 50.85),
    # Rheinland-Pfalz / Saarland
    ("eifel", "Eifel", "DE", 6.75, 50.3),
    ("westerwald", "Westerwald", "DE", 7.7, 50.55),
    ("hunsrueck", "Hunsrück/Nahe", "DE", 7.4, 49.95),
    ("mosel", "Mosel/Trier", "DE", 6.75, 49.8),
    ("rheinhessen", "Rheinhessen", "DE", 8.1, 49.8),
    ("pfalz", "Pfalz/Pfälzerwald", "DE", 7.65, 49.35),
    ("saarland", "Saarland", "DE", 6.95, 49.38),
    # Baden-Württemberg
    ("rhein-neckar", "Rhein-Neckar/Kraichgau", "DE", 8.6, 49.3),
    ("oberrhein-nord", "Oberrhein Nord/Karlsruhe", "DE", 8.2, 48.85),
    ("nordschwarzwald", "Nordschwarzwald", "DE", 8.55, 48.55),
    ("breisgau", "Oberrhein Süd/Breisgau", "DE", 7.68, 48.1),
    ("suedschwarzwald", "Schwarzwald Süd/Baar", "DE", 8.3, 47.95),
    ("neckarland", "Neckarland/Stuttgart", "DE", 9.25, 48.85),
    ("hohenlohe", "Hohenlohe/Taubertal", "DE", 9.75, 49.35),
    ("schwaebische-alb", "Schwäbische Alb", "DE", 9.2, 48.35),
    ("ostalb", "Ostalb/Ries", "DE", 10.25, 48.85),
    ("bodensee", "Bodensee/Oberschwaben", "DE", 9.45, 47.85),
    # Bayern
    ("mainfranken", "Mainfranken", "DE", 10.05, 49.9),
    ("frankenwald", "Frankenwald/Fichtelgebirge", "DE", 11.65, 50.15),
    ("fraenkische-alb", "Fränkische Schweiz/Alb", "DE", 11.3, 49.6),
    ("mittelfranken", "Mittelfranken/Altmühltal", "DE", 10.65, 49.25),
    ("oberpfalz", "Oberpfalz", "DE", 12.15, 49.45),
    ("bayerischer-wald", "Bayerischer Wald", "DE", 13.2, 48.95),
    ("niederbayern", "Niederbayern/Regensburg", "DE", 12.35, 48.75),
    ("rottal-inn", "Rottal/Inn", "DE", 12.95, 48.4),
    ("donautal", "Donautal/Ingolstadt", "DE", 11.4, 48.75),
    ("augsburg", "Schwaben/Augsburg", "DE", 10.55, 48.35),
    ("allgaeu", "Allgäu", "DE", 10.3, 47.72),
    ("muenchen", "München/Oberbayern", "DE", 11.65, 48.2),
    ("oberland", "Oberland/Werdenfels", "DE", 11.3, 47.68),
    ("chiemgau", "Chiemgau/Berchtesgaden", "DE", 12.5, 47.8),
    # Niederlande
    ("wadden-friesland", "Waddeneilanden/Friesland", "NL", 5.75, 53.15),
    ("groningen-drenthe", "Groningen/Drenthe", "NL", 6.65, 53.0),
    ("noord-holland", "Noord-Holland", "NL", 4.85, 52.65),
    ("flevoland", "Flevoland/IJsselmeer", "NL", 5.55, 52.6),
    ("zuid-holland", "Zuid-Holland", "NL", 4.45, 51.95),
    ("utrecht-rivierenland", "Utrecht/Rivierenland", "NL", 5.25, 52.0),
    ("veluwe", "Veluwe", "NL", 5.85, 52.22),
    ("overijssel-twente", "Overijssel/Twente", "NL", 6.55, 52.4),
    ("achterhoek", "Achterhoek", "NL", 6.45, 51.95),
    ("zeeland", "Zeeland", "NL", 3.85, 51.5),
    ("west-brabant", "West-Brabant", "NL", 4.75, 51.55),
    ("oost-brabant", "Oost-Brabant/Peel", "NL", 5.6, 51.5),
    ("limburg-nl", "Limburg (NL)", "NL", 5.95, 51.15),
    # Belgien
    ("westhoek", "Westhoek/Belgische Küste", "BE", 2.85, 51.05),
    ("flandern", "Flandern (Gent)", "BE", 3.75, 50.98),
    ("antwerpen", "Antwerpen", "BE", 4.5, 51.2),
    ("kempen", "Kempen/Limburg (BE)", "BE", 5.3, 51.05),
    ("brabant", "Brüssel/Brabant", "BE", 4.45, 50.75),
    ("hennegau", "Hennegau", "BE", 3.95, 50.45),
    ("namur", "Namur/Condroz", "BE", 4.9, 50.35),
    ("luettich", "Lüttich/Hohes Venn", "BE", 5.85, 50.5),
    ("ardennen", "Ardennen", "BE", 5.35, 50.05),
    ("gaume", "Gaume/Belg. Luxemburg", "BE", 5.6, 49.7),
    # Luxemburg
    ("oesling", "Ösling", "LU", 6.0, 50.0),
    ("gutland", "Gutland", "LU", 6.15, 49.65),
    # Frankreich (Grenzstreifen)
    ("cote-d-opale", "Côte d'Opale", "FR", 1.95, 50.72),
    ("flandre-lille", "Flandre/Lille", "FR", 3.0, 50.55),
    ("avesnois", "Avesnois/Thiérache", "FR", 3.85, 50.05),
    ("ardennen-fr", "Ardennen (FR)", "FR", 4.7, 49.75),
    ("champagne-nord", "Champagne (Nord)", "FR", 4.05, 49.35),
    ("argonne-meuse", "Argonne/Maas", "FR", 5.3, 49.2),
    ("lothringen-nord", "Lothringen (Metz/Thionville)", "FR", 6.2, 49.2),
    ("lothringen-ost", "Lothringen (Ost)", "FR", 6.95, 48.95),
    ("lothringen-sued", "Lothringen (Nancy)", "FR", 6.2, 48.65),
    ("elsass-nord", "Elsass (Nord)", "FR", 7.7, 48.75),
    ("vogesen", "Vogesen", "FR", 6.85, 48.05),
    ("elsass-sued", "Elsass (Süd)", "FR", 7.4, 47.95),
]

ISO = {"DE": "DEU", "NL": "NLD", "BE": "BEL", "LU": "LUX", "FR": "FRA"}
FR_STRIP_KM = 90.0          # Tiefe des französischen Streifens ab DE/BE/LU-Grenze
FR_MIN_LAT = 47.45         # Südgrenze des Streifens (Sundgau)
SIMPLIFY_KM = 0.6           # ~0.008° – topologieerhaltende Vereinfachung
MIN_ISLAND_KM2 = 8.0        # kleinere Inseln entfallen
EXCLAVE_KM2 = 400.0         # kleinere abgetrennte Teile → Nachbarregion
WINDOW = box(1.0, 46.5, 16.0, 56.0)   # nur Mitteleuropa (lon/lat)

# lokale Projektion (km), äquirektangulär um 51° N / 10° O
LAT0, LON0 = 51.0, 10.0
KX = 111.32 * math.cos(math.radians(LAT0))
KY = 111.32


def to_km(g):
    return shapely.transform(g, lambda c: (c - [LON0, 0.0]) * [KX, KY])


def to_deg(g):
    return shapely.transform(g, lambda c: c / [KX, KY] + [LON0, 0.0])


def polys(g):
    if g.is_empty:
        return []
    if isinstance(g, Polygon):
        return [g]
    if isinstance(g, MultiPolygon):
        return list(g.geoms)
    return [p for p in getattr(g, "geoms", []) for p in polys(p)]


def load_countries(path):
    with open(path, encoding="utf-8") as f:
        fc = json.load(f)
    out = {}
    for feat in fc["features"]:
        a3 = feat["properties"].get("ADM0_A3")
        for cc, iso in ISO.items():
            if a3 == iso:
                g = shape(feat["geometry"]).intersection(WINDOW)
                out[cc] = to_km(shapely.make_valid(g))
    missing = set(ISO) - set(out)
    if missing:
        sys.exit(f"Länder fehlen in {path}: {missing}")
    return out


def build(ne_path):
    ctry = load_countries(ne_path)

    # französischer Grenzstreifen
    core = unary_union([ctry["DE"], ctry["BE"], ctry["LU"]])
    strip = ctry["FR"].intersection(core.buffer(FR_STRIP_KM, quad_segs=16))
    strip = strip.intersection(to_km(box(1.5, FR_MIN_LAT, 9.0, 51.2)))  # nicht südlich des Sundgaus
    strip = strip.buffer(-15).buffer(15).intersection(ctry["FR"])  # Zipfel glätten
    ctry["FR"] = strip

    # gemeinsame topologieerhaltende Vereinfachung aller Landesflächen
    order = list(ISO)
    simp = shapely.coverage_simplify([ctry[c] for c in order], SIMPLIFY_KM)
    for c, g in zip(order, simp):
        ctry[c] = shapely.make_valid(g)

    regions = {}
    for cc in order:
        seeds = [s for s in SEEDS if s[2] == cc]
        pts = [to_km(shapely.Point(s[3], s[4])) for s in seeds]
        area = ctry[cc]
        env = area.envelope.buffer(500)
        cells = shapely.voronoi_polygons(MultiPoint(pts), extend_to=env)
        for cell in cells.geoms:
            hit = [i for i, p in enumerate(pts) if cell.contains(p)]
            assert len(hit) == 1
            s = seeds[hit[0]]
            g = cell.intersection(area)
            regions[s[0]] = {"seed": s, "geom": shapely.make_valid(g)}

    fix_exclaves(regions)
    return regions


def fix_exclaves(regions):
    """Abgetrennte Teilflächen dem Nachbarn mit längster Grenze geben, Mini-Inseln weg."""
    ids = list(regions)
    for rid in ids:
        parts = sorted(polys(regions[rid]["geom"]), key=lambda p: -p.area)
        keep = [parts[0]] if parts else []
        for p in parts[1:]:
            best, blen = None, 0.0
            if p.area < EXCLAVE_KM2:
                for oid in ids:
                    if oid == rid or regions[oid]["seed"][2] != regions[rid]["seed"][2]:
                        continue
                    og = regions[oid]["geom"]
                    if not p.buffer(0.05).intersects(og):
                        continue
                    ln = p.boundary.intersection(og.buffer(0.05)).length
                    if ln > blen:
                        best, blen = oid, ln
            if best:
                regions[best]["geom"] = unary_union([regions[best]["geom"], p])
            elif p.area >= MIN_ISLAND_KM2:
                keep.append(p)
        regions[rid]["geom"] = unary_union(keep) if keep else Polygon()
    for r in regions.values():
        r["geom"] = unary_union(polys(r["geom"]))


def rounded(g, nd=4):
    g = shapely.set_precision(g, 10 ** -nd)
    return shapely.make_valid(g)


def round_coords(m, nd=4):
    def rc(x):
        if isinstance(x[0], (int, float)):
            return [round(x[0], nd), round(x[1], nd)]
        return [rc(y) for y in x]
    return {"type": m["type"], "coordinates": rc(m["coordinates"])}


def to_feature(r):
    rid, name, cc = r["seed"][:3]
    gkm = r["geom"]
    biggest = max(polys(gkm), key=lambda p: p.area)
    c = polylabel(biggest, tolerance=1.0)
    if not gkm.contains(c):
        c = biggest.representative_point()
    cdeg = to_deg(c)
    gdeg = rounded(to_deg(gkm))
    ps = polys(gdeg)
    geom = ps[0] if len(ps) == 1 else MultiPolygon(ps)
    geom = shapely.orient_polygons(geom)  # RFC 7946: außen gegen Uhrzeigersinn
    return {
        "type": "Feature",
        "properties": {"id": rid, "name": name, "cc": cc,
                       "center": [round(cdeg.x, 3), round(cdeg.y, 3)]},
        "geometry": round_coords(mapping(geom)),
    }


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    regions = build(sys.argv[1])
    ids = [s[0] for s in SEEDS]
    assert len(ids) == len(set(ids)), "doppelte ids"
    assert all(re.fullmatch(r"[a-z0-9-]+", i) for i in ids), "ungültige id"
    feats = [to_feature(regions[i]) for i in ids if not regions[i]["geom"].is_empty]
    fc = {"type": "FeatureCollection", "features": feats}
    with open(sys.argv[2], "w", encoding="utf-8") as f:
        json.dump(fc, f, ensure_ascii=False, separators=(",", ":"))
        f.write("\n")
    print(f"{len(feats)} Regionen → {sys.argv[2]}")


if __name__ == "__main__":
    main()
