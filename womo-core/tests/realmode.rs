//! Real-mode pipeline against a local fake Open-Meteo server (batching, parsing, aggregation).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use womo_core::cli::build_outputs;
use womo_core::openmeteo::Client;
use womo_core::regions::{build_points, Regions};
use womo_core::time::TimeAxis;

/// Serves the first location of the fixture once per requested coordinate.
fn spawn_server(requests: Arc<AtomicUsize>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/openmeteo_multi.json")).unwrap();
    let one = fixture[0].clone();
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
    assert_eq!(&cls[0..2], "NN");
    assert_eq!(&cls[10..11], "S");
    assert_eq!(west["hourly"]["sun"][10], 100);
    assert_eq!(west["hourly"]["sunny"][10], 100);
    assert_eq!(west["daily"][0]["sunset"], "16:59");
    assert_eq!(west["daily"][0]["sun_h"], 6.7);
    assert_eq!(west["daily"][0]["cls"], "S");
    // Grid: every in-region cell is '9' at 10:00, the gap column is '.'.
    let row0 = &g.sun[10][0..g.nx];
    assert_eq!(row0, "99999.9");
}
