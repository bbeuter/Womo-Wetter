// Fachlogik: Slots, Sonnenuntergang, Ranking „Beste Ziele“.
import { dateOf, hourOf, minutes, type AppData, type BerlinNow, type Daily, type Latest, type Region, type RegionWx } from './data';
import { drive, type Drive } from './geo';
import { startPlace, settings, favs } from './store';

export const SLOT_OFFSETS = [0, 2, 4, 6];
/** Sonnenstunden-Malus je Fahrstunde. */
export const DRIVE_PENALTY = 0.35;

export const dailyFor = (rw: RegionWx, date: string): Daily | undefined => rw.daily.find(d => d.date === date);

/** Median-Sonnenuntergang aller Regionen für ein Datum ("HH:MM"). */
export function medianSunset(latest: Latest, date: string): string | null {
  const v: number[] = [];
  for (const id in latest.regions) { const d = dailyFor(latest.regions[id], date); if (d?.sunset) v.push(minutes(d.sunset)); }
  if (!v.length) return null;
  v.sort((a, b) => a - b);
  const m = v[Math.floor(v.length / 2)];
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
}
/** Liegt die Stunde (Beginn) so, dass die Sonne in ihr oder davor untergeht? */
export const atOrAfterSunset = (hourIso: string, sunset: string | null): boolean =>
  !!sunset && hourOf(hourIso) * 60 + 60 > minutes(sunset);

export interface SunAhead { hours: number; tomorrow: boolean }
/** Sonnenstunden ab Stunde idx bis Sonnenuntergang heute; nach Sonnenuntergang: morgen (daily.sun_h). */
export function sunAhead(latest: Latest, rw: RegionWx, idx: number): SunAhead {
  const date = dateOf(latest.hours[idx]);
  const d = dailyFor(rw, date);
  const ss = d ? minutes(d.sunset) : 24 * 60;
  let s = 0, n = 0;
  for (let i = idx; i < latest.hours.length && dateOf(latest.hours[i]) === date && hourOf(latest.hours[i]) * 60 < ss; i++) {
    s += (rw.hourly.sun[i] ?? 0) / 100; n++;
  }
  if (n) return { hours: s, tomorrow: false };
  const di = rw.daily.findIndex(x => x.date === date);
  const next = di >= 0 ? rw.daily[di + 1] : undefined;
  return { hours: next?.sun_h ?? 0, tomorrow: true };
}

/** Wochenendtage: Sa → Sa+So, So → nur heute, sonst nächstes Sa+So. */
export function weekendDates(days: string[], now: BerlinNow): string[] {
  const wd = now.weekday;
  const start = days.indexOf(now.date);
  const base = start >= 0 ? start : 0;
  let want: number[];
  if (wd === 6) want = [0, 1]; else if (wd === 0) want = [0]; else want = [6 - wd, 7 - wd];
  return want.map(o => days[base + o]).filter((d): d is string => !!d);
}

export interface Ranked { r: Region; score: number; sunH: number; tomorrow: boolean; d: Drive; fav: boolean; wk?: Daily[] }

export function rank(data: AppData, idx: number, mode: 'heute' | 'we', weDates: string[]): Ranked[] {
  const from = startPlace();
  return data.regions.map(r => {
    const rw = data.latest.regions[r.id];
    const d = drive(from, { lon: r.center[0], lat: r.center[1] }, settings.speed);
    if (mode === 'we') {
      const wk = weDates.map(dt => dailyFor(rw, dt)).filter((x): x is Daily => !!x);
      const sunH = wk.reduce((a, x) => a + x.sun_h, 0);
      return { r, sunH, tomorrow: false, d, wk, fav: favs.has(r.id), score: sunH - DRIVE_PENALTY * d.hours };
    }
    const sa = sunAhead(data.latest, rw, idx);
    return { r, sunH: sa.hours, tomorrow: sa.tomorrow, d, fav: favs.has(r.id), score: sa.hours - DRIVE_PENALTY * d.hours };
  }).sort((a, b) => b.score - a.score);
}
