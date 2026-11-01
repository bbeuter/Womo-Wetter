//! Command line interface and the two pipelines (forecast, stellplaetze).

use crate::aggregate::aggregate_region;
use crate::export::{encode_grid, write_json, GridOut, Latest, Stellplaetze};
use crate::mock;
use crate::openmeteo::{self, PointWeather};
use crate::regions::{build_points, Regions, SamplePoint};
use crate::stellplaetze;
use crate::time::{fmt_dt, TimeAxis};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(
    name = "womo-core",
    version,
    about = "Womo-Wetter: Sonnen-Übersicht je Region"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Vorhersage holen (oder simulieren) und latest.json + grid.json schreiben.
    Forecast(ForecastArgs),
    /// Wohnmobil-Stellplätze aus OpenStreetMap (Overpass) nach stellplaetze.json.
    Stellplaetze(StellplaetzeArgs),
}

#[derive(clap::Args, Debug)]
pub struct ForecastArgs {
    /// Regionen (GeoJSON, DATENFORMAT §1).
    #[arg(long)]
    pub regions: PathBuf,
    /// Ausgabeverzeichnis für latest.json und grid.json.
    #[arg(long)]
    pub out: PathBuf,
    /// Synthetisches Wetter statt Open-Meteo (deterministisch).
    #[arg(long)]
    pub mock: bool,
    /// Gitterweite in Grad.
    #[arg(long, default_value_t = 0.25)]
    pub step: f64,
    /// Referenzzeit (RFC 3339), Standard: jetzt.
    #[arg(long)]
    pub now: Option<String>,
    /// Open-Meteo-Endpunkt.
    #[arg(long, env = "OPEN_METEO_URL", default_value = openmeteo::DEFAULT_URL)]
    pub api_url: String,
    /// Optionaler Open-Meteo-API-Schlüssel (kommerzieller Zugang).
    #[arg(long, env = "OPEN_METEO_APIKEY", hide_env_values = true)]
    pub apikey: Option<String>,
    /// Pause zwischen Paketen in Sekunden (Rate-Limit 600 Aufrufe/min).
    #[arg(long, default_value_t = 10.0)]
    pub pause: f64,
}

#[derive(clap::Args, Debug)]
pub struct StellplaetzeArgs {
    /// Regionen (GeoJSON).
    #[arg(long)]
    pub regions: PathBuf,
    /// Ausgabedatei (stellplaetze.json).
    #[arg(long)]
    pub out: PathBuf,
    /// Gespeicherte Overpass-Antwort statt Live-Abfrage.
    #[arg(long)]
    pub input: Option<PathBuf>,
    /// Overpass-Endpunkt.
    #[arg(long, env = "OVERPASS_URL", default_value = stellplaetze::OVERPASS_URL)]
    pub overpass_url: String,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Forecast(a) => forecast(&a),
        Command::Stellplaetze(a) => stellplaetze_cmd(&a),
    }
}

fn parse_now(now: Option<&str>) -> Result<DateTime<Utc>> {
    match now {
        Some(s) => Ok(DateTime::parse_from_rfc3339(s)
            .with_context(|| format!("--now: invalid RFC 3339 time {s:?}"))?
            .to_utc()),
        None => Ok(Utc::now()),
    }
}

/// Aggregates all regions and builds both output documents.
pub fn build_outputs(
    regions: &Regions,
    grid: &crate::regions::Grid,
    points: &[SamplePoint],
    weather: &[PointWeather],
    axis: &TimeAxis,
    mock: bool,
) -> (Latest, GridOut) {
    let mut by_region: Vec<Vec<usize>> = vec![Vec::new(); regions.list.len()];
    for (i, p) in points.iter().enumerate() {
        by_region[p.region].push(i);
    }
    let mut out = BTreeMap::new();
    for (ri, region) in regions.list.iter().enumerate() {
        let idx = &by_region[ri];
        let pw: Vec<&PointWeather> = idx.iter().map(|&i| &weather[i]).collect();
        out.insert(
            region.id.clone(),
            aggregate_region(&pw, region.center, axis),
        );
    }
    let hours = axis.hour_strings();
    let latest = Latest {
        generated: fmt_dt(&axis.generated),
        source: if mock {
            "mock (synthetisch)".to_string()
        } else {
            format!("open-meteo {}", openmeteo::MODEL)
        },
        mock,
        hours: hours.clone(),
        days: axis.day_strings(),
        regions: out,
    };
    let grid_out = GridOut {
        lon0: grid.lon0,
        lat0: grid.lat0,
        step: grid.step,
        nx: grid.nx,
        ny: grid.ny,
        hours,
        sun: encode_grid(grid, points, weather, axis),
    };
    (latest, grid_out)
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn forecast(a: &ForecastArgs) -> Result<()> {
    let started = Instant::now();
    if !(a.step > 0.0 && a.step <= 5.0) {
        bail!("--step must be in (0, 5]");
    }
    let regions = Regions::load(&a.regions)?;
    let (grid, points) = build_points(&regions, a.step);
    let n_grid = points.iter().filter(|p| p.cell.is_some()).count();
    eprintln!(
        "{} Regionen, Gitter {}×{} ({} Punkte in Regionen, {} Zusatzpunkte)",
        regions.list.len(),
        grid.nx,
        grid.ny,
        n_grid,
        points.len() - n_grid
    );
    let axis = TimeAxis::new(parse_now(a.now.as_deref())?);

    let weather = if a.mock {
        mock::generate(&points, &axis)
    } else {
        let client = openmeteo::Client::new(
            &a.api_url,
            a.apikey.clone(),
            Duration::from_secs_f64(a.pause.max(0.0)),
        )?;
        let coords: Vec<(f64, f64)> = points.iter().map(|p| (p.lon, p.lat)).collect();
        client.fetch_all(&coords)?
    };

    let (latest, grid_out) = build_outputs(&regions, &grid, &points, &weather, &axis, a.mock);
    let latest_path = a.out.join("latest.json");
    let grid_path = a.out.join("grid.json");
    write_json(&latest_path, &latest)?;
    write_json(&grid_path, &grid_out)?;
    eprintln!(
        "geschrieben: {} ({} KB), {} ({} KB) in {:.2} s",
        latest_path.display(),
        file_size(&latest_path) / 1024,
        grid_path.display(),
        file_size(&grid_path) / 1024,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn stellplaetze_cmd(a: &StellplaetzeArgs) -> Result<()> {
    let regions = Regions::load(&a.regions)?;
    let body = match &a.input {
        Some(p) => {
            std::fs::read_to_string(p).with_context(|| format!("cannot read {}", p.display()))?
        }
        None => {
            let q = stellplaetze::build_query(regions.bbox());
            eprintln!("Overpass-Abfrage an {} …", a.overpass_url);
            stellplaetze::fetch_overpass(&a.overpass_url, &q)?
        }
    };
    let items = stellplaetze::parse_overpass(&body, &regions)?;
    let doc = Stellplaetze {
        generated: fmt_dt(&Utc::now().with_timezone(&chrono_tz::Europe::Berlin)),
        items,
    };
    write_json(&a.out, &doc)?;
    eprintln!(
        "{} Stellplätze → {} ({} KB)",
        doc.items.len(),
        a.out.display(),
        file_size(&a.out) / 1024
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_pipeline_on_fixture() {
        let regions =
            Regions::parse(include_str!("../tests/fixtures/two_regions.geojson")).unwrap();
        let (grid, points) = build_points(&regions, 0.25);
        let axis = TimeAxis::new(parse_now(Some("2026-11-07T09:12:00+01:00")).unwrap());
        let weather = mock::generate(&points, &axis);
        let (latest, g) = build_outputs(&regions, &grid, &points, &weather, &axis, true);
        let v = serde_json::to_value(&latest).unwrap();
        assert_eq!(v["generated"], "2026-11-07T09:12+01:00");
        assert_eq!(v["mock"], true);
        assert_eq!(v["hours"].as_array().unwrap().len(), 48);
        assert_eq!(v["days"].as_array().unwrap().len(), 7);
        let west = &v["regions"]["west"];
        assert_eq!(west["hourly"]["cls"].as_str().unwrap().len(), 48);
        for k in ["sun", "sunny", "rain", "temp", "wind", "wdir"] {
            assert_eq!(west["hourly"][k].as_array().unwrap().len(), 48, "{k}");
        }
        let d0 = &west["daily"][0];
        let keys: Vec<&String> = d0.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 10);
        assert!(d0["sunset"].as_str().unwrap().starts_with("16:"));
        // Night at 02:00 everywhere.
        assert_eq!(&west["hourly"]["cls"].as_str().unwrap()[2..3], "N");
        assert_eq!(g.sun.len(), 48);
        assert!(g.sun.iter().all(|s| s.len() == g.nx * g.ny));
        assert!(g.sun[2].chars().all(|c| c == 'N' || c == '.'));
        // Gap column between the squares (lon 7.25) has no data.
        assert_eq!(g.sun[12].as_bytes()[5], b'.');
    }
}
