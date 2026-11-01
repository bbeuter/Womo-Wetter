#!/usr/bin/env python3
"""Prüft data/regions.geojson und rendert optional eine PNG-Vorschau.

Aufruf:  python -I check_regions.py <regions.geojson> <ne_10m_admin_0_countries.geojson> [preview.png]
Benötigt: shapely >= 2.1, für die Vorschau zusätzlich matplotlib.
Exit-Code 1 bei Fehlern.
"""
import collections
import json
import math
import re
import sys

import shapely
from shapely.geometry import box, shape
from shapely.ops import unary_union

KX = 111.32 * math.cos(math.radians(51.0))
KY = 111.32
ISO = {"DE": "DEU", "NL": "NLD", "BE": "BEL", "LU": "LUX", "FR": "FRA"}


def km2(g):
    return shapely.transform(g, lambda c: c * [KX, KY]).area


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    fc = json.load(open(sys.argv[1], encoding="utf-8"))
    ne = json.load(open(sys.argv[2], encoding="utf-8"))
    errors = []
    feats = fc["features"]
    ids = [f["properties"]["id"] for f in feats]
    if len(ids) != len(set(ids)):
        errors.append("doppelte ids: %s" % [i for i, n in collections.Counter(ids).items() if n > 1])
    for f in feats:
        p = f["properties"]
        if not re.fullmatch(r"[a-z0-9-]+", p["id"]):
            errors.append(f"ungültige id {p['id']}")
        if p["cc"] not in ISO:
            errors.append(f"{p['id']}: cc {p['cc']}")
        if f["geometry"]["type"] not in ("Polygon", "MultiPolygon"):
            errors.append(f"{p['id']}: Geometrietyp")
    geoms = [shape(f["geometry"]) for f in feats]
    for f, g in zip(feats, geoms):
        p = f["properties"]
        if not g.is_valid:
            errors.append(f"{p['id']}: ungültig ({shapely.is_valid_reason(g)})")
        if not g.contains(shapely.Point(p["center"])):
            errors.append(f"{p['id']}: center liegt nicht in der Region")

    # Überlappungen
    tree = shapely.STRtree(geoms)
    worst = 0.0
    for i, g in enumerate(geoms):
        for j in tree.query(g):
            if j <= i:
                continue
            a = km2(g.intersection(geoms[j]))
            worst = max(worst, a)
            if a > 0.05:
                errors.append(f"Überlappung {ids[i]} / {ids[j]}: {a:.2f} km²")
    print(f"größte paarweise Überlappung: {worst:.4f} km²")

    # Abdeckung je Land
    ctry = {}
    for feat in ne["features"]:
        a3 = feat["properties"].get("ADM0_A3")
        for cc, iso in ISO.items():
            if a3 == iso:
                ctry[cc] = shapely.make_valid(shape(feat["geometry"])).intersection(box(1, 46.5, 16, 56))
    count = collections.Counter(f["properties"]["cc"] for f in feats)
    sizes = collections.defaultdict(list)
    for f, g in zip(feats, geoms):
        sizes[f["properties"]["cc"]].append(km2(g))
    print("Land  Anzahl  Abdeckung  Fläche min/median/max km²")
    for cc in ISO:
        u = unary_union([g for f, g in zip(feats, geoms) if f["properties"]["cc"] == cc])
        cov = km2(u.intersection(ctry[cc])) / km2(ctry[cc]) * 100
        s = sorted(sizes[cc])
        print(f"{cc}    {count[cc]:5d}   {cov:7.2f} %  {s[0]:.0f}/{s[len(s)//2]:.0f}/{s[-1]:.0f}"
              + ("   (Streifen)" if cc == "FR" else ""))
        if cc != "FR" and cov < 99.0:
            errors.append(f"{cc}: Abdeckung nur {cov:.2f} %")
        outside = km2(u.difference(ctry[cc]))
        if outside > 200:
            errors.append(f"{cc}: {outside:.0f} km² außerhalb des Landes")
    print(f"gesamt {len(feats)} Regionen")

    if len(sys.argv) > 3:
        preview(feats, geoms, sys.argv[3])

    for e in errors:
        print("FEHLER:", e)
    sys.exit(1 if errors else 0)


def preview(feats, geoms, out):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    pal = ["#8dd3c7", "#ffffb3", "#bebada", "#fb8072", "#80b1d3", "#fdb462",
           "#b3de69", "#fccde5", "#d9d9d9", "#bc80bd", "#ccebc5", "#ffed6f"]
    fig, ax = plt.subplots(figsize=(16, 18), dpi=110)
    for k, (f, g) in enumerate(zip(feats, geoms)):
        for poly in getattr(g, "geoms", [g]):
            x, y = poly.exterior.xy
            ax.fill([v * KX for v in x], [v * KY for v in y], color=pal[k % len(pal)],
                    ec="#333", lw=0.5)
        cx, cy = f["properties"]["center"]
        ax.text(cx * KX, cy * KY, f["properties"]["name"].replace("/", "/\n"),
                fontsize=5.5, ha="center", va="center")
    ax.set_aspect("equal")
    ax.axis("off")
    fig.tight_layout()
    fig.savefig(out)
    print("Vorschau →", out)


if __name__ == "__main__":
    main()
