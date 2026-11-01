// Einstellungs-Sheet: Startort (Heimatort / GPS) und Reisetempo.
import { esc } from './dom';
import { TOWNS } from './towns';
import { saveSettings, settings, startPlace } from './store';

const CC_NAME: Record<string, string> = { DE: 'Deutschland', NL: 'Niederlande', BE: 'Belgien', LU: 'Luxemburg', FR: 'Frankreich' };

export function startLabel(): string {
  const s = startPlace();
  return s.gps ? '📍 GPS-Standort' : `📍 Start: ${s.name}`;
}

export function openSettings(dlg: HTMLDialogElement, onChange: () => void): void {
  const groups = ['DE', 'NL', 'BE', 'LU'].map(cc => `<optgroup label="${CC_NAME[cc]}">${TOWNS.filter(t => t.cc === cc)
    .map(t => `<option value="${esc(t.name)}"${t.name === settings.home.name ? ' selected' : ''}>${esc(t.name)}</option>`).join('')}</optgroup>`).join('');
  const custom = !TOWNS.some(t => t.name === settings.home.name);
  dlg.innerHTML = `
    <form method="dialog" class="sheetbody">
      <div class="sheethead">
        <h2 id="settingsTitle">Einstellungen</h2>
        <button class="close" value="close" aria-label="Schließen">✕</button>
      </div>
      <fieldset class="modes">
        <legend>Fahrzeit berechnen ab</legend>
        <label class="opt"><input type="radio" name="mode" value="home"${settings.mode === 'home' ? ' checked' : ''}>
          <span><b>Fester Heimatort</b><small>${esc(settings.home.name)}</small></span></label>
        <label class="opt"><input type="radio" name="mode" value="gps"${settings.mode === 'gps' ? ' checked' : ''}>
          <span><b>Aktueller Standort (GPS)</b><small id="gpsInfo">${settings.lastGps ? `zuletzt ${new Date(settings.lastGps.at).toLocaleString('de-DE', { dateStyle: 'short', timeStyle: 'short' })}` : 'noch nicht ermittelt'}</small></span></label>
      </fieldset>
      <p class="msg" id="gpsMsg" role="status"></p>

      <h3>Heimatort</h3>
      <label class="field"><span>Ort wählen</span>
        <select id="townSel">${custom ? `<option selected value="">${esc(settings.home.name)} (eigene Koordinaten)</option>` : ''}${groups}</select></label>
      <details class="manual"${custom ? ' open' : ''}>
        <summary>Koordinaten manuell eingeben</summary>
        <div class="grid2">
          <label class="field"><span>Breite (lat)</span><input id="lat" inputmode="decimal" autocomplete="off" value="${settings.home.lat}"></label>
          <label class="field"><span>Länge (lon)</span><input id="lon" inputmode="decimal" autocomplete="off" value="${settings.home.lon}"></label>
        </div>
        <label class="field"><span>Name</span><input id="hname" autocomplete="off" value="${esc(custom ? settings.home.name : 'Mein Ort')}"></label>
        <button type="button" class="btn" id="applyManual">Koordinaten übernehmen</button>
      </details>

      <h3>Reisetempo Womo</h3>
      <label class="field range"><span><output id="spOut">${settings.speed}</output> km/h Durchschnitt</span>
        <input type="range" id="speed" min="50" max="110" step="5" value="${settings.speed}" aria-label="Reisetempo in km/h"></label>
      <p class="hint">Fahrzeit ≈ Luftlinie × 1,3 ÷ Tempo.</p>

      <h3>Navigation</h3>
      <fieldset class="modes">
        <legend class="sr">Route öffnen mit</legend>
        <label class="opt"><input type="radio" name="nav" value="google"${settings.nav === 'google' ? ' checked' : ''}>
          <span><b>Google Maps</b></span></label>
        <label class="opt"><input type="radio" name="nav" value="apple"${settings.nav === 'apple' ? ' checked' : ''}>
          <span><b>Apple Karten</b></span></label>
      </fieldset>
      <button class="btn primary done" value="ok">Fertig</button>
    </form>`;

  const q = <T extends HTMLElement>(s: string) => dlg.querySelector(s) as T;
  const msg = q<HTMLParagraphElement>('#gpsMsg');
  const commit = () => { saveSettings(); onChange(); };

  const locate = () => {
    if (!('geolocation' in navigator)) { msg.textContent = 'GPS wird von diesem Gerät nicht unterstützt – Heimatort wird verwendet.'; return; }
    msg.textContent = 'Standort wird ermittelt …';
    navigator.geolocation.getCurrentPosition(pos => {
      settings.lastGps = { name: 'GPS-Standort', lat: +pos.coords.latitude.toFixed(4), lon: +pos.coords.longitude.toFixed(4), at: Date.now() };
      msg.textContent = `Standort: ${settings.lastGps.lat.toFixed(3)}, ${settings.lastGps.lon.toFixed(3)}`;
      q('#gpsInfo').textContent = 'gerade eben';
      commit();
    }, err => {
      msg.textContent = (err.code === err.PERMISSION_DENIED ? 'Standortzugriff verweigert.' : 'Standort nicht verfügbar.') +
        (settings.lastGps ? ' Letzte bekannte Position wird verwendet.' : ' Heimatort wird verwendet.');
    }, { enableHighAccuracy: false, timeout: 10000, maximumAge: 10 * 60000 });
  };

  dlg.querySelectorAll<HTMLInputElement>('input[name=mode]').forEach(r => r.addEventListener('change', () => {
    settings.mode = r.value === 'gps' ? 'gps' : 'home';
    commit();
    if (settings.mode === 'gps') locate(); else msg.textContent = '';
  }));
  q<HTMLSelectElement>('#townSel').addEventListener('change', e => {
    const t = TOWNS.find(x => x.name === (e.target as HTMLSelectElement).value);
    if (!t) return;
    settings.home = { name: t.name, lat: t.lat, lon: t.lon };
    q('.modes .opt small').textContent = t.name;
    (q<HTMLInputElement>('#lat')).value = String(t.lat); (q<HTMLInputElement>('#lon')).value = String(t.lon);
    commit();
  });
  q('#applyManual').addEventListener('click', () => {
    const lat = parseFloat(q<HTMLInputElement>('#lat').value.replace(',', '.'));
    const lon = parseFloat(q<HTMLInputElement>('#lon').value.replace(',', '.'));
    if (!(lat >= 40 && lat <= 60 && lon >= -5 && lon <= 25)) { msg.textContent = 'Bitte gültige Koordinaten eingeben (z. B. 50.94 / 6.96).'; return; }
    const name = q<HTMLInputElement>('#hname').value.trim().slice(0, 40) || 'Mein Ort';
    settings.home = { name, lat, lon };
    q('.modes .opt small').textContent = name;
    msg.textContent = `Heimatort gesetzt: ${name}`;
    commit();
  });
  dlg.querySelectorAll<HTMLInputElement>('input[name=nav]').forEach(r => r.addEventListener('change', () => {
    settings.nav = r.value === 'apple' ? 'apple' : 'google';
    commit();
  }));
  const sp = q<HTMLInputElement>('#speed');
  sp.addEventListener('input', () => { q('#spOut').textContent = sp.value; });
  sp.addEventListener('change', () => { settings.speed = +sp.value; commit(); });

  if (typeof dlg.showModal === 'function') dlg.showModal(); else dlg.setAttribute('open', '');
  // Tippen auf den Hintergrund schließt
  dlg.onclick = e => { if (e.target === dlg) dlg.close(); };
  if (settings.mode === 'gps' && !settings.lastGps) locate();
}
