import { defineConfig, type Plugin } from 'vite';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

// Erzeugt dist/sw.js aus src/sw-template.js und trägt die gebauten
// Dateien (App-Shell) + eine Version in die Precache-Liste ein.
function serviceWorker(): Plugin {
  return {
    name: 'sonnen-radar-sw',
    apply: 'build',
    generateBundle(_opts, bundle) {
      const files = Object.keys(bundle).filter(f => !f.endsWith('.map'));
      const shell = ['./', ...files.map(f => './' + f), './manifest.webmanifest', './icons/icon-192.png', './icons/icon-512.png', './icons/apple-touch-icon.png', './icons/icon.svg'];
      const version = createHash('sha1').update(files.sort().join('|')).digest('hex').slice(0, 10);
      const tpl = readFileSync(new URL('./src/sw-template.js', import.meta.url), 'utf8');
      this.emitFile({
        type: 'asset',
        fileName: 'sw.js',
        source: tpl.replace('__VERSION__', version).replace('__SHELL__', JSON.stringify(shell)),
      });
    },
  };
}

export default defineConfig({
  base: './',
  plugins: [serviceWorker()],
  build: { target: 'es2020', assetsInlineLimit: 0, sourcemap: false },
});
