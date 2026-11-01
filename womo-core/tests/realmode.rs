//! Real-mode pipeline against a local fake Open-Meteo server (batching, parsing, aggregation).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Timelike;
use womo_core::cli::build_outputs;
use womo_core::openmeteo::Client;
use womo_core::regions::{build_points, Regions};
use womo_core::time::TimeAxis;

/// 2026-11-07T00:00+01:00
const T0: i64 = 1_794_006_000;

/// One location, 7 days + 1 h in Open-Meteo convention: sunshine covers the
/// preceding hour and is full for 08:00–16:00 local; is_day is 1 at 08..=16.
fn location_json() -> serde_json::Value {
    let n = 7 * 24 + 1;
    let times: Vec<i64> = (0..n).map(|k| T0 + k * 3600).collect();
    let hour = |t: &i64| womo_core::time::to_local(*t).hour();
    serde_json::json!({
        "latitude": 50.5, "longitude": 6.5, "utc_offset_seconds": 3600,
        "timezone": "Europe/Berlin",
        "hourly": {
            "time": times,
            "sunshine_duration": times.iter().map(|t| if (9..=16).contains(&hour(t)) { 3600.0 } else { 0.0 }).collect::<Vec<_>>(),
            "precipitation_probability": vec![10; n as usize],
            "precipitation": vec![0.0; n as usize],
            "temperature_2m": times.iter().map(|t| 10.0 + hour(t) as f64 / 2.0).collect::<Vec<_>>(),
            "wind_speed_10m": vec![10.0; n as usize],
            "wind_direction_10m": vec![250; n as usize],
            "is_day": times.iter().map(|t| u8::from((8..=16).contains(&hour(t)))).collect::<Vec<_>>(),
        }
    })
}

/// Fake Open-Meteo: answers each request with one location per requested latitude.
fn spawn_server(requests: Arc<AtomicUsize>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let one = location_json();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            // Drain headers.
            loop {
                let mut h = String::new();
                reader.read_line(&mut h).unwrap();
                if h == "\r\n" || h.is_empty() {
                    break;
                }
            }
            requests.fetch_add(1, Ordering::SeqCst);
            let lat = line
                .split(['?', '&', ' '])
                .find_map(|p| p.strip_prefix("latitude="))
                .unwrap_or("");
            let n = lat.split(',').count();
            let body = serde_json::to_string(&vec![one.clone(); n]).unwrap();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).unwrap();
        }
    });
    format!("http://{addr}/v1/forecast")
}

#[test]
fn real_mode_with_fake_server() {
    let requests = Arc::new(AtomicUsize::new(0));
    let url = spawn_server(requests.clone());
    let regions = Regions::parse(include_str!("fixtures/two_regions.geojson")).unwrap();
    let (grid, points) = build_points(&regions, 0.25);
    assert_eq!(points.len(), 30);

    let mut client = Client::new(&url, None, Duration::ZERO).unwrap();
    client.batch_size = 8;
    let coords: Vec<(f64, f64)> = points.iter().map(|p| (p.lon, p.lat)).collect();
    let weather = client.fetch_all(&coords).unwrap();
    assert_eq!(weather.len(), 30);
    assert_eq!(requests.load(Ordering::SeqCst), 4); // 8 + 8 + 8 + 6

    let now = chrono::DateTime::parse_from_rfc3339("2026-11-07T09:12:00+01:00")
        .unwrap()
        .to_utc();
    let axis = TimeAxis::new(now);
    let (latest, g) = build_outputs(&regions, &grid, &points, &weather, &axis, false);
    let v = serde_json::to_value(&latest).unwrap();
    assert_eq!(v["mock"], false);
    assert_eq!(v["source"], "open-meteo icon_seamless");
    let west = &v["regions"]["west"];
    let cls = west["hourly"]["cls"].as_str().unwrap();
    // Hour h describes h..h+1: 07 is N (no sun before 08:00), 08–15 sunny,
    // 16 is day (is_day at 16:00) without sun, 17 night.
    assert_eq!(&cls[0..24], "NNNNNNNNSSSSSSSSBNNNNNNN");
    assert_eq!(&cls[24..48], &cls[0..24]);
    assert_eq!(west["hourly"]["sun"][8], 100);
    assert_eq!(west["hourly"]["sunny"][8], 100);
    assert_eq!(west["hourly"]["sun"][16], 0);
    assert_eq!(west["hourly"]["temp"][10], 15);
    let d0 = &west["daily"][0];
    assert_eq!(d0["sun_h"], 8.0);
    assert_eq!(d0["cls"], "S");
    assert_eq!(d0["rain"], 10);
    assert_eq!(d0["wind"], 10);
    assert_eq!(d0["wdir"], 250);
    assert_eq!(d0["tmax"], 22);
    assert_eq!(d0["tmin"], 10);
    assert!(d0["sunset"].as_str().unwrap().starts_with("16:"));
    assert_eq!(west["daily"][6]["sun_h"], 8.0);
    // Grid at 10:00: every in-region cell is '9', the gap column is '.'.
    assert_eq!(&g.sun[10][0..g.nx], "99999.9");
    assert_eq!(&g.sun[2][0..g.nx], "NNNNN.N");
}
