export const $ = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;
export const SVGNS = 'http://www.w3.org/2000/svg';
export function svgEl<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, parent?: Element): SVGElementTagNameMap[K] {
  const e = document.createElementNS(SVGNS, tag);
  for (const k in attrs) e.setAttribute(k, String(attrs[k]));
  parent?.appendChild(e);
  return e;
}
const ESC: Record<string, string> = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
export const esc = (s: string): string => s.replace(/[&<>"']/g, c => ESC[c]);
/** Nur http(s)-Links durchlassen (Daten stammen aus OSM). */
export const safeUrl = (u: string | undefined): string | null => (u && /^https?:\/\//i.test(u) ? u : null);
export const reducedMotion = (): boolean => matchMedia('(prefers-reduced-motion: reduce)').matches;
export const fmt1 = (v: number): string => v.toLocaleString('de-DE', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
