// Klickbarer UI-Prototyp. Alle Wetterwerte sind SYNTHETISCH (Beispiel-Szenario:
// Kaltfront zieht von Nordwest nach Südost, dahinter Aufklarung, Nebel an der Donau).
'use strict';
const NS = 'http://www.w3.org/2000/svg';
const NOW_H = 10, SUNSET = 17.05, SUNRISE = 7.2;
const SLOTS = [0, 2, 4, 6];
const CLASSES = {
  S: { label: 'Sonnig', color: 'var(--S)', icon: '☀︎' },
  H: { label: 'Heiter', color: 'var(--H)', icon: '🌤' },
  W: { label: 'Wolkig', color: 'var(--W)', icon: '⛅' },
  B: { label: 'Bedeckt', color: 'var(--B)', icon: '☁︎' },
  R: { label: 'Regen', color: 'var(--R)', icon: '🌧' },
  N: { label: 'Nacht', color: 'var(--N)', icon: '☾' },
};
const HOME = { name: 'Köln', lon: 6.96, lat: 50.94 };
const TOWNS = [
  ['Köln', 6.96, 50.94], ['Aachen', 6.08, 50.78], ['Trier', 6.64, 49.75], ['Koblenz', 7.59, 50.36],
  ['Freiburg', 7.85, 47.99], ['München', 11.58, 48.14], ['Dresden', 13.74, 51.05], ['Berlin', 13.4, 52.52],
  ['Hamburg', 9.99, 53.55], ['Amsterdam', 4.9, 52.37], ['Brüssel', 4.35, 50.85], ['Lüttich', 5.57, 50.63],
  ['Luxemburg', 6.13, 49.61], ['Straßburg', 7.75, 48.58], ['Metz', 6.18, 49.12], ['Nürnberg', 11.08, 49.45],
  ['Frankfurt', 8.68, 50.11], ['Stuttgart', 9.18, 48.78], ['Leipzig', 12.37, 51.34], ['Hannover', 9.73, 52.37],
  ['Bastogne', 5.72, 50.0], ['Monschau', 6.24, 50.55], ['Cochem', 7.17, 50.15], ['Daun', 6.83, 50.2],
  ['Bitburg', 6.53, 49.97], ['Prüm', 6.42, 50.21], ['Bernkastel', 7.07, 49.92], ['Passau', 13.43, 48.57],
  ['Garmisch', 11.1, 47.49], ['Rügen', 13.4, 54.45], ['Sylt', 8.3, 54.9], ['Texel', 4.8, 53.07],
];

// --- Projektion (gleich wie in geo.js erzeugt) ---
const merc = lat => Math.log(Math.tan(Math.PI / 4 + lat * Math.PI / 360));
const Y0 = merc(PROJ.lat0), Y1 = merc(PROJ.lat1);
const proj = (lon, lat) => [(lon - PROJ.lon0) * PROJ.W / (PROJ.lon1 - PROJ.lon0), PROJ.H - (merc(lat) - Y0) / (Y1 - Y0) * PROJ.H];
const unproj = (x, y) => {
  const lon = PROJ.lon0 + x * (PROJ.lon1 - PROJ.lon0) / PROJ.W;
  const m = Y0 + (PROJ.H - y) / PROJ.H * (Y1 - Y0);
  return [lon, (2 * Math.atan(Math.exp(m)) - Math.PI / 2) * 180 / Math.PI];
};

// --- Synthetisches Wetter: Sonnenanteil 0..1 und Regenwahrscheinlichkeit 0..1 ---
function wx(lon, lat, h) {
  if (h >= SUNSET || h < SUNRISE) return { sun: 0, rain: 0, night: true };
  const t = h - NOW_H;
  const u = lon * 0.62 - lat * 0.78 + 30;          // Achse quer zur Front
  const front = -7.4 + t * 0.6;                    // Front wandert nach SO
  const d = u - front;
  let cloud = Math.exp(-(d * d) / 1.1);           // Frontbewölkung
  let rain = Math.max(0, cloud - 0.55) * 2;
  if (d < -1.2) cloud = 0.15 + 0.25 * Math.max(0, Math.sin(lon * 3.1 + lat * 2.3 + t)); // Rückseite: Schauerwolken
  if (d > 1.2) {                                   // Vorderseite: sonnig, aber Donaunebel am Vormittag
    const fog = Math.exp(-((lat - 48.6) ** 2) / 0.12) * (lon > 10 && lon < 13.6 ? 1 : 0) * Math.max(0, 1 - t / 4.5);
    cloud = Math.max(0.08, fog * 0.95);
  }
  const noise = 0.08 * Math.sin(lon * 7.3 + lat * 5.1) * Math.cos(lat * 4.7 - t);
  const sun = Math.min(1, Math.max(0, 1 - cloud + noise));
  return { sun, rain: Math.min(1, rain), night: false };
}
function classify(w) {
  if (w.night) return 'N';
  if (w.rain > 0.45) return 'R';
  return w.sun >= 0.75 ? 'S' : w.sun >= 0.5 ? 'H' : w.sun >= 0.25 ? 'W' : 'B';
}
// Regionswert = Mittel über Stützpunkte im Umkreis (wie später: mehrere Gitterpunkte je Region)
function regionWx(r, h) {
  let s = 0, rn = 0, n = 0, night = false;
  for (const [dx, dy] of [[0, 0], [-.3, -.2], [.3, .2], [-.3, .2], [.3, -.2]]) {
    const w = wx(r.lon + dx, r.lat + dy, h); s += w.sun; rn += w.rain; n++; night = w.night;
  }
  return { sun: s / n, rain: rn / n, night };
}
const hav = (a, b) => {
  const R = 6371, dLat = (b.lat - a.lat) * Math.PI / 180, dLon = (b.lon - a.lon) * Math.PI / 180;
  const x = Math.sin(dLat / 2) ** 2 + Math.cos(a.lat * Math.PI / 180) * Math.cos(b.lat * Math.PI / 180) * Math.sin(dLon / 2) ** 2;
  return 2 * R * Math.asin(Math.sqrt(x));
};
const drive = r => { const km = Math.round(hav(HOME, r) * 1.3); const min = Math.round(km / 75 * 60); return { km, txt: `${Math.floor(min / 60)}:${String(min % 60).padStart(2, '0')} h` }; };
const fmtH = h => `${String(Math.floor(h)).padStart(2, '0')}:00`;

// --- State ---
const state = { slot: 0, region: null };
const $ = id => document.getElementById(id);
const el = (tag, attrs = {}, parent) => { const e = document.createElementNS(NS, tag); for (const k in attrs) e.setAttribute(k, attrs[k]); parent && parent.appendChild(e); return e; };

// --- Karte aufbauen ---
const svg = $('map');
const defs = el('defs', {}, svg);
const clip = el('clipPath', { id: 'land' }, defs);
['DEU', 'NLD', 'BEL', 'LUX', 'FRA'].forEach(c => el('path', { d: COUNTRIES[c] }, clip));
const gBase = el('g', {}, svg), gReg = el('g', { 'clip-path': 'url(#land)' }, svg), gBorder = el('g', {}, svg),
  gDetail = el('g', {}, svg), gCodes = el('g', {}, svg), gTop = el('g', {}, svg);
for (const c in COUNTRIES) el('path', { d: COUNTRIES[c], class: 'country' + (['DEU', 'NLD', 'BEL', 'LUX'].includes(c) ? '' : ' out') }, gBase);
['DEU', 'NLD', 'BEL', 'LUX'].forEach(c => el('path', { d: COUNTRIES[c], class: 'border' }, gBorder));
const regEls = {}, codeEls = {};
for (const r of REGIONS) {
  const p = el('path', { d: r.d, class: 'reg', 'data-id': r.id }, gReg);
  p.addEventListener('click', () => openRegion(r));
  regEls[r.id] = p;
  const g = el('g', {}, gCodes);
  el('circle', { cx: r.cx, cy: r.cy, r: 11, class: 'codebg' }, g);
  codeEls[r.id] = el('text', { x: r.cx, y: r.cy, class: 'code' }, g);
}
const [hx, hy] = proj(HOME.lon, HOME.lat);
const homeEl = el('circle', { cx: hx, cy: hy, r: 7, class: 'home' }, gTop);

// --- Zeit-Slots ---
SLOTS.forEach((off, i) => {
  const b = document.createElement('button');
  b.className = 'slot'; b.setAttribute('role', 'tab');
  const h = NOW_H + off;
  b.innerHTML = `<b>${off ? '+' + off + ' h' : 'Jetzt'}</b><small>${fmtH(h)}${h + 1 > SUNSET ? ' · 🌅 17:03' : ''}</small>`;
  b.onclick = () => { state.slot = i; render(); };
  $('slots').appendChild(b);
});
$('legend').innerHTML = Object.entries(CLASSES).map(([k, c]) =>
  `<span class="lg"><span class="sw" style="background:${c.color}">${k}</span>${c.label}</span>`).join('');
$('backBtn').onclick = () => closeRegion();
$('startBtn').onclick = () => alert('Hier später: Startort per GPS oder Suche festlegen.');

// --- Rendern ---
function render() {
  const h = NOW_H + SLOTS[state.slot];
  [...$('slots').children].forEach((b, i) => b.setAttribute('aria-selected', i === state.slot));
  for (const r of REGIONS) {
    const c = classify(regionWx(r, h));
    regEls[r.id].style.fill = CLASSES[c].color;
    regEls[r.id].classList.toggle('dim', !!state.region && state.region.id !== r.id);
    codeEls[r.id].textContent = c;
    codeEls[r.id].parentNode.style.display = state.region ? 'none' : '';
  }
  state.region ? renderDetail(state.region, h) : renderList(h);
}

function sparkline(r) {
  return '<span class="spark">' + SLOTS.map(o => `<i style="background:${CLASSES[classify(regionWx(r, NOW_H + o))].color}"></i>`).join('') + '</span>';
}
function renderList(h) {
  const ranked = REGIONS.map(r => {
    // Score: Sonne ab gewähltem Zeitpunkt bis Sonnenuntergang, leicht abgewertet nach Fahrzeit
    let s = 0, n = 0; for (let t = h; t < SUNSET; t += 1) { s += regionWx(r, t).sun; n++; }
    const sunH = n ? s : 0; const d = drive(r);
    return { r, sunH, d, score: sunH - d.km / 400 };
  }).sort((a, b) => b.score - a.score).slice(0, 8);
  $('side').innerHTML = `<div class="grab"></div><h2>Beste Ziele ab ${fmtH(h)}</h2>
    <div class="hint">Sonnenstunden bis Sonnenuntergang · Fahrzeit ab ${HOME.name} (Womo ~75 km/h)</div>` +
    ranked.map((x, i) => `<div class="row" data-id="${x.r.id}"><span class="rank">${i + 1}</span>
      <span class="name"><b>${x.r.name}</b><small>${x.r.cc} · ${x.d.km} km · ${x.d.txt}</small></span>
      ${sparkline(x.r)}<span class="pill">☀︎ ${x.sunH.toFixed(1)} h</span></div>`).join('');
  $('side').querySelectorAll('.row').forEach(row => row.onclick = () => openRegion(REGIONS.find(r => r.id === row.dataset.id)));
}

function renderDetail(r, h) {
  // Feines Raster nur innerhalb der Region (später: ICON-D2 2-km-Gitter)
  gDetail.innerHTML = '';
  const cp = el('clipPath', { id: 'rc' }, gDetail); el('path', { d: r.d }, cp);
  const inner = el('g', { 'clip-path': 'url(#rc)' }, gDetail);
  const g2 = el('g', { 'clip-path': 'url(#land)' }, inner);
  const bb = regEls[r.id].getBBox(), step = 5;
  const fs = Math.max(bb.width, bb.height) / 22;
  for (let x = bb.x; x < bb.x + bb.width; x += step) for (let y = bb.y; y < bb.y + bb.height; y += step) {
    const [lon, lat] = unproj(x + step / 2, y + step / 2);
    el('rect', { x, y, width: step + .3, height: step + .3, class: 'cell', fill: CLASSES[classify(wx(lon, lat, h))].color }, g2);
  }
  el('path', { d: r.d, class: 'hl' }, gDetail);
  for (const [n, lon, lat] of TOWNS) {
    const [x, y] = proj(lon, lat);
    if (x > bb.x - 30 && x < bb.x + bb.width + 30 && y > bb.y - 30 && y < bb.y + bb.height + 30) {
      el('circle', { cx: x, cy: y, r: fs / 5, fill: '#1D2430' }, gDetail);
      const t = el('text', { x: x + fs / 3, y: y - fs / 4, class: 'town', 'font-size': fs }, gDetail); t.textContent = n;
    }
  }
  const w = regionWx(r, h), c = classify(w), d = drive(r);
  let sunH = 0; for (let t = 7; t < 17; t++) sunH += regionWx(r, t + .5).sun;
  const hours = [];
  for (let t = 7; t <= 19; t++) { const k = classify(regionWx(r, t)); hours.push(`<div class="hour ${t === h ? 'now' : ''}"><i style="background:${CLASSES[k].color}">${CLASSES[k].icon}</i>${t}</div>`); }
  const nav = `https://maps.apple.com/?daddr=${r.lat},${r.lon}&dirflg=d`;
  $('side').innerHTML = `<div class="grab"></div>
    <h2>${CLASSES[c].icon} ${r.name}</h2>
    <div class="hint">${CLASSES[c].label} um ${fmtH(h)} · ${d.km} km · ${d.txt} ab ${HOME.name}</div>
    <div class="stats">
      <div class="stat"><b>${Math.round(w.sun * 100)}%</b><small>Sonnenanteil</small></div>
      <div class="stat"><b>${sunH.toFixed(1)} h</b><small>Sonne heute</small></div>
      <div class="stat"><b>${Math.round(9 + w.sun * 5)}°</b><small>Temperatur</small></div>
      <div class="stat"><b>${Math.round(w.rain * 100)}%</b><small>Regenrisiko</small></div>
    </div>
    <h3>Stundenverlauf heute</h3><div class="hours">${hours.join('')}</div>
    <h3>Wochenende</h3>
    <div class="days"><div class="day"><b>Sa</b> ☀︎ ${sunH.toFixed(1)} h<br><small>Wind SW 3 Bft</small></div>
      <div class="day"><b>So</b> 🌤 ${(sunH * .7 + 1).toFixed(1)} h<br><small>Wind W 2 Bft</small></div></div>
    <div class="actions"><a class="btn primary" style="text-align:center;text-decoration:none" href="${nav}" target="_blank">🚐 Route starten</a>
      <button class="btn" onclick="alert('Später: Favoriten & Push bei Sonne')">☆ Merken</button></div>`;
}

let anim;
function zoomTo(vb) {
  const from = svg.getAttribute('viewBox').split(' ').map(Number), t0 = performance.now();
  cancelAnimationFrame(anim);
  const step = now => {
    const k = Math.min(1, (now - t0) / 450), e = 1 - Math.pow(1 - k, 3);
    svg.setAttribute('viewBox', from.map((v, i) => v + (vb[i] - v) * e).join(' '));
    if (k < 1) anim = requestAnimationFrame(step);
  };
  anim = requestAnimationFrame(step);
}
function openRegion(r) {
  homeEl.setAttribute('r', 3); state.region = r; $('backBtn').classList.remove('hidden');
  const bb = regEls[r.id].getBBox(), pad = 40, s = Math.max(bb.width, bb.height / 1.1) + pad * 2;
  zoomTo([bb.x + bb.width / 2 - s / 2, bb.y + bb.height / 2 - s * 0.55, s, s * 1.1]);
  render();
}
function closeRegion() {
  homeEl.setAttribute('r', 7); state.region = null; gDetail.innerHTML = ''; $('backBtn').classList.add('hidden');
  zoomTo([0, 0, 1000, 1100]); render();
}
render();
