// Detail-Panel einer Region.
import { CLASSES, asCls, beaufort, compass, dateOf, ddmm, hhmm, hourOf, wdShort } from './data';
import { drive } from './geo';
import { bindSites, sitesSection } from './sites';
import { esc, fmt1 } from './dom';
import { dailyFor, weekendDates } from './model';
import { favs, settings, startPlace, toggleFav } from './store';
import type { Ctx } from './state';

const windArrow = (deg: number): string =>
  `<span class="warrow" style="transform:rotate(${(deg + 180) % 360}deg)" aria-hidden="true">↑</span>`;

export function renderDetail(ctx: Ctx, el: HTMLElement, id: string): void {
  const { st } = ctx;
  const { latest, byId } = st.data;
  const r = byId.get(id)!;
  const rw = latest.regions[id];
  const H = rw.hourly, i = st.idx;
  const c = asCls(H.cls[i]);
  const from = startPlace();
  const d = drive(from, { lon: r.center[0], lat: r.center[1] }, settings.speed);
  const today = dateOf(latest.hours[st.nowIdx]);
  const dToday = dailyFor(rw, today);
  const selDate = dateOf(latest.hours[i]);
  const fav = favs.has(id);

  // Stundenstreifen 07–20 des Tages der gewählten Stunde
  const hours: string[] = [];
  latest.hours.forEach((h, k) => {
    if (dateOf(h) !== selDate) return;
    const hr = hourOf(h);
    if (hr < 7 || hr > 20) return;
    const kc = asCls(H.cls[k]);
    hours.push(`<button type="button" class="hour${k === i ? ' now' : ''}${k < st.nowIdx ? ' past' : ''}" data-idx="${k}"
      aria-label="${hr} Uhr: ${CLASSES[kc].label}, Sonne ${H.sun[k]} %" aria-pressed="${k === i}">
      <i style="background:${CLASSES[kc].color}" class="c${kc}">${CLASSES[kc].icon}</i>${hr}</button>`);
  });

  const weDates = new Set(weekendDates(latest.days, st.now));
  const days = rw.daily.map(x => {
    const dc = asCls(x.cls);
    return `<li class="drow${weDates.has(x.date) ? ' we' : ''}">
      <span class="dd"><b>${x.date === today ? 'Heute' : wdShort(x.date)}</b><small>${ddmm(x.date)}</small></span>
      <span class="dcls c${dc}" style="background:${CLASSES[dc].color}" title="${CLASSES[dc].label}">${dc}</span>
      <span class="dsun">☀︎ ${fmt1(x.sun_h)} h</span>
      <span class="dtemp">${x.tmax}° <small>/ ${x.tmin}°</small></span>
      <span class="dwind">${windArrow(x.wdir)} ${compass(x.wdir)} ${x.wind}<small> km/h</small></span>
    </li>`;
  }).join('');

  el.innerHTML = `
    <div class="dhead">
      <h2><span class="badge-l c${c}" style="background:${CLASSES[c].color}">${c}</span> ${esc(r.name)}</h2>
      <button type="button" class="favbtn${fav ? ' on' : ''}" id="favBtn" aria-pressed="${fav}" aria-label="${fav ? 'Nicht mehr merken' : 'Merken'}">${fav ? '★' : '☆'}</button>
    </div>
    <div class="hint">${CLASSES[c].label} um ${hhmm(latest.hours[i])}${selDate !== today ? ' (morgen)' : ''} · ${r.cc} · ${d.km} km · ${d.txt} ab ${esc(from.name)}</div>
    <div class="stats">
      <div class="stat"><b>${H.sun[i]}%</b><small>Sonne</small></div>
      <div class="stat"><b>${dToday ? fmt1(dToday.sun_h) : '–'} h</b><small>Sonne heute</small></div>
      <div class="stat"><b>${H.temp[i]}°</b><small>Temperatur</small></div>
      <div class="stat"><b>${H.rain[i]}%</b><small>Regenrisiko</small></div>
      <div class="stat wide"><b>${windArrow(H.wdir[i])} ${H.wind[i]} km/h</b><small>Wind aus ${compass(H.wdir[i])} · ${beaufort(H.wind[i])} Bft</small></div>
      <div class="stat wide"><b>${dailyFor(rw, selDate)?.sunrise ?? '–'} – ${dailyFor(rw, selDate)?.sunset ?? '–'}</b><small>Sonnenauf-/-untergang</small></div>
    </div>
    <h3>Stundenverlauf ${selDate === today ? 'heute' : 'morgen'}</h3>
    <div class="hours" role="group" aria-label="Stunde wählen">${hours.join('') || '<p class="hint">Keine Tagesstunden verfügbar.</p>'}</div>
    ${sitesSection(ctx, id)}
    <h3>7-Tage-Ausblick</h3>
    <ul class="days">${days}</ul>`;

  el.querySelectorAll<HTMLButtonElement>('.hour').forEach(b => b.addEventListener('click', () => ctx.setIdx(+b.dataset.idx!)));
  const onFav = () => { toggleFav(id); ctx.renderAll(); };
  el.querySelector('#favBtn')?.addEventListener('click', onFav);
  bindSites(ctx, el);
}
