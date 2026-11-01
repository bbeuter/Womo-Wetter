// SVG-Karte: Länderumrisse, Regionen (Klassenfarbe + Buchstabe), Startort, Detail-Zoom mit Heatmap.
import countriesRaw from './assets/countries.json';
import { CLASSES, asCls, type AppData, type Cls, type Grid, type LonLat, type Polygon, type Region } from './data';
import { bboxOf, makeProjection, pathOf, type BBox, type Projection } from './geo';
import { svgEl, reducedMotion } from './dom';
import { TOWNS, LABEL_TOWNS } from './towns';
import { startPlace, favs } from './store';

const countries = countriesRaw as unknown as Record<string, LonLat[][][]>;
const CORE = ['DEU', 'NLD', 'BEL', 'LUX'];

interface RegEl { r: Region; path: SVGPathElement; badge: SVGGElement; text: SVGTextElement; d: string; bb: BBox }

export class SunMap {
  readonly P: Projection;
  private svg: SVGSVGElement;
  private regs = new Map<string, RegEl>();
  private gDetail: SVGGElement;
  private gCodes: SVGGElement;
  private home: SVGGElement;
  private vb: number[];
  private anim = 0;
  private selected: string | null = null;

  constructor(svg: SVGSVGElement, private data: AppData, private onSelect: (id: string) => void) {
    this.svg = svg;
    this.P = makeProjection(data.regions);
    const { W, H } = this.P;
    this.vb = [0, 0, W, H];
    svg.setAttribute('viewBox', this.vb.join(' '));
    svg.innerHTML = '';

    const defs = svgEl('defs', {}, svg);
    const blur = svgEl('filter', { id: 'soft', x: '-10%', y: '-10%', width: '120%', height: '120%' }, defs);
    svgEl('feGaussianBlur', { stdDeviation: 3, id: 'softDev' }, blur);

    const gBase = svgEl('g', { class: 'base', 'aria-hidden': 'true' }, svg);
    const gReg = svgEl('g', { class: 'regions' }, svg);
    const gBorder = svgEl('g', { 'aria-hidden': 'true' }, svg);
    this.gDetail = svgEl('g', { class: 'detail', 'aria-hidden': 'true' }, svg);
    this.gCodes = svgEl('g', { 'aria-hidden': 'true' }, svg);
    const gTop = svgEl('g', { 'aria-hidden': 'true' }, svg);

    for (const c in countries) {
      const d = pathOf(countries[c] as Polygon[], this.P);
      svgEl('path', { d, class: 'country' + (CORE.includes(c) || c === 'FRA' ? '' : ' out') }, gBase);
      if (CORE.includes(c)) svgEl('path', { d, class: 'border' }, gBorder);
    }
    for (const r of data.regions) {
      const d = pathOf(r.polys, this.P);
      const path = svgEl('path', { d, class: 'reg', tabindex: 0, role: 'button', 'data-id': r.id }, gReg);
      path.addEventListener('click', () => this.onSelect(r.id));
      path.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); this.onSelect(r.id); } });
      const badge = svgEl('g', { class: 'badge' }, this.gCodes);
      svgEl('circle', { r: 10.5, class: 'codebg' }, badge);
      const text = svgEl('text', { class: 'code', y: 0.5 }, badge);
      this.regs.set(r.id, { r, path, badge, text, d, bb: bboxOf(r.polys, this.P) });
    }
    this.home = svgEl('g', { class: 'home' }, gTop);
    // Stecknadel: Spitze zeigt auf den Startort
    svgEl('path', { d: 'M0 0C-2-5-8-9-8-15a8 8 0 0 1 16 0c0 6-6 10-8 15z', class: 'pin' }, this.home);
    svgEl('circle', { cy: -15, r: 3, class: 'pinhole' }, this.home);
    this.updateHome();
    this.placeBadges();
  }

  updateHome(): void {
    const s = startPlace();
    const [x, y] = this.P.proj(s.lon, s.lat);
    this.home.setAttribute('transform', `translate(${x.toFixed(1)} ${y.toFixed(1)}) scale(${this.unitPx()})`);
    this.home.classList.toggle('gps', s.gps);
  }

  /** Füllt Regionen nach Klasse der Stunde idx. */
  render(idx: number): void {
    for (const [id, e] of this.regs) {
      const c: Cls = asCls(this.data.latest.regions[id].hourly.cls[idx]);
      e.path.style.fill = CLASSES[c].color;
      e.text.textContent = c;
      e.path.setAttribute('aria-label', `${e.r.name}: ${CLASSES[c].label}${favs.has(id) ? ', gemerkt' : ''}`);
      e.path.classList.toggle('dim', !!this.selected && this.selected !== id);
      e.path.classList.toggle('sel', this.selected === id);
      e.badge.classList.toggle('fav', favs.has(id));
    }
    this.gCodes.style.display = this.selected ? 'none' : '';
    if (this.selected) this.renderDetail(this.selected, idx);
  }

  /** SVG-Einheiten je CSS-Pixel für eine viewBox (preserveAspectRatio meet). */
  private unitPx(vb = this.vb): number {
    const r = this.svg.getBoundingClientRect();
    if (!r.width || !r.height) return vb[2] / 400;
    return Math.max(vb[2] / r.width, vb[3] / r.height);
  }

  /** Buchstaben-Badges bildschirmkonstant skalieren (kleiner bei kleinen Regionen). */
  private placeBadges(): void {
    const u = this.unitPx([0, 0, this.P.W, this.P.H]);
    for (const e of this.regs.values()) {
      const minPx = Math.min(e.bb.w, e.bb.h) / u;
      const k = Math.max(0.62, Math.min(0.9, minPx / 34));
      const [cx, cy] = this.P.proj(e.r.center[0], e.r.center[1]);
      e.badge.setAttribute('transform', `translate(${cx.toFixed(1)} ${cy.toFixed(1)}) scale(${(u * k).toFixed(3)})`);
    }
  }

  private fitBox(bb: BBox): number[] {
    const r = this.svg.getBoundingClientRect();
    const a = r.width && r.height ? r.width / r.height : 1;
    const pad = Math.max(bb.w, bb.h) * 0.18 + 12;
    let w = bb.w + 2 * pad, h = bb.h + 2 * pad;
    if (w / h < a) w = h * a; else h = w / a;
    return [bb.x + bb.w / 2 - w / 2, bb.y + bb.h / 2 - h / 2, w, h];
  }

  private zoomTo(target: number[]): void {
    cancelAnimationFrame(this.anim);
    const from = this.vb.slice();
    const apply = (v: number[]) => { this.vb = v; this.svg.setAttribute('viewBox', v.map(n => n.toFixed(2)).join(' ')); };
    if (reducedMotion()) { apply(target); return; }
    const t0 = performance.now();
    const step = (now: number) => {
      const k = Math.min(1, (now - t0) / 450), e = 1 - Math.pow(1 - k, 3);
      apply(from.map((v, i) => v + (target[i] - v) * e));
      if (k < 1) this.anim = requestAnimationFrame(step);
    };
    this.anim = requestAnimationFrame(step);
  }

  select(id: string | null, idx: number): void {
    this.selected = id;
    this.gDetail.innerHTML = '';
    const target = id ? this.fitBox(this.regs.get(id)!.bb) : [0, 0, this.P.W, this.P.H];
    this.render(idx);
    if (id) this.renderDetail(id, idx, target);
    this.home.setAttribute('transform', this.home.getAttribute('transform')!.replace(/scale\([^)]*\)/, `scale(${this.unitPx(target)})`));
    this.zoomTo(target);
  }

  /** Nach Größenänderung den Ausschnitt neu einpassen. */
  refit(idx: number): void {
    const target = this.selected ? this.fitBox(this.regs.get(this.selected)!.bb) : [0, 0, this.P.W, this.P.H];
    cancelAnimationFrame(this.anim);
    this.vb = target;
    this.svg.setAttribute('viewBox', target.join(' '));
    this.updateHome();
    this.placeBadges();
    if (this.selected) this.renderDetail(this.selected, idx, target);
  }

  // ---------- Detail: Heatmap + Orte + Stellplätze ----------
  private renderDetail(id: string, idx: number, vb = this.vb): void {
    const e = this.regs.get(id)!;
    const g = this.gDetail;
    g.innerHTML = '';
    const px = this.unitPx(vb); // SVG-Einheiten je CSS-Pixel (für Schrift/Punkte)
    const clip = svgEl('clipPath', { id: 'rclip' }, g);
    svgEl('path', { d: e.d }, clip);
    const inner = svgEl('g', { 'clip-path': 'url(#rclip)' }, g);
    const cls = asCls(this.data.latest.regions[id].hourly.cls[idx]);
    svgEl('path', { d: e.d, style: `fill:${CLASSES[cls].color}` }, inner);
    if (this.data.grid) this.heat(inner, this.data.grid, idx, e.bb);
    svgEl('path', { d: e.d, class: 'hl' }, g);

    // Stellplätze dieser Region
    for (const s of this.data.sites ?? []) {
      if (s.region !== id) continue;
      const [x, y] = this.P.proj(s.lon, s.lat);
      const c = svgEl('circle', { cx: x, cy: y, r: 4 * px, class: 'site' }, g);
      svgEl('title', {}, c).textContent = s.name ?? 'Wohnmobil-Stellplatz';
    }
    // Orte (aus eingebauter Liste) mit einfacher Kollisionsvermeidung
    const fs = 13 * px;
    const [vx, vy, vw, vh] = vb;
    // freizuhaltende Bereiche: Zurück-Knopf oben links, Legende unten, Startort
    const [hx, hy] = this.P.proj(startPlace().lon, startPlace().lat);
    const placed: BBox[] = [{ x: vx, y: vy, w: 150 * px, h: 56 * px }, { x: hx - 10 * px, y: hy - 26 * px, w: 20 * px, h: 28 * px }];
    for (const t of [...TOWNS, ...LABEL_TOWNS]) {
      const [x, y] = this.P.proj(t.lon, t.lat);
      const box = { x: x - fs * 0.3, y: y - fs * 1.2, w: fs * (0.6 * t.name.length + 1), h: fs * 1.5 };
      if (box.x < vx + 4 * px || box.y < vy + 4 * px || box.x + box.w > vx + vw - 4 * px || y > vy + vh - 34 * px) continue;
      if (placed.some(b => b.x < box.x + box.w && box.x < b.x + b.w && b.y < box.y + box.h && box.y < b.y + b.h)) continue;
      placed.push(box);
      svgEl('circle', { cx: x, cy: y, r: 2.6 * px, class: 'towndot' }, g);
      const tx = svgEl('text', { x: x + fs * 0.4, y: y - fs * 0.3, class: 'town', 'font-size': fs.toFixed(2) }, g);
      tx.textContent = t.name;
    }
  }

  private heat(parent: SVGGElement, grid: Grid, idx: number, bb: BBox): void {
    const row = grid.sun[idx];
    if (!row) return;
    const css = getComputedStyle(document.documentElement);
    const col = (v: string) => hex(css.getPropertyValue(v).trim());
    const ramp: [number, number[]][] = [[0, col('--B')], [3, col('--W')], [5.5, col('--H')], [8, col('--S')]];
    const R = rgb(col('--R')), N = rgb(col('--N'));
    const [lonA, latB] = this.P.unproj(bb.x, bb.y + bb.h);
    const [lonB, latA] = this.P.unproj(bb.x + bb.w, bb.y);
    const st = grid.step, m = 2;
    const i0 = Math.max(0, Math.floor((lonA - grid.lon0) / st) - m), i1 = Math.min(grid.nx - 1, Math.ceil((lonB - grid.lon0) / st) + m);
    const j0 = Math.max(0, Math.floor((latB - grid.lat0) / st) - m), j1 = Math.min(grid.ny - 1, Math.ceil((latA - grid.lat0) / st) + m);
    const cells = svgEl('g', { filter: 'url(#soft)' }, parent);
    let cellPx = 10;
    for (let j = j0; j <= j1; j++) for (let i = i0; i <= i1; i++) {
      const ch = row[j * grid.nx + i];
      if (!ch || ch === '.') continue;
      const lon = grid.lon0 + i * st, lat = grid.lat0 + j * st;
      const [x0, y0] = this.P.proj(lon - st / 2, lat + st / 2);
      const [x1, y1] = this.P.proj(lon + st / 2, lat - st / 2);
      cellPx = x1 - x0;
      const fill = ch === 'R' ? R : ch === 'N' ? N : rgb(lerpRamp(ramp, +ch));
      svgEl('rect', { x: x0.toFixed(2), y: y0.toFixed(2), width: (x1 - x0 + 0.4).toFixed(2), height: (y1 - y0 + 0.4).toFixed(2), fill }, cells);
    }
    document.getElementById('softDev')?.setAttribute('stdDeviation', (cellPx * 0.45).toFixed(2));
  }
}

function hex(s: string): number[] {
  const m = /^#?([0-9a-f]{6})$/i.exec(s);
  if (!m) return [200, 200, 200];
  const n = parseInt(m[1], 16);
  return [n >> 16, (n >> 8) & 255, n & 255];
}
const rgb = (c: number[]): string => `rgb(${c.map(v => Math.round(v)).join(',')})`;
function lerpRamp(ramp: [number, number[]][], v: number): number[] {
  if (v <= ramp[0][0]) return ramp[0][1];
  for (let k = 1; k < ramp.length; k++) {
    const [a, ca] = ramp[k - 1], [b, cb] = ramp[k];
    if (v <= b) { const t = (v - a) / (b - a); return ca.map((x, i) => x + (cb[i] - x) * t); }
  }
  return ramp[ramp.length - 1][1];
}
