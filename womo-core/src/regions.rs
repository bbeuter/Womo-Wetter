//! Region polygons (regions.geojson), regular grid and point assignment.

use anyhow::{bail, Context, Result};
use geo::{BoundingRect, Coord, InteriorPoint, Intersects, LineString, MultiPolygon, Point, Polygon, Rect};
use serde::Deserialize;
use std::path::Path;

/// Minimum number of sample points per region.
pub const MIN_POINTS: usize = 3;

#[derive(Debug, Clone)]
pub struct Region {
    pub id: String,
    pub name: String,
    pub cc: String,
    /// (lon, lat), inside the region.
    pub center: (f64, f64),
    pub geom: MultiPolygon<f64>,
    pub bbox: Rect<f64>,
}

impl Region {
    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        let p = Point::new(lon, lat);
        self.bbox.intersects(&p) && self.geom.intersects(&p)
    }
}

#[derive(Debug, Clone)]
pub struct Regions {
    pub list: Vec<Region>,
}

// --- minimal GeoJSON model -------------------------------------------------

#[derive(Deserialize)]
struct FeatureCollection {
    features: Vec<Feature>,
}

#[derive(Deserialize)]
struct Feature {
    properties: Props,
    geometry: Geometry,
}

#[derive(Deserialize)]
struct Props {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    cc: Option<String>,
    #[serde(default)]
    center: Option<[f64; 2]>,
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "coordinates")]
enum Geometry {
    Polygon(Vec<Vec<[f64; 2]>>),
    MultiPolygon(Vec<Vec<Vec<[f64; 2]>>>),
}

fn to_polygon(rings: &[Vec<[f64; 2]>]) -> Option<Polygon<f64>> {
    let mut it = rings.iter().map(|r| {
        LineString::from(r.iter().map(|c| Coord { x: c[0], y: c[1] }).collect::<Vec<_>>())
    });
    let ext = it.next()?;
    Some(Polygon::new(ext, it.collect()))
}

impl Regions {
    pub fn load(path: &Path) -> Result<Self> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        Self::parse(&s).with_context(|| format!("invalid regions file {}", path.display()))
    }

    pub fn parse(s: &str) -> Result<Self> {
        let fc: FeatureCollection = serde_json::from_str(s)?;
        let mut list = Vec::with_capacity(fc.features.len());
        for f in fc.features {
            let polys: Vec<Polygon<f64>> = match &f.geometry {
                Geometry::Polygon(r) => to_polygon(r).into_iter().collect(),
                Geometry::MultiPolygon(ps) => ps.iter().filter_map(|r| to_polygon(r)).collect(),
            };
            let geom = MultiPolygon(polys);
            let Some(bbox) = geom.bounding_rect() else {
                bail!("region {} has empty geometry", f.properties.id);
            };
            let center = match f.properties.center {
                Some([x, y]) => (x, y),
                None => geom
                    .interior_point()
                    .map(|p| (p.x(), p.y()))
                    .unwrap_or_else(|| (bbox.center().x, bbox.center().y)),
            };
            list.push(Region {
                name: f.properties.name.unwrap_or_else(|| f.properties.id.clone()),
                cc: f.properties.cc.unwrap_or_default(),
                id: f.properties.id,
                center,
                geom,
                bbox,
            });
        }
        if list.is_empty() {
            bail!("no regions");
        }
        let mut ids: Vec<&str> = list.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        if let Some(w) = ids.windows(2).find(|w| w[0] == w[1]) {
            bail!("duplicate region id {}", w[0]);
        }
        Ok(Regions { list })
    }

    /// Bounding box of all regions.
    pub fn bbox(&self) -> Rect<f64> {
        let mut min = self.list[0].bbox.min();
        let mut max = self.list[0].bbox.max();
        for r in &self.list[1..] {
            min.x = min.x.min(r.bbox.min().x);
            min.y = min.y.min(r.bbox.min().y);
            max.x = max.x.max(r.bbox.max().x);
            max.y = max.y.max(r.bbox.max().y);
        }
        Rect::new(min, max)
    }

    /// Index of the first region containing the point (boundary counts as inside).
    pub fn locate(&self, lon: f64, lat: f64) -> Option<usize> {
        self.list.iter().position(|r| r.contains(lon, lat))
    }
}

/// Regular lon/lat grid; cell (i, j) center = (lon0 + i*step, lat0 + j*step).
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub lon0: f64,
    pub lat0: f64,
    pub step: f64,
    pub nx: usize,
    pub ny: usize,
}

impl Grid {
    /// Grid covering `bbox`, origin snapped down to a multiple of `step`.
    pub fn covering(bbox: Rect<f64>, step: f64) -> Self {
        let lon0 = round6((bbox.min().x / step).floor() * step);
        let lat0 = round6((bbox.min().y / step).floor() * step);
        let nx = ((bbox.max().x - lon0) / step + 1e-9).floor() as usize + 1;
        let ny = ((bbox.max().y - lat0) / step + 1e-9).floor() as usize + 1;
        Grid {
            lon0,
            lat0,
            step,
            nx,
            ny,
        }
    }

    pub fn len(&self) -> usize {
        self.nx * self.ny
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn center(&self, i: usize, j: usize) -> (f64, f64) {
        (
            round6(self.lon0 + i as f64 * self.step),
            round6(self.lat0 + j as f64 * self.step),
        )
    }
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// A location where weather is sampled.
#[derive(Debug, Clone, PartialEq)]
pub struct SamplePoint {
    pub lon: f64,
    pub lat: f64,
    /// Index into `Regions::list`.
    pub region: usize,
    /// Cell index `j*nx + i` for grid points; `None` for extra points (aggregation only).
    pub cell: Option<usize>,
}

/// Builds grid points inside regions plus extra points for small regions.
pub fn build_points(regions: &Regions, step: f64) -> (Grid, Vec<SamplePoint>) {
    let grid = Grid::covering(regions.bbox(), step);
    let mut points = Vec::new();
    let mut counts = vec![0usize; regions.list.len()];
    for j in 0..grid.ny {
        for i in 0..grid.nx {
            let (lon, lat) = grid.center(i, j);
            if let Some(r) = regions.locate(lon, lat) {
                counts[r] += 1;
                points.push(SamplePoint {
                    lon,
                    lat,
                    region: r,
                    cell: Some(j * grid.nx + i),
                });
            }
        }
    }
    for (ri, region) in regions.list.iter().enumerate() {
        if counts[ri] >= MIN_POINTS {
            continue;
        }
        for (lon, lat) in extra_points(region, step, MIN_POINTS - counts[ri]) {
            points.push(SamplePoint {
                lon,
                lat,
                region: ri,
                cell: None,
            });
        }
    }
    (grid, points)
}

/// Center plus samples from a finer local grid (at least `need` points if possible).
fn extra_points(region: &Region, step: f64, need: usize) -> Vec<(f64, f64)> {
    let mut out = vec![(round6(region.center.0), round6(region.center.1))];
    let mut fine = step / 4.0;
    let mut cand = Vec::new();
    for _ in 0..4 {
        let g = Grid::covering(region.bbox, fine);
        cand = (0..g.ny)
            .flat_map(|j| (0..g.nx).map(move |i| (i, j)))
            .map(|(i, j)| g.center(i, j))
            .filter(|&(x, y)| region.contains(x, y))
            .collect();
        if cand.len() >= need + 1 {
            break;
        }
        fine /= 2.0;
    }
    // Take up to 4 evenly spread candidates.
    let take = cand.len().min(4.max(need));
    for k in 0..take {
        let p = cand[k * cand.len() / take];
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_SQUARES: &str = include_str!("../tests/fixtures/two_regions.geojson");

    #[test]
    fn parse_fixture() {
        let r = Regions::parse(TWO_SQUARES).unwrap();
        assert_eq!(r.list.len(), 2);
        assert_eq!(r.list[0].id, "west");
        assert_eq!(r.list[1].cc, "NL");
        let b = r.bbox();
        assert_eq!((b.min().x, b.min().y, b.max().x, b.max().y), (6.0, 50.0, 7.6, 51.0));
    }

    #[test]
    fn locate_points() {
        let r = Regions::parse(TWO_SQUARES).unwrap();
        assert_eq!(r.locate(6.5, 50.5), Some(0));
        assert_eq!(r.locate(7.5, 50.9), Some(1));
        assert_eq!(r.locate(5.0, 50.5), None);
        // Gap between the squares (7.0..7.4) is outside both.
        assert_eq!(r.locate(7.2, 50.5), None);
    }

    #[test]
    fn grid_snapping() {
        let g = Grid::covering(Rect::new((2.13, 47.02), (2.9, 47.5)), 0.25);
        assert_eq!((g.lon0, g.lat0), (2.0, 47.0));
        assert_eq!((g.nx, g.ny), (4, 3));
        assert_eq!(g.center(3, 2), (2.75, 47.5));
    }

    #[test]
    fn assignment_and_small_region_extras() {
        let r = Regions::parse(TWO_SQUARES).unwrap();
        let (grid, pts) = build_points(&r, 0.25);
        assert_eq!((grid.lon0, grid.lat0, grid.nx, grid.ny), (6.0, 50.0, 7, 5));
        // West square 6..7 x 50..51 contains 5x5 cell centers (boundary included).
        let west: Vec<_> = pts.iter().filter(|p| p.region == 0).collect();
        assert_eq!(west.len(), 25);
        assert!(west.iter().all(|p| p.cell.is_some()));
        // East square 7.4..7.6 contains only the centers at lon 7.5 (5 points).
        let east: Vec<_> = pts.iter().filter(|p| p.region == 1).collect();
        assert!(east.len() >= MIN_POINTS);
        for p in &pts {
            if let Some(c) = p.cell {
                let (lon, lat) = grid.center(c % grid.nx, c / grid.nx);
                assert_eq!((lon, lat), (p.lon, p.lat));
            }
        }
    }

    #[test]
    fn tiny_region_gets_extra_points() {
        let gj = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"id":"big","name":"Big","cc":"DE","center":[6.5,50.5]},
           "geometry":{"type":"Polygon","coordinates":[[[6,50],[7,50],[7,51],[6,51],[6,50]]]}},
          {"type":"Feature","properties":{"id":"tiny","name":"Tiny","cc":"LU","center":[7.6,50.6]},
           "geometry":{"type":"MultiPolygon","coordinates":[[[[7.55,50.55],[7.65,50.55],[7.65,50.65],[7.55,50.65],[7.55,50.55]]]]}}
        ]}"#;
        let r = Regions::parse(gj).unwrap();
        let (_, pts) = build_points(&r, 0.25);
        let tiny: Vec<_> = pts.iter().filter(|p| p.region == 1).collect();
        assert!(tiny.len() >= MIN_POINTS, "{}", tiny.len());
        assert!(tiny.iter().all(|p| p.cell.is_none()));
        assert!(tiny.iter().any(|p| (p.lon, p.lat) == (7.6, 50.6)));
        assert!(tiny.iter().all(|p| r.list[1].contains(p.lon, p.lat)));
    }

    #[test]
    fn duplicate_ids_rejected() {
        let gj = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"id":"a"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}},
          {"type":"Feature","properties":{"id":"a"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}}]}"#;
        assert!(Regions::parse(gj).is_err());
    }
}
