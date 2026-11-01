// Erzeugt src/assets/countries.json aus Natural Earth 50m (public domain).
// Aufruf: node scripts/make-countries.mjs [/tmp/ne.json]
import fs from 'node:fs';
import pc from 'polygon-clipping';

const src = process.argv[2] || '/tmp/ne.json';
const CODES = ['DEU', 'NLD', 'BEL', 'LUX', 'FRA', 'CHE', 'AUT', 'DNK', 'POL', 'CZE'];
const BBOX = [[[0.5, 45.0], [17.5, 45.0], [17.5, 56.5], [0.5, 56.5], [0.5, 45.0]]];
const TOL = 0.012; // Grad
const ne = JSON.parse(fs.readFileSync(src, 'utf8'));

function dp(pts, tol) {
  if (pts.length < 4) return pts;
  const keep = new Uint8Array(pts.length); keep[0] = keep[pts.length - 1] = 1;
  const stack = [[0, pts.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop();
    const [ax, ay] = pts[a], [bx, by] = pts[b];
    const dx = bx - ax, dy = by - ay, L = Math.hypot(dx, dy) || 1e-12;
    let md = -1, mi = -1;
    for (let i = a + 1; i < b; i++) {
      const d = Math.abs(dy * pts[i][0] - dx * pts[i][1] + bx * ay - by * ax) / L;
      if (d > md) { md = d; mi = i; }
    }
    if (md > tol) { keep[mi] = 1; stack.push([a, mi], [mi, b]); }
  }
  return pts.filter((_, i) => keep[i]);
}
// Geschlossener Ring: an der Hälfte teilen, damit DP nicht an identischen Endpunkten scheitert
function dpRing(r, tol) {
  const m = Math.floor(r.length / 2);
  const a = dp(r.slice(0, m + 1), tol), b = dp(r.slice(m), tol);
  return a.concat(b.slice(1));
}
const r3 = v => Math.round(v * 1000) / 1000;
const out = {};
for (const code of CODES) {
  const f = ne.features.find(x => x.properties.ADM0_A3 === code);
  const polys = f.geometry.type === 'Polygon' ? [f.geometry.coordinates] : f.geometry.coordinates;
  const clipped = pc.intersection(polys, BBOX);
  const res = [];
  for (const poly of clipped) {
    const rings = poly.map(r => dpRing(r, TOL).map(([x, y]) => [r3(x), r3(y)])).filter(r => r.length >= 4);
    if (rings.length && rings[0].length >= 4) res.push(rings);
  }
  out[code] = res;
}
fs.writeFileSync(new URL('../src/assets/countries.json', import.meta.url), JSON.stringify(out));
console.log('countries.json', JSON.stringify(out).length, 'bytes');
