// Projektion (Web-Mercator in eine SVG-viewBox), Pfade, Distanzen.
import type { LonLat, Polygon, Region } from './data';

export const VB_W = 1000;
const merc = (lat: number): number => Math.log(Math.tan(Math.PI / 4 + (lat * Math.PI) / 360));

export interface Projection {
  W: number; H: number;
  lon0: number; lon1: number; lat0: number; lat1: number;
  proj(lon: number, lat: number): [number, number];
  unproj(x: number, y: number): LonLat;
}

/** Projektion, die die Regionen (plus Rand) in eine 1000 breite viewBox abbildet. */
export function makeProjection(regions: Region[], padDeg = 0.35): Projection {
  let lon0 = Infinity, lon1 = -Infinity, lat0 = Infinity, lat1 = -Infinity;
  for (const r of regions) for (const poly of r.polys) for (const [x, y] of poly[0]) {
    if (x < lon0) lon0 = x; if (x > lon1) lon1 = x; if (y < lat0) lat0 = y; if (y > lat1) lat1 = y;
  }
  lon0 -= padDeg; lon1 += padDeg; lat0 -= padDeg * 0.6; lat1 += padDeg * 0.6;
  const k = VB_W / (((lon1 - lon0) * Math.PI) / 180); // Einheiten je Radiant
  const Y1 = merc(lat1);
  const H = Math.round((Y1 - merc(lat0)) * k);
  return {
    W: VB_W, H, lon0, lon1, lat0, lat1,
    proj: (lon, lat) => [((lon - lon0) * Math.PI / 180) * k, (Y1 - merc(lat)) * k],
    unproj: (x, y) => [lon0 + (x / k) * 180 / Math.PI, (2 * Math.atan(Math.exp(Y1 - y / k)) - Math.PI / 2) * 180 / Math.PI],
  };
}

const r1 = (v: number): number => Math.round(v * 10) / 10;
export function pathOf(polys: Polygon[], P: Projection): string {
  let d = '';
  for (const poly of polys) for (const ring of poly) {
    let prev = '';
    ring.forEach(([lon, lat], i) => {
      const [x, y] = P.proj(lon, lat);
      const s = `${r1(x)},${r1(y)}`;
      if (s === prev) return;
      d += (i === 0 ? 'M' : 'L') + s; prev = s;
    });
    d += 'Z';
  }
  return d;
}
export interface BBox { x: number; y: number; w: number; h: number }
export function bboxOf(polys: Polygon[], P: Projection): BBox {
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const poly of polys) for (const [lon, lat] of poly[0]) {
    const [x, y] = P.proj(lon, lat);
    if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
  }
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

export interface Pt { lon: number; lat: number }
export function haversineKm(a: Pt, b: Pt): number {
  const R = 6371, rad = Math.PI / 180;
  const dLat = (b.lat - a.lat) * rad, dLon = (b.lon - a.lon) * rad;
  const x = Math.sin(dLat / 2) ** 2 + Math.cos(a.lat * rad) * Math.cos(b.lat * rad) * Math.sin(dLon / 2) ** 2;
  return 2 * R * Math.asin(Math.sqrt(x));
}
export interface Drive { km: number; hours: number; txt: string }
/** Fahrstrecke ≈ Luftlinie × 1,3; Fahrzeit mit Womo-Tempo. */
export function drive(from: Pt, to: Pt, speedKmh: number): Drive {
  const km = Math.round(haversineKm(from, to) * 1.3);
  const min = Math.round((km / speedKmh) * 60);
  return { km, hours: min / 60, txt: `${Math.floor(min / 60)}:${String(min % 60).padStart(2, '0')} h` };
}

function inRing(lon: number, lat: number, r: LonLat[]): boolean {
  let c = false;
  for (let i = 0, j = r.length - 1; i < r.length; j = i++) {
    const [xi, yi] = r[i], [xj, yj] = r[j];
    if ((yi > lat) !== (yj > lat) && lon < ((xj - xi) * (lat - yi)) / (yj - yi) + xi) c = !c;
  }
  return c;
}
export const inRegion = (lon: number, lat: number, r: Region): boolean =>
  r.polys.some(p => inRing(lon, lat, p[0]) && !p.slice(1).some(h => inRing(lon, lat, h)));
