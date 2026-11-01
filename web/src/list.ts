// Liste „Beste Ziele“ (Heute | Wochenende).
import { CLASSES, asCls, ddmm, hhmm, wdShort } from './data';
import { esc, fmt1 } from './dom';
import { rank, SLOT_OFFSETS, weekendDates, DRIVE_PENALTY, type Ranked } from './model';
import { settings, startPlace } from './store';
import type { Ctx } from './state';

const TOP = 10;

export function renderList(ctx: Ctx, el: HTMLElement): void {
  const { st } = ctx;
  const { latest } = st.data;
  const we = st.listMode === 'we';
  const weDates = weekendDates(latest.days, st.now);
  const all = rank(st.data, st.idx, st.listMode, weDates);
  const favRows = all.filter(x => x.fav);
  const rest = all.filter(x => !x.fav).slice(0, TOP);
  const from = startPlace();
  const t = hhmm(latest.hours[st.idx]);
  const anyTomorrow = !we && all.slice(0, TOP).some(x => x.tomorrow);

  const title = we
    ? `Beste Ziele am ${weDates.length === 2 ? 'Wochenende' : 'Sonntag'}`
    : `Beste Ziele ab ${t}`;
  const hint = we
    ? `Sonnenstunden ${weDates.map(d => `${wdShort(d)} ${ddmm(d)}`).join(' + ')}`
    : anyTomorrow ? 'Nach Sonnenuntergang: Sonnenstunden morgen' : 'Sonnenstunden bis Sonnenuntergang';

  el.innerHTML = `
    <div class="listhead">
      <h2>${title}</h2>
      <div class="seg" role="tablist" aria-label="Zeitraum">
        <button type="button" role="tab" data-mode="heute" aria-selected="${!we}">Heute</button>
        <button type="button" role="tab" data-mode="we" aria-selected="${we}">Wochenende</button>
      </div>
    </div>
    <div class="hint">${hint} · Fahrzeit ab ${esc(from.name)} (Womo ~${settings.speed} km/h, −${fmt1(DRIVE_PENALTY)} h Sonne je Fahrstunde)</div>
    ${favRows.length ? `<h3>★ Gemerkt</h3><ol class="rows">${favRows.map(x => row(ctx, x, all.indexOf(x) + 1, we)).join('')}</ol><h3>Vorschläge</h3>` : ''}
    <ol class="rows">${rest.map(x => row(ctx, x, all.indexOf(x) + 1, we)).join('')}</ol>`;

  el.querySelectorAll<HTMLButtonElement>('.seg button').forEach(b => b.addEventListener('click', () => {
    st.listMode = b.dataset.mode === 'we' ? 'we' : 'heute';
    ctx.renderAll();
  }));
  el.querySelectorAll<HTMLButtonElement>('button.row').forEach(b => b.addEventListener('click', () => ctx.open(b.dataset.id!)));
}

function row(ctx: Ctx, x: Ranked, rankNo: number, we: boolean): string {
  const { st } = ctx;
  const rw = st.data.latest.regions[x.r.id];
  const spark = we
    ? (x.wk ?? []).map(d => { const c = asCls(d.cls); return `<i class="we c${c}" style="background:${CLASSES[c].color}" title="${wdShort(d.date)}: ${CLASSES[c].label}"><span>${wdShort(d.date)}</span></i>`; }).join('')
    : SLOT_OFFSETS.map(o => {
      const i = Math.min(st.nowIdx + o, st.data.latest.hours.length - 1);
      const c = asCls(rw.hourly.cls[i]);
      return `<i class="${i === st.idx ? 'cur' : ''}" style="background:${CLASSES[c].color}" title="${hhmm(st.data.latest.hours[i])}: ${CLASSES[c].label}"></i>`;
    }).join('');
  const label = `${x.r.name}, ${x.d.km} km, ${x.d.txt} Fahrzeit, ${fmt1(x.sunH)} Sonnenstunden${x.tomorrow ? ' morgen' : ''}${x.fav ? ', gemerkt' : ''}`;
  return `<li><button type="button" class="row" data-id="${esc(x.r.id)}" aria-label="${esc(label)}">
    <span class="rank">${x.fav ? '★' : rankNo}</span>
    <span class="name"><b>${esc(x.r.name)}</b><small>${x.r.cc} · ${x.d.km} km · ${x.d.txt}</small></span>
    <span class="spark" aria-hidden="true">${spark}</span>
    <span class="pill">☀︎ ${fmt1(x.sunH)} h${x.tomorrow ? '<small> morgen</small>' : ''}</span>
  </button></li>`;
}
