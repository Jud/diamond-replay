use diamond_replay::replay_from_json;
use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: replay_stats <event_file.json> [event_file2.json ...]");
        process::exit(1);
    }

    let mut results = Vec::new();
    for path in &args {
        let data = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error reading {path}: {e}");
            process::exit(1);
        });
        match replay_from_json(&data) {
            Ok(result) => {
                let name = std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path)
                    .to_string();
                results.push(serde_json::json!({
                    "name": name,
                    "home_id": result.home_id,
                    "away_id": result.away_id,
                    "linescore_home": result.linescore_home,
                    "linescore_away": result.linescore_away,
                    "home_batting": result.home_batting,
                    "away_batting": result.away_batting,
                    "home_halves_bat": result.home_halves_bat,
                    "away_halves_bat": result.away_halves_bat,
                    "first_timestamp": result.first_timestamp,
                    "last_timestamp": result.last_timestamp,
                    "duration_min": result.first_timestamp.zip(result.last_timestamp)
                        .map(|(f, l)| f64::from(i32::try_from(l - f).unwrap_or(i32::MAX)) / 60_000.0),
                }));
            }
            Err(e) => {
                eprintln!("Error replaying {path}: {e}");
                process::exit(1);
            }
        }
    }

    println!("{}", serde_json::to_string_pretty(&results).unwrap());
}
