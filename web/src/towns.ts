// Eingebaute Ortsliste für den Heimatort (offline, keine Suche per Geocoder nötig).
export interface Town { name: string; cc: 'DE' | 'NL' | 'BE' | 'LU' | 'FR'; lon: number; lat: number }
const T = (name: string, cc: Town['cc'], lat: number, lon: number): Town => ({ name, cc, lat, lon });

export const TOWNS: Town[] = [
  T('Aachen', 'DE', 50.776, 6.084), T('Augsburg', 'DE', 48.371, 10.898), T('Berlin', 'DE', 52.520, 13.405),
  T('Bielefeld', 'DE', 52.021, 8.532), T('Bochum', 'DE', 51.482, 7.216), T('Bonn', 'DE', 50.737, 7.098),
  T('Braunschweig', 'DE', 52.269, 10.521), T('Bremen', 'DE', 53.079, 8.802), T('Chemnitz', 'DE', 50.833, 12.925),
  T('Dortmund', 'DE', 51.514, 7.466), T('Dresden', 'DE', 51.050, 13.738), T('Duisburg', 'DE', 51.434, 6.762),
  T('Düsseldorf', 'DE', 51.227, 6.773), T('Erfurt', 'DE', 50.978, 11.029), T('Essen', 'DE', 51.456, 7.012),
  T('Flensburg', 'DE', 54.794, 9.437), T('Frankfurt am Main', 'DE', 50.110, 8.682), T('Freiburg', 'DE', 47.999, 7.842),
  T('Göttingen', 'DE', 51.541, 9.916), T('Hamburg', 'DE', 53.551, 9.994), T('Hannover', 'DE', 52.376, 9.732),
  T('Heidelberg', 'DE', 49.399, 8.672), T('Karlsruhe', 'DE', 49.007, 8.404), T('Kassel', 'DE', 51.313, 9.479),
  T('Kiel', 'DE', 54.323, 10.123), T('Koblenz', 'DE', 50.356, 7.593), T('Köln', 'DE', 50.938, 6.960),
  T('Konstanz', 'DE', 47.660, 9.175), T('Leipzig', 'DE', 51.340, 12.375), T('Lübeck', 'DE', 53.866, 10.686),
  T('Magdeburg', 'DE', 52.121, 11.628), T('Mainz', 'DE', 49.993, 8.247), T('Mannheim', 'DE', 49.487, 8.466),
  T('München', 'DE', 48.137, 11.576), T('Münster', 'DE', 51.961, 7.626), T('Nürnberg', 'DE', 49.452, 11.077),
  T('Oldenburg', 'DE', 53.144, 8.214), T('Osnabrück', 'DE', 52.279, 8.047), T('Passau', 'DE', 48.567, 13.431),
  T('Potsdam', 'DE', 52.391, 13.064), T('Regensburg', 'DE', 49.013, 12.102), T('Rostock', 'DE', 54.092, 12.099),
  T('Saarbrücken', 'DE', 49.240, 6.997), T('Schwerin', 'DE', 53.636, 11.401), T('Stuttgart', 'DE', 48.776, 9.183),
  T('Trier', 'DE', 49.750, 6.637), T('Ulm', 'DE', 48.401, 9.988), T('Würzburg', 'DE', 49.792, 9.954),
  T('Wuppertal', 'DE', 51.256, 7.150),
  T('Amsterdam', 'NL', 52.370, 4.895), T('Arnhem', 'NL', 51.985, 5.899), T('Eindhoven', 'NL', 51.441, 5.470),
  T('Groningen', 'NL', 53.219, 6.567), T('Maastricht', 'NL', 50.851, 5.691), T('Rotterdam', 'NL', 51.924, 4.478),
  T('Den Haag', 'NL', 52.070, 4.300), T('Utrecht', 'NL', 52.091, 5.122), T('Zwolle', 'NL', 52.517, 6.083),
  T('Antwerpen', 'BE', 51.219, 4.402), T('Brügge', 'BE', 51.209, 3.225), T('Brüssel', 'BE', 50.850, 4.352),
  T('Gent', 'BE', 51.054, 3.717), T('Lüttich', 'BE', 50.633, 5.567), T('Namur', 'BE', 50.467, 4.867),
  T('Eupen', 'BE', 50.628, 6.036),
  T('Luxemburg', 'LU', 49.612, 6.130), T('Esch-sur-Alzette', 'LU', 49.496, 5.981),
];
export const DEFAULT_HOME: Town = TOWNS.find(t => t.name === 'Köln')!;
