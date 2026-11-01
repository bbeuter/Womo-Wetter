// Typen gemäß docs/DATENFORMAT.md + Laden der Dateien relativ zur App (GitHub-Pages-tauglich).

export type Cls = 'S' | 'H' | 'W' | 'B' | 'R' | 'N';
export type LonLat = [number, number];

export interface RegionProps { id: string; name: string; cc: 'DE' | 'NL' | 'BE' | 'LU' | 'FR'; center: LonLat }
export type Polygon = LonLat[][];
export interface RegionFeature {
  type: 'Feature';
  properties: RegionProps;
  geometry: { type: 'Polygon'; coordinates: Polygon } | { type: 'MultiPolygon'; coordinates: Polygon[] };
}
export interface RegionsGeo { type: 'FeatureCollection'; features: RegionFeature[] }

export interface Hourly {
  cls: string; sun: number[]; sunny: number[]; rain: number[];
  temp: number[]; wind: number[]; wdir: number[];
}
export interface Daily {
  date: string; sunrise: string; sunset: string; sun_h: number;
  tmax: number; tmin: number; rain: number; wind: number; wdir: number; cls: Cls;
}
export interface RegionWx { hourly: Hourly; daily: Daily[] }
export interface Latest {
  generated: string; source: string; mock?: boolean;
  hours: string[]; days: string[];
  regions: Record<string, RegionWx>;
}
export interface Grid {
  lon0: number; lat0: number; step: number; nx: number; ny: number;
  hours: string[]; sun: string[];
}
export interface Stellplatz {
  id: string; name?: string; lon: number; lat: number; region: string;
  type?: string; fee?: string; website?: string;
  am?: string; cap?: number; stars?: number;
}
export interface Stellplaetze { generated: string; items: Stellplatz[] }

/** Region mit vorbereiteter Geometrie (MultiPolygon normalisiert). */
export interface Region extends RegionProps { polys: Polygon[] }

export interface AppData {
  regions: Region[];
  byId: Map<string, Region>;
  latest: Latest;
  grid: Grid | null;
  sites: Stellplatz[] | null;
}

async function getJson<T>(file: string): Promise<T> {
  const res = await fetch(`./data/${file}`, { cache: 'no-cache' });
  if (!res.ok) throw new Error(`${file}: HTTP ${res.status}`);
  return res.json() as Promise<T>;
}
const optional = <T>(p: Promise<T>): Promise<T | null> => p.catch(err => { console.warn(err); return null; });

export async function loadData(): Promise<AppData> {
  const [geo, latest, grid, sites] = await Promise.all([
    getJson<RegionsGeo>('regions.geojson'),
    getJson<Latest>('latest.json'),
    optional(getJson<Grid>('grid.json')),
    optional(getJson<Stellplaetze>('stellplaetze.json')),
  ]);
  const regions: Region[] = geo.features
    .filter(f => latest.regions[f.properties.id])
    .map(f => ({
      ...f.properties,
      polys: f.geometry.type === 'Polygon' ? [f.geometry.coordinates] : f.geometry.coordinates,
    }));
  return {
    regions,
    byId: new Map(regions.map(r => [r.id, r])),
    latest,
    grid: grid && grid.sun?.length ? grid : null,
    sites: sites && Array.isArray(sites.items) ? sites.items : null,
  };
}

// ---------- Klassen ----------
export const CLASSES: Record<Cls, { label: string; color: string; icon: string }> = {
  S: { label: 'Sonnig', color: 'var(--S)', icon: '☀︎' },
  H: { label: 'Heiter', color: 'var(--H)', icon: '🌤' },
  W: { label: 'Wolkig', color: 'var(--W)', icon: '⛅' },
  B: { label: 'Bedeckt', color: 'var(--B)', icon: '☁︎' },
  R: { label: 'Regen', color: 'var(--R)', icon: '🌧' },
  N: { label: 'Nacht', color: 'var(--N)', icon: '☾' },
};
export const asCls = (c: string | undefined): Cls => (c && c in CLASSES ? (c as Cls) : 'B');

// ---------- Zeit (Europe/Berlin) ----------
// Alle Zeitstempel der Daten sind bereits Berliner Ortszeit → Datum/Stunde direkt aus dem String lesen.
const TZ = 'Europe/Berlin';
const partsFmt = new Intl.DateTimeFormat('en-CA', {
  timeZone: TZ, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', weekday: 'short', hourCycle: 'h23',
});
export interface BerlinNow { date: string; hour: number; minute: number; weekday: number /* 0=So */ }
const WD: Record<string, number> = { Sun: 0, Mon: 1, Tue: 2, Wed: 3, Thu: 4, Fri: 5, Sat: 6 };
export function berlinNow(d = new Date()): BerlinNow {
  const p = Object.fromEntries(partsFmt.formatToParts(d).map(x => [x.type, x.value]));
  return { date: `${p.year}-${p.month}-${p.day}`, hour: +p.hour % 24, minute: +p.minute, weekday: WD[p.weekday] ?? 0 };
}
/** ISO mit Offset ("2026-11-07T09:12+01:00") → Epoche ms (ohne Date.parse-Eigenheiten von Safari). */
export function parseIso(s: string): number {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?(?:\.\d+)?(Z|([+-])(\d{2}):?(\d{2}))?$/.exec(s);
  if (!m) return NaN;
  const utc = Date.UTC(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +(m[6] ?? 0));
  if (!m[7] || m[7] === 'Z') return utc;
  const off = (+m[9] * 60 + +m[10]) * (m[8] === '-' ? -1 : 1);
  return utc - off * 60000;
}
export const hhmm = (iso: string): string => iso.slice(11, 16);
export const dateOf = (iso: string): string => iso.slice(0, 10);
export const hourOf = (iso: string): number => +iso.slice(11, 13);
/** "17:03" → Minuten seit Mitternacht */
export const minutes = (t: string): number => { const [h, m] = t.split(':').map(Number); return h * 60 + (m || 0); };

const WD_DE = ['So', 'Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa'];
export function weekdayOf(date: string): number {
  const [y, m, d] = date.split('-').map(Number);
  return new Date(Date.UTC(y, m - 1, d)).getUTCDay();
}
export const wdShort = (date: string): string => WD_DE[weekdayOf(date)];
export const ddmm = (date: string): string => `${date.slice(8, 10)}.${date.slice(5, 7)}.`;

/** Index der aktuellen Stunde in latest.hours (geklemmt auf den gültigen Bereich). */
export function currentHourIndex(hours: string[], now: BerlinNow): number {
  const key = `${now.date}T${String(now.hour).padStart(2, '0')}`;
  const i = hours.findIndex(h => h.slice(0, 13) === key);
  if (i >= 0) return i;
  // außerhalb: vor dem Datenbeginn → 0, danach → letzter Index
  return key < hours[0].slice(0, 13) ? 0 : hours.length - 1;
}

// ---------- Wind ----------
const COMPASS = ['N', 'NO', 'O', 'SO', 'S', 'SW', 'W', 'NW'];
export const compass = (deg: number): string => COMPASS[Math.round((((deg % 360) + 360) % 360) / 45) % 8];
export function beaufort(kmh: number): number {
  const lim = [1, 6, 12, 20, 29, 39, 50, 62, 75, 89, 103, 118];
  const i = lim.findIndex(l => kmh < l);
  return i < 0 ? 12 : i;
}
