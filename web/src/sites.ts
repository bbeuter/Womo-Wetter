// Übernachtungsplätze (Wohnmobilstellplätze + Campingplätze) einer Region.
import { CLASSES, type Cls, type Grid, type Stellplatz } from './data';
import { drive } from './geo';
import { esc, safeUrl } from './dom';
import { saveSettings, settings, startPlace } from './store';
import type { Ctx } from './state';

export type SiteKind = 'caravan_site' | 'camp_site';
export const kindOf = (s: Stellplatz): SiteKind => (s.type === 'camp_site' ? 'camp_site' : 'caravan_site');
export const KIND: Record<SiteKind, { icon: string; label: string; plural: string }> = {
  caravan_site: { icon: '🚐', label: 'Wohnmobilstellplatz', plural: 'Stellplätze' },
  camp_site: { icon: '⛺', label: 'Campingplatz', plural: 'Camping' },
};
export const siteVisible = (s: Stellplatz): boolean => settings.siteFilter === 'all' || kindOf(s) === settings.siteFilter;

export function routeUrl(lat: number, lon: number): string {
  return settings.nav === 'apple'
    ? `https://maps.apple.com/?daddr=${lat},${lon}&dirflg=d`
    : `https://www.google.com/maps/dir/?api=1&destination=${lat},${lon}&travelmode=driving`;
}

/** Wetter am Platz zur Stunde idx aus grid.json (nächste Gitterzelle). */
export function wxAt(grid: Grid | null, idx: number, lon: number, lat: number): { cls: Cls; sun: number } | null {
  const row = grid?.sun[idx];
  if (!grid || !row) return null;
  const i = Math.round((lon - grid.lon0) / grid.step), j = Math.round((lat - grid.lat0) / grid.step);
  if (i < 0 || j < 0 || i >= grid.nx || j >= grid.ny) return null;
  const ch = row[j * grid.nx + i];
  if (!ch || ch === '.') return null;
  if (ch === 'R') return { cls: 'R', sun: 0 };
  if (ch === 'N') return { cls: 'N', sun: -1 };
  const sun = +ch * 10;
  return { cls: sun >= 75 ? 'S' : sun >= 50 ? 'H' : sun >= 25 ? 'W' : 'B', sun };
}

export const AMEN: Record<string, { icon: string; label: string }> = {
  P: { icon: '🔌', label: 'Strom' }, W: { icon: '🚰', label: 'Frischwasser' }, T: { icon: '🚻', label: 'WC' },
  S: { icon: '🚿', label: 'Dusche' }, D: { icon: '♻️', label: 'Entsorgung' }, I: { icon: '📶', label: 'WLAN' },
};
const GENERIC = new Set(['Stellplatz', 'Campingplatz', 'Wohnmobilstellplatz']);
/** Google Maps: Suche nach dem Namen an der Position – zeigt Sterne und Rezensionen. */
export function reviewUrl(s: Stellplatz): string {
  const q = s.name && !GENERIC.has(s.name) ? s.name : KIND[kindOf(s)].label;
  return `https://www.google.com/maps/search/${encodeURIComponent(q)}/@${s.lat},${s.lon},16z`;
}
export const p4nUrl = (s: Stellplatz): string => `https://park4night.com/de/search?lat=${s.lat}&lng=${s.lon}&z=15`;

const feeText = (f?: string): string =>
  !f ? '' : f === 'yes' ? 'kostenpflichtig' : f === 'no' ? 'kostenlos' : f === 'donation' ? 'Spende' : esc(f.slice(0, 30));

/** HTML-Abschnitt „Übernachten“ für die Detailansicht. */
export function sitesSection(ctx: Ctx, id: string): string {
  const { sites, grid } = ctx.st.data;
  if (!sites) return '';
  const from = startPlace();
  const all = sites.filter(s => s.region === id);
  const n = { caravan_site: 0, camp_site: 0 };
  all.forEach(s => n[kindOf(s)]++);
  const list = all.filter(siteVisible).map(s => {
    const w = wxAt(grid, ctx.st.idx, s.lon, s.lat);
    return { s, w, d: drive(from, s, settings.speed) };
  }).sort((a, b) => (b.w?.sun ?? -1) - (a.w?.sun ?? -1) || a.d.km - b.d.km);

  const chip = (v: typeof settings.siteFilter, label: string, count: number) =>
    `<button type="button" class="fchip" data-f="${v}" aria-pressed="${settings.siteFilter === v}">${label} <small>${count}</small></button>`;
  const item = ({ s, w, d }: typeof list[number]) => {
    const k = KIND[kindOf(s)];
    const name = esc(s.name ?? k.label);
    const web = safeUrl(s.website);
    const fee = feeText(s.fee);
    const am = [...(s.am ?? '')].filter(c => AMEN[c]);
    const sun = w ? `<span class="ssun c${w.cls}" style="background:${CLASSES[w.cls].color}" title="${CLASSES[w.cls].label}">${w.cls === 'N' || w.cls === 'R' ? CLASSES[w.cls].icon : `☀︎ ${w.sun}%`}</span>` : '';
    const facts = [
      s.stars ? `${'★'.repeat(s.stars)} Kategorie` : '',
      s.cap ? `${s.cap} Plätze` : '',
      fee ? fee[0].toUpperCase() + fee.slice(1) : '',
    ].filter(Boolean);
    return `<li class="site" data-site="${esc(s.id)}">
      <div class="srow" role="button" tabindex="0" aria-expanded="false">
        <span class="sicon" aria-hidden="true">${k.icon}</span>
        <span class="name"><b>${name}</b><small>${d.km} km · ${d.txt}${s.stars ? ' · ' + '★'.repeat(s.stars) : ''}${am.length ? ` · <span class="amen" aria-label="${am.map(c => AMEN[c].label).join(', ')}">${am.map(c => AMEN[c].icon).join('')}</span>` : ''}</small></span>
        ${sun}
        <a class="lnk go" href="${esc(routeUrl(s.lat, s.lon))}" target="_blank" rel="noopener" aria-label="Route zu ${name}">Route</a>
      </div>
      <div class="sx" hidden>
        <p>${k.label}${facts.length ? ' · ' + facts.join(' · ') : ''}</p>
        ${am.length ? `<p class="amenl">${am.map(c => `<span>${AMEN[c].icon} ${AMEN[c].label}</span>`).join('')}</p>` : '<p class="hint">Keine Ausstattung in OpenStreetMap eingetragen.</p>'}
        <div class="sxb">
          <a class="lnk" href="${esc(reviewUrl(s))}" target="_blank" rel="noopener">★ Bewertungen (Google)</a>
          <a class="lnk" href="${esc(p4nUrl(s))}" target="_blank" rel="noopener">park4night</a>
          ${web ? `<a class="lnk" href="${esc(web)}" target="_blank" rel="noopener">Webseite</a>` : ''}
        </div>
      </div></li>`;
  };
  const TOP = 10;
  return `<section class="sitesec" id="sites">
    <h3>Übernachten in der Region</h3>
    <div class="fchips" role="group" aria-label="Platzart">${chip('all', 'Alle', all.length)}${chip('caravan_site', '🚐 Stellplätze', n.caravan_site)}${chip('camp_site', '⛺ Camping', n.camp_site)}</div>
    <p class="hint">Sortiert nach Sonne am Platz um die gewählte Uhrzeit, dann Entfernung. Antippen zeigt Ausstattung und Bewertungen.</p>
    ${list.length
      ? `<ul class="sites">${list.slice(0, TOP).map(item).join('')}</ul>` +
        (list.length > TOP ? `<details class="more"><summary>Alle ${list.length} anzeigen</summary><ul class="sites">${list.slice(TOP).map(item).join('')}</ul></details>` : '')
      : '<p class="hint">Keine passenden Plätze in dieser Region bekannt.</p>'}
    <p class="attr">Plätze: © <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap-Mitwirkende</a> (ODbL) · Angaben ohne Gewähr</p>
  </section>`;
}

export function bindSites(ctx: Ctx, el: HTMLElement): void {
  el.querySelectorAll<HTMLButtonElement>('.fchip').forEach(b => b.addEventListener('click', () => {
    settings.siteFilter = b.dataset.f as typeof settings.siteFilter;
    saveSettings();
    ctx.renderAll();
    document.getElementById('sites')?.scrollIntoView({ block: 'start' });
  }));
  el.querySelectorAll<HTMLElement>('.srow').forEach(row => {
    const act = (e: Event) => {
      if ((e.target as HTMLElement).closest('a')) return;
      const id = row.parentElement!.dataset.site!;
      const wasOpen = row.getAttribute('aria-expanded') === 'true';
      ctx.st.map.highlightSite(id);
      markSite(id, false);
      if (wasOpen) toggle(row.parentElement!, false);
    };
    row.addEventListener('click', act);
    row.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); act(e); } });
  });
}

function toggle(li: Element, open: boolean): void {
  li.querySelector('.srow')?.setAttribute('aria-expanded', String(open));
  const sx = li.querySelector<HTMLElement>('.sx');
  if (sx) sx.hidden = !open;
}

/** Platz in der Liste hervorheben und aufklappen (bei Bedarf hinscrollen). */
export function markSite(id: string, scroll: boolean): void {
  document.querySelectorAll('.site.sel').forEach(x => { x.classList.remove('sel'); toggle(x, false); });
  const li = document.querySelector<HTMLLIElement>(`.site[data-site="${CSS.escape(id)}"]`);
  if (!li) return;
  const more = li.closest('details');
  if (more) more.open = true;
  li.classList.add('sel');
  toggle(li, true);
  if (scroll) li.scrollIntoView({ block: 'center', behavior: 'smooth' });
}
