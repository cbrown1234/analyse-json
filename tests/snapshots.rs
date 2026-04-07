use std::path::PathBuf;

use analyse_json::json::ndjson::{FileStats, JSONStats, Stats};
use analyse_json::{Cli, Settings};

fn settings_default() -> Settings {
    Settings::init(Cli::default()).unwrap()
}

fn settings_with(f: impl Fn(&mut Cli)) -> Settings {
    let mut args = Cli::default();
    f(&mut args);
    Settings::init(args).unwrap()
}

fn realistic_path() -> PathBuf {
    PathBuf::from("test_data/realistic.ndjson")
}

fn arrays_path() -> PathBuf {
    PathBuf::from("test_data/arrays.ndjson")
}

#[test]
fn snapshot_realistic_sequential() {
    let settings = settings_default();
    let mut stats = realistic_path().json_stats(&settings).unwrap();
    stats.bad_lines.sort();
    stats.empty_lines.sort();
    insta::assert_json_snapshot!(stats);
}

#[test]
fn snapshot_realistic_parallel() {
    let settings = settings_with(|a| a.parallel = true);
    let mut stats = realistic_path().json_stats(&settings).unwrap();
    stats.bad_lines.sort();
    stats.empty_lines.sort();
    // DashMap has non-deterministic order; sort maps for stable snapshots
    let mut keys_count: Vec<_> = stats.keys_count.into_iter().collect();
    keys_count.sort_by(|a, b| a.0.cmp(&b.0));
    stats.keys_count = keys_count.into_iter().collect();
    let mut keys_types_count: Vec<_> = stats.keys_types_count.into_iter().collect();
    keys_types_count.sort_by(|a, b| a.0.cmp(&b.0));
    stats.keys_types_count = keys_types_count.into_iter().collect();
    insta::assert_json_snapshot!(stats);
}

#[test]
fn snapshot_realistic_display() {
    let settings = settings_default();
    let mut stats = realistic_path().json_stats(&settings).unwrap();
    stats.bad_lines.sort();
    stats.empty_lines.sort();
    insta::assert_snapshot!(stats.to_string());
}

#[test]
fn snapshot_arrays_inspect() {
    let settings = settings_with(|a| a.inspect_arrays = true);
    let stats = arrays_path().json_stats(&settings).unwrap();
    insta::assert_json_snapshot!(stats);
}

#[test]
fn snapshot_arrays_explode() {
    let settings = settings_with(|a| a.explode_arrays = true);
    let stats = arrays_path().json_stats(&settings).unwrap();
    insta::assert_json_snapshot!(stats);
}

#[test]
fn snapshot_realistic_jsonpath() {
    let settings = settings_with(|a| a.jsonpath = Some("$.address".to_string()));
    let mut stats = realistic_path().json_stats(&settings).unwrap();
    stats.bad_lines.sort();
    stats.empty_lines.sort();
    insta::assert_json_snapshot!(stats);
}

#[test]
fn snapshot_stats_merge() {
    let settings = settings_default();
    let stats1 = FileStats::new(
        "file1.ndjson".to_string(),
        realistic_path().json_stats(&settings).unwrap(),
    );
    let stats2 = FileStats::new(
        "file2.ndjson".to_string(),
        arrays_path().json_stats(&settings).unwrap(),
    );
    let mut merged: Stats = stats1 + stats2;
    merged.bad_lines.sort();
    merged.empty_lines.sort();
    insta::assert_json_snapshot!(merged);
}
