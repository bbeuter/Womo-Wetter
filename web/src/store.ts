// Persistente Einstellungen (localStorage) + kleiner Event-Bus.
import { DEFAULT_HOME } from './towns';
import type { Pt } from './geo';

export interface Place extends Pt { name: string }
export interface Settings {
  mode: 'home' | 'gps';
  home: Place;
  speed: number;           // km/h
  lastGps?: Place & { at: number };
  nav: 'apple' | 'google'; // Navigations-App für "Route"
  siteFilter: 'all' | 'caravan_site' | 'camp_site';
}
const KEY = 'sonnenradar.settings.v1';
const FAV = 'sonnenradar.favs.v1';

function read<T>(key: string, fallback: T): T {
  try { const s = localStorage.getItem(key); return s ? { ...fallback, ...JSON.parse(s) } : fallback; } catch { return fallback; }
}
const isApple = typeof navigator !== 'undefined' && /iPhone|iPad|iPod|Macintosh/.test(navigator.userAgent);
export const settings: Settings = read<Settings>(KEY, {
  mode: 'home', home: { ...DEFAULT_HOME }, speed: 75, nav: isApple ? 'apple' : 'google', siteFilter: 'all',
});
if (settings.nav !== 'apple' && settings.nav !== 'google') settings.nav = isApple ? 'apple' : 'google';
if (!['all', 'caravan_site', 'camp_site'].includes(settings.siteFilter)) settings.siteFilter = 'all';
if (!(settings.speed >= 30 && settings.speed <= 130)) settings.speed = 75;

export function saveSettings(): void {
  try { localStorage.setItem(KEY, JSON.stringify(settings)); } catch { /* privat/voll */ }
}

let favList: string[] = [];
try { favList = JSON.parse(localStorage.getItem(FAV) || '[]'); } catch { favList = []; }
export const favs = new Set<string>(Array.isArray(favList) ? favList : []);
export function toggleFav(id: string): boolean {
  favs.has(id) ? favs.delete(id) : favs.add(id);
  try { localStorage.setItem(FAV, JSON.stringify([...favs])); } catch { /* ignorieren */ }
  return favs.has(id);
}

/** Aktuell gültiger Startort (GPS nur, wenn eine Position vorliegt). */
export function startPlace(): Place & { gps: boolean } {
  if (settings.mode === 'gps' && settings.lastGps) return { ...settings.lastGps, gps: true };
  return { ...settings.home, gps: false };
}
