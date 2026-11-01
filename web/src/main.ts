import './style.css';
import { CLASSES, type AppData, berlinNow, currentHourIndex, dateOf, hhmm, loadData, parseIso, wdShort, type Cls } from './data';
import { $, esc } from './dom';
import { SunMap } from './map';
import { atOrAfterSunset, medianSunset, SLOT_OFFSETS } from './model';
import { renderList } from './list';
import { renderDetail } from './detail';
import { openSettings, startLabel } from './settings';
import { saveSettings, settings } from './store';
import type { Ctx, State } from './state';

const STALE_H = 6;

async function boot(): Promise<void> {
  let data: AppData;
  try {
    data = await loadData();
  } catch (err) {
    console.error(err);
    $('status').textContent = 'Keine Vorhersage verfügbar';
    $('sideBody').innerHTML = `<h2>Daten konnten nicht geladen werden</h2>
      <p class="hint">Bitte Internetverbindung prüfen und neu laden. (${esc(String((err as Error).message ?? err))})</p>
      <button class="btn primary" type="button" onclick="location.reload()">Neu laden</button>`;
    return;
  }
  // ?now=2026-11-07T14 (nur zum Testen): aktuelle Berliner Zeit überschreiben
  const fake = /^(\d{4}-\d{2}-\d{2})T(\d{2})$/.exec(new URLSearchParams(location.search).get('now') ?? '');
  const now = fake ? { date: fake[1], hour: +fake[2], minute: 0, weekday: new Date(fake[1] + 'T12:00Z').getUTCDay() } : berlinNow();
  const nowIdx = currentHourIndex(data.latest.hours, now);
  const svg = document.getElementById('map') as unknown as SVGSVGElement;
  const st: State = {
    data, now, nowIdx, idx: nowIdx, region: null, listMode: 'heute',
    map: null as unknown as SunMap,
  };
  const ctx: Ctx = {
    st,
    setIdx(i) { st.idx = Math.max(0, Math.min(data.latest.hours.length - 1, i)); ctx.renderAll(); },
    open(id) {
      const target = id ? `#${encodeURIComponent(id)}` : '';
      if (location.hash !== target) {
        if (id) location.hash = target;
        else history.replaceState(null, '', location.pathname + location.search);
      }
      applyRoute();
    },
    renderAll,
  };
  st.map = new SunMap(svg, data, id => ctx.open(id));

  function applyRoute(): void {
    const id = decodeURIComponent(location.hash.slice(1));
    const next = id && data.byId.has(id) ? id : null;
    if (next === st.region) return;
    st.region = next;
    $('backBtn').classList.toggle('hidden', !next);
    document.body.classList.toggle('detail', !!next);
    st.map.select(next, st.idx);
    renderSide();
    $('side').scrollTop = 0;
    if (next) $('backBtn').focus({ preventScroll: true });
  }

  function renderSlots(): void {
    const { hours } = data.latest;
    const nav = $('slots');
    nav.innerHTML = '';
    let prev = -1;
    SLOT_OFFSETS.forEach(off => {
      const i = Math.min(nowIdx + off, hours.length - 1);
      const dup = i === prev; prev = i;
      const h = hours[i];
      const ss = medianSunset(data.latest, dateOf(h));
      const night = atOrAfterSunset(h, ss);
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'slot';
      b.setAttribute('role', 'tab');
      b.setAttribute('aria-selected', String(i === st.idx && !dup));
      b.disabled = dup; // Daten reichen nicht so weit
      b.setAttribute('aria-label', `${off ? `In ${off} Stunden` : 'Jetzt'}, ${hhmm(h)} Uhr${night && ss ? `, Sonnenuntergang ${ss}` : ''}`);
      b.innerHTML = `<b>${off ? `+${off} h` : 'Jetzt'}</b><small>${hhmm(h)}${night && ss ? `<span class="ss"><i> · </i>🌅 ${ss}</span>` : ''}</small>`;
      b.addEventListener('click', () => ctx.setIdx(i));
      nav.appendChild(b);
    });
  }

  function renderStatus(): void {
    const g = data.latest.generated;
    const ms = parseIso(g);
    const ageH = (Date.now() - ms) / 3.6e6;
    const day = dateOf(g) === now.date ? '' : `${wdShort(dateOf(g))} `;
    const parts = [`Stand: ${day}${hhmm(g)}`];
    if (data.latest.mock) parts.push('<span class="tag mock">Beispieldaten</span>');
    if (ageH > STALE_H) parts.push(`<span class="tag warn" title="Die Vorhersage ist ${Math.round(ageH)} h alt">⚠︎ ${Math.round(ageH)} h alt</span>`);
    if (!navigator.onLine) parts.push('<span class="tag">offline</span>');
    $('status').innerHTML = parts.join(' ');
  }

  function renderSide(): void {
    const body = $('sideBody');
    if (st.region) renderDetail(ctx, body, st.region); else renderList(ctx, body);
  }

  function renderAll(): void {
    renderSlots();
    st.map.render(st.idx);
    renderSide();
    $('startBtn').textContent = startLabel();
  }

  // Legende
  $('legend').innerHTML = (Object.keys(CLASSES) as Cls[]).map(k =>
    `<span class="lg"><span class="sw c${k}" style="background:${CLASSES[k].color}">${k}</span>${CLASSES[k].label}</span>`).join('');

  $('backBtn').addEventListener('click', () => ctx.open(null));
  $('startBtn').addEventListener('click', () => openSettings($('settings') as HTMLDialogElement, () => { st.map.updateHome(); renderAll(); }));
  $('grab').addEventListener('click', () => document.body.classList.toggle('expanded'));
  window.addEventListener('hashchange', applyRoute);
  window.addEventListener('keydown', e => { if (e.key === 'Escape' && st.region && !($('settings') as HTMLDialogElement).open) ctx.open(null); });
  window.addEventListener('online', renderStatus);
  window.addEventListener('offline', renderStatus);
  let rz = 0;
  new ResizeObserver(() => { cancelAnimationFrame(rz); rz = requestAnimationFrame(() => st.map.refit(st.idx)); }).observe(svg);
  // Beim Zurückkehren in die App (iOS hält PWAs im Speicher): bei neuer Stunde neu laden
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible' && berlinNow().hour !== now.hour) location.reload();
  });

  renderStatus();
  renderAll();
  applyRoute();

  // GPS-Modus: Position still im Hintergrund aktualisieren
  if (settings.mode === 'gps' && 'geolocation' in navigator) {
    navigator.geolocation.getCurrentPosition(p => {
      settings.lastGps = { name: 'GPS-Standort', lat: +p.coords.latitude.toFixed(4), lon: +p.coords.longitude.toFixed(4), at: Date.now() };
      saveSettings(); st.map.updateHome(); renderAll();
    }, () => { /* Fallback: letzte Position bzw. Heimatort */ }, { timeout: 10000, maximumAge: 15 * 60000 });
  }
}

if ('serviceWorker' in navigator && import.meta.env.PROD) {
  window.addEventListener('load', () => { navigator.serviceWorker.register('./sw.js').catch(e => console.warn('SW', e)); });
}
boot();
