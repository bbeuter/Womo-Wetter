// Erzeugt SYNTHETISCHE Beispieldaten für die Entwicklung der Web-App
// streng nach docs/DATENFORMAT.md:
//   public/data/regions.geojson, latest.json, grid.json, stellplaetze.json
// Regionen: aus dem Prototyp (docs/mockup/geo.js) zurückprojiziert und an
// Ländergrenzen (src/assets/countries.json) abgeschnitten.
// Aufruf: node scripts/make-sample-data.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import pc from 'polygon-clipping';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const outDir = path.join(root, 'public', 'data');
fs.mkdirSync(outDir, { recursive: true });

// ---------- Prototyp-Geometrie laden ----------
const geoSrc = fs.readFileSync(path.resolve(root, '../docs/mockup/geo.js'), 'utf8');
const { PROJ, REGIONS } = new Function(geoSrc + '\nreturn { PROJ, REGIONS };')();
const countries = JSON.parse(fs.readFileSync(path.join(root, 'src/assets/countries.json'), 'utf8'));

const merc = lat => Math.log(Math.tan(Math.PI / 4 + lat * Math.PI / 360));
const Y0 = merc(PROJ.lat0), Y1 = merc(PROJ.lat1);
const unproj = (x, y) => {
  const lon = PROJ.lon0 + x * (PROJ.lon1 - PROJ.lon0) / PROJ.W;
  const m = Y0 + (PROJ.H - y) / PROJ.H * (Y1 - Y0);
  return [lon, (2 * Math.atan(Math.exp(m)) - Math.PI / 2) * 180 / Math.PI];
};
const r3 = v => Math.round(v * 1000) / 1000;
const CC3 = { DE: 'DEU', NL: 'NLD', BE: 'BEL', LU: 'LUX', FR: 'FRA' };

function parsePath(d) {
  const ring = d.replace(/[MZ]/g, '').split('L').map(p => unproj(...p.split(',').map(Number)));
  ring.push(ring[0]);
  return [ring];
}
function ringArea(r) { let a = 0; for (let i = 0; i < r.length - 1; i++) a += r[i][0] * r[i + 1][1] - r[i + 1][0] * r[i][1]; return a / 2; }
function centroid(poly) {
  const r = poly[0]; let a = 0, x = 0, y = 0;
  for (let i = 0; i < r.length - 1; i++) { const f = r[i][0] * r[i + 1][1] - r[i + 1][0] * r[i][1]; a += f; x += (r[i][0] + r[i + 1][0]) * f; y += (r[i][1] + r[i + 1][1]) * f; }
  return a ? [x / (3 * a), y / (3 * a)] : r[0];
}
function inRing(pt, r) {
  let c = false;
  for (let i = 0, j = r.length - 1; i < r.length; j = i++) {
    if ((r[i][1] > pt[1]) !== (r[j][1] > pt[1]) && pt[0] < (r[j][0] - r[i][0]) * (pt[1] - r[i][1]) / (r[j][1] - r[i][1]) + r[i][0]) c = !c;
  }
  return c;
}
const inMulti = (pt, mp) => mp.some(poly => inRing(pt, poly[0]) && !poly.slice(1).some(h => inRing(pt, h)));

// Zellen je Land abschneiden, Lücken dem nächstgelegenen Regionsmittelpunkt zuschlagen
const geom = {};
for (const cc of Object.keys(CC3)) {
  const regs = REGIONS.filter(r => r.cc === cc);
  const land = countries[CC3[cc]];
  for (const r of regs) geom[r.id] = pc.intersection(parsePath(r.d), land);
  if (cc === 'FR') continue; // Frankreich nur als Grenzstreifen
  const covered = pc.union(...regs.map(r => geom[r.id]));
  const rest = pc.difference(land, covered);
  for (const piece of rest) {
    if (Math.abs(ringArea(piece[0])) < 1e-5) continue;
    const c = centroid(piece);
    let best = regs[0], bd = Infinity;
    for (const r of regs) { const d = (r.lon - c[0]) ** 2 + (r.lat - c[1]) ** 2; if (d < bd) { bd = d; best = r; } }
    geom[best.id] = pc.union(geom[best.id], [piece]);
  }
}
const features = [];
for (const r of REGIONS) {
  const mp = geom[r.id].map(poly => poly.map(ring => ring.map(([x, y]) => [r3(x), r3(y)])))
    .filter(poly => Math.abs(ringArea(poly[0])) > 2e-5);
  if (!mp.length) { console.warn('leer:', r.id); continue; }
  let center = [r.lon, r.lat];
  if (!inMulti(center, mp)) {
    const big = mp.slice().sort((a, b) => Math.abs(ringArea(b[0])) - Math.abs(ringArea(a[0])))[0];
    center = centroid(big).map(v => Math.round(v * 100) / 100);
    if (!inMulti(center, mp)) center = big[0][0];
  }
  features.push({
    type: 'Feature',
    properties: { id: r.id, name: r.name, cc: r.cc, center },
    geometry: mp.length === 1 ? { type: 'Polygon', coordinates: mp[0] } : { type: 'MultiPolygon', coordinates: mp },
  });
}
const regionsGeo = { type: 'FeatureCollection', features };
fs.writeFileSync(path.join(outDir, 'regions.geojson'), JSON.stringify(regionsGeo));
const mpOf = f => f.geometry.type === 'Polygon' ? [f.geometry.coordinates] : f.geometry.coordinates;

// ---------- Zeit (Europe/Berlin) ----------
const TZ = 'Europe/Berlin';
const fmtParts = new Intl.DateTimeFormat('en-CA', { timeZone: TZ, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hourCycle: 'h23' });
function berlin(ms) {
  const p = Object.fromEntries(fmtParts.formatToParts(new Date(ms)).map(x => [x.type, x.value]));
  const wall = Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour, +p.minute);
  const off = Math.round((wall - Math.floor(ms / 60000) * 60000) / 60000);
  return { y: +p.year, mo: +p.month, d: +p.day, h: +p.hour, mi: +p.minute, off };
}
const pad = n => String(n).padStart(2, '0');
function iso(ms) {
  const b = berlin(ms), s = b.off >= 0 ? '+' : '-', o = Math.abs(b.off);
  return `${b.y}-${pad(b.mo)}-${pad(b.d)}T${pad(b.h)}:${pad(b.mi)}${s}${pad(Math.floor(o / 60))}:${pad(o % 60)}`;
}
const now = Date.now();
const bn = berlin(now);
const midnight = Date.UTC(bn.y, bn.mo - 1, bn.d) - berlin(Date.UTC(bn.y, bn.mo - 1, bn.d)).off * 60000;
const hoursMs = Array.from({ length: 48 }, (_, i) => midnight + i * 3600000);
const hours = hoursMs.map(iso);
const days = Array.from({ length: 7 }, (_, i) => iso(midnight + i * 86400000 + 12 * 3600000).slice(0, 10));
const nowRel = (now - midnight) / 3600000; // Stunden seit Mitternacht

// Sonnenauf-/-untergang (vereinfachte NOAA-Formel), lokale Stunde (Dezimal)
function sunTimes(lon, lat, dateStr) {
  const [y, m, d] = dateStr.split('-').map(Number);
  const n = Math.round((Date.UTC(y, m - 1, d) - Date.UTC(y, 0, 0)) / 86400000);
  const g = 2 * Math.PI / 365 * (n - 1);
  const eqt = 229.18 * (0.000075 + 0.001868 * Math.cos(g) - 0.032077 * Math.sin(g) - 0.014615 * Math.cos(2 * g) - 0.040849 * Math.sin(2 * g));
  const decl = 0.006918 - 0.399912 * Math.cos(g) + 0.070257 * Math.sin(g) - 0.006758 * Math.cos(2 * g) + 0.000907 * Math.sin(2 * g);
  const phi = lat * Math.PI / 180;
  const ha = Math.acos(Math.cos(90.833 * Math.PI / 180) / (Math.cos(phi) * Math.cos(decl)) - Math.tan(phi) * Math.tan(decl)) * 180 / Math.PI;
  const off = berlin(Date.UTC(y, m - 1, d, 12)).off;
  const rise = (720 - 4 * (lon + ha) - eqt + off) / 60, set = (720 - 4 * (lon - ha) - eqt + off) / 60;
  return { rise, set };
}
const hm = h => `${pad(Math.floor(h))}:${pad(Math.round((h % 1) * 60) % 60)}`;

// ---------- Synthetisches Wetter ----------
// Kaltfront zieht von NW nach SO, dahinter Schauer/Aufklarung, morgen Nachmittag neues Tief von W.
function wx(lon, lat, t /* Stunden seit Mitternacht heute */) {
  const tr = t - nowRel;
  const u = lon * 0.62 - lat * 0.78 + 30;
  const front = -7.4 + tr * 0.45;
  const d = u - front;
  let cloud = Math.exp(-(d * d) / 1.1);
  let rain = Math.max(0, cloud - 0.5) * 2.2;
  if (d < -1.2) cloud = 0.15 + 0.3 * Math.max(0, Math.sin(lon * 3.1 + lat * 2.3 + tr * 0.7));
  if (d > 1.2) {
    const fog = Math.exp(-((lat - 48.6) ** 2) / 0.12) * (lon > 10 && lon < 13.6 ? 1 : 0) * Math.max(0, 1 - ((t % 24) - 8) / 4);
    cloud = Math.max(0.08, fog * 0.95);
  }
  const w2 = (lon - 2) - (t - 36) * 0.5; // zweites System von Westen ab morgen Mittag
  if (w2 < 1.5) { const k = Math.min(1, (1.5 - w2) / 2); cloud = Math.max(cloud, 0.85 * k); rain = Math.max(rain, w2 < 0 ? 0.6 * k : 0); }
  const noise = 0.1 * Math.sin(lon * 7.3 + lat * 5.1) * Math.cos(lat * 4.7 - tr);
  const sun = Math.min(1, Math.max(0, 1 - cloud + noise));
  const precip = rain > 0.5 ? rain * 2 : rain * 0.2;
  return { sun: Math.round(sun * 100), rain: Math.round(Math.min(1, rain) * 100), precip };
}

// ---------- Gitter ----------
const G = { lon0: 2.0, lat0: 47.0, step: 0.25, nx: 55, ny: 34 };
const cellRegion = new Array(G.nx * G.ny).fill(null);
const regionCells = Object.fromEntries(features.map(f => [f.properties.id, []]));
for (let j = 0; j < G.ny; j++) for (let i = 0; i < G.nx; i++) {
  const pt = [G.lon0 + i * G.step, G.lat0 + j * G.step];
  const f = features.find(f => inMulti(pt, mpOf(f)));
  if (f) { cellRegion[j * G.nx + i] = f.properties.id; regionCells[f.properties.id].push(pt); }
}
const isDay = (lon, lat, idx) => {
  const dateStr = hours[idx].slice(0, 10), h = +hours[idx].slice(11, 13);
  const { rise, set } = sunTimes(lon, lat, dateStr);
  return h + 0.5 >= rise && h + 0.5 < set;
};
const gridSun = hours.map((_, idx) => {
  let s = '';
  for (let k = 0; k < G.nx * G.ny; k++) {
    if (!cellRegion[k]) { s += '.'; continue; }
    const lon = G.lon0 + (k % G.nx) * G.step, lat = G.lat0 + Math.floor(k / G.nx) * G.step;
    if (!isDay(lon, lat, idx)) { s += 'N'; continue; }
    const w = wx(lon, lat, idx);
    s += w.rain >= 50 && w.precip >= 0.2 ? 'R' : String(Math.min(9, Math.floor(w.sun / 10)));
  }
  return s;
});
fs.writeFileSync(path.join(outDir, 'grid.json'), JSON.stringify({ ...G, hours, sun: gridSun }));

// ---------- latest.json ----------
const classOf = (sun, rain, precip, day) => !day ? 'N' : rain >= 50 && precip >= 0.2 ? 'R' : sun >= 75 ? 'S' : sun >= 50 ? 'H' : sun >= 25 ? 'W' : 'B';
const regionsOut = {};
for (const f of features) {
  const { id, center } = f.properties;
  const pts = regionCells[id].length ? regionCells[id] : [center];
  const hourly = { cls: '', sun: [], sunny: [], rain: [], temp: [], wind: [], wdir: [] };
  const precipArr = [];
  for (let idx = 0; idx < 48; idx++) {
    let s = 0, rn = 0, pr = 0, sunny = 0, dayN = 0;
    for (const [lon, lat] of pts) {
      const day = isDay(lon, lat, idx); dayN += day ? 1 : 0;
      const w = wx(lon, lat, idx);
      const sv = day ? w.sun : 0;
      s += sv; rn += w.rain; pr += w.precip; sunny += sv >= 50 ? 1 : 0;
    }
    const n = pts.length, day = dayN * 2 > n;
    const sun = day ? Math.round(s / n) : 0, rain = Math.round(rn / n), precip = pr / n;
    hourly.cls += classOf(sun, rain, precip, day);
    hourly.sun.push(sun); hourly.sunny.push(day ? Math.round(sunny / n * 100) : 0); hourly.rain.push(rain);
    precipArr.push(precip);
    const h = idx % 24, [lon, lat] = center;
    const diurnal = Math.sin(Math.max(0, Math.min(1, (h - 7) / 10)) * Math.PI);
    hourly.temp.push(Math.round(9 - (lat - 50) * 1.1 - (lon > 10 && lat < 48.5 ? 2 : 0) + diurnal * (2 + sun / 25) - (idx >= 24 ? 1 : 0)));
    hourly.wind.push(Math.round(12 + 10 * Math.abs(Math.sin(lon * 0.7 + idx * 0.15)) + (lat > 53 ? 10 : 0)));
    hourly.wdir.push(Math.round((240 + 40 * Math.sin(idx * 0.1 + lon * 0.3)) % 360));
  }
  const daily = days.map((date, di) => {
    const { rise, set } = sunTimes(center[0], center[1], date);
    const len = set - rise;
    let sun_h, tmax, tmin, rain, wind, wdir;
    if (di < 2) {
      const sl = a => a.slice(di * 24, di * 24 + 24);
      sun_h = sl(hourly.sun).reduce((a, b) => a + b, 0) / 100;
      tmax = Math.max(...sl(hourly.temp)); tmin = Math.min(...sl(hourly.temp)) - 2;
      rain = Math.max(...sl(hourly.rain)); wind = Math.max(...sl(hourly.wind)); wdir = sl(hourly.wdir)[14];
    } else {
      const f = 0.5 + 0.45 * Math.sin(di * 1.3 + center[0] * 0.4 - center[1] * 0.3);
      sun_h = len * Math.max(0.03, Math.min(0.95, f));
      tmax = Math.round(8 + f * 6 - (center[1] - 50)); tmin = tmax - 6 - Math.round(f * 3);
      rain = Math.round(Math.max(0, 0.7 - f) * 100); wind = Math.round(15 + 15 * (1 - f)); wdir = Math.round(200 + di * 20) % 360;
    }
    sun_h = Math.round(sun_h * 10) / 10;
    const rel = sun_h / len;
    const cls = rain >= 60 && sun_h < 2 ? 'R' : rel >= 0.6 ? 'S' : rel >= 0.4 ? 'H' : rel >= 0.2 ? 'W' : 'B';
    return { date, sunrise: hm(rise), sunset: hm(set), sun_h, tmax, tmin, rain, wind, wdir, cls };
  });
  regionsOut[id] = { hourly, daily };
}
const generated = iso(now - 25 * 60000);
fs.writeFileSync(path.join(outDir, 'latest.json'), JSON.stringify({
  generated, source: 'synthetisch (make-sample-data.mjs)', mock: true, hours, days, regions: regionsOut,
}));

// ---------- stellplaetze.json (fiktive Beispielpunkte, keine echten Plätze) ----------
let seed = 42;
const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const items = [];
let nid = 1000;
for (const f of features) {
  const mp = mpOf(f); const ring = mp[0][0];
  const xs = ring.map(p => p[0]), ys = ring.map(p => p[1]);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const n = 2 + Math.floor(rnd() * 7);
  for (let k = 0, tries = 0; k < n && tries < 200; tries++) {
    const pt = [x0 + rnd() * (x1 - x0), y0 + rnd() * (y1 - y0)];
    if (!inMulti(pt, mp)) continue;
    k++;
    const it = { id: 'n' + (nid++), name: `Beispiel-Stellplatz ${f.properties.name} ${k}`, lon: r3(pt[0]), lat: r3(pt[1]), region: f.properties.id, type: 'caravan_site' };
    const q = rnd();
    if (q < 0.45) it.fee = 'yes'; else if (q < 0.65) it.fee = 'no';
    if (rnd() < 0.4) it.website = 'https://example.org/stellplatz/' + it.id;
    if (k === 1 && rnd() < 0.3) delete it.name; // fehlender Name kommt in OSM vor
    items.push(it);
  }
}
fs.writeFileSync(path.join(outDir, 'stellplaetze.json'), JSON.stringify({ generated, items }));

for (const fn of ['regions.geojson', 'latest.json', 'grid.json', 'stellplaetze.json']) {
  console.log(fn.padEnd(18), (fs.statSync(path.join(outDir, fn)).size / 1024).toFixed(1), 'KB');
}
console.log(features.length, 'Regionen,', items.length, 'Stellplätze, Stand', generated);
