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

/** Zusätzliche Orte nur zur Kartenbeschriftung im Detail-Zoom. */
export const LABEL_TOWNS: Town[] = [
  T('Straßburg', 'FR', 48.573, 7.752), T('Colmar', 'FR', 48.079, 7.358), T('Mülhausen', 'FR', 47.750, 7.336),
  T('Metz', 'FR', 49.120, 6.176), T('Nancy', 'FR', 48.692, 6.184), T('Épinal', 'FR', 48.173, 6.450),
  T('Lille', 'FR', 50.629, 3.057), T('Reims', 'FR', 49.258, 4.032), T('Charleville', 'FR', 49.773, 4.720),
  T('Dünkirchen', 'FR', 51.034, 2.377), T('Basel', 'FR', 47.560, 7.588),
  T('Monschau', 'DE', 50.555, 6.240), T('Daun', 'DE', 50.198, 6.832), T('Cochem', 'DE', 50.147, 7.167),
  T('Bitburg', 'DE', 49.967, 6.527), T('Prüm', 'DE', 50.207, 6.424), T('Bernkastel', 'DE', 49.916, 7.071),
  T('Garmisch', 'DE', 47.492, 11.096), T('Berchtesgaden', 'DE', 47.631, 13.002), T('Lindau', 'DE', 47.546, 9.684),
  T('Titisee', 'DE', 47.903, 8.155), T('Baden-Baden', 'DE', 48.761, 8.240), T('Winterberg', 'DE', 51.193, 8.533),
  T('Wernigerode', 'DE', 51.835, 10.785), T('Fulda', 'DE', 50.555, 9.680), T('Bamberg', 'DE', 49.898, 10.902),
  T('Bayreuth', 'DE', 49.946, 11.578), T('Cuxhaven', 'DE', 53.861, 8.694), T('Husum', 'DE', 54.477, 9.052),
  T('Sylt', 'DE', 54.907, 8.317), T('Stralsund', 'DE', 54.309, 13.082), T('Görlitz', 'DE', 51.153, 14.987),
  T('Frankfurt (Oder)', 'DE', 52.347, 14.551), T('Lüneburg', 'DE', 53.249, 10.411), T('Jena', 'DE', 50.927, 11.586),
  T('Texel', 'NL', 53.055, 4.797), T('Leeuwarden', 'NL', 53.201, 5.800), T('Middelburg', 'NL', 51.499, 3.611),
  T('Venlo', 'NL', 51.370, 6.172), T('Nijmegen', 'NL', 51.842, 5.853),
  T('Ostende', 'BE', 51.216, 2.927), T('Bastogne', 'BE', 50.003, 5.716), T('Spa', 'BE', 50.492, 5.865),
  T('Dinant', 'BE', 50.261, 4.912), T('Mons', 'BE', 50.454, 3.952), T('Hasselt', 'BE', 50.931, 5.338),
  T('Clerf', 'LU', 50.055, 6.028), T('Echternach', 'LU', 49.812, 6.418),
];
