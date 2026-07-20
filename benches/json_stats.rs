//! Benchmarks for the end-to-end NDJSON statistics pipeline.
//!
//! These exercise [`JSONStats::json_stats`] over generated NDJSON files, comparing:
//! - the single-threaded and `--parallel` (rayon) code paths across input sizes, and
//! - the cost of applying a `--jsonpath` filter.
//!
//! Run with `cargo bench`. All cases pass `--quiet` so the progress spinner is hidden
//! and the benchmark measures the actual processing rather than terminal I/O.
//!
//! The non-quiet (spinner) path is deliberately not benchmarked: indicatif suppresses
//! the spinner when stderr is not a terminal (so a redirected run measures nothing),
//! and when attached to a terminal the spinner competes with criterion's own progress
//! output — producing garbled, non-deterministic timings. Measuring spinner overhead
//! cleanly would require the code to accept an injectable draw target (e.g. an
//! in-memory terminal).

use std::io::Write;
use std::path::Path;

use analyse_json::json::ndjson::JSONStats;
use analyse_json::{Cli, Settings};
use clap::Parser;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use tempfile::NamedTempFile;

/// Write `records` lines of representative NDJSON (scalars, a nested object and an
/// array) to a temp file and return the handle, keeping the file alive for the bench.
fn make_ndjson(records: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("create temp file");
    for i in 0..records {
        writeln!(
            file,
            r#"{{"key1": "value{i}", "key2": {i}, "nested": {{"a": [1, 2, 3], "b": "x"}}}}"#
        )
        .expect("write record");
    }
    file.flush().expect("flush temp file");
    file
}

/// Build [`Settings`] as the CLI would for the given file plus any extra flags
/// (e.g. `--quiet`, `--parallel`, `--jsonpath <query>`).
fn settings(path: &Path, extra_args: &[&str]) -> Settings {
    let path_str = path.to_str().expect("temp path is valid UTF-8");
    let mut argv = vec!["analyse-json", path_str];
    argv.extend_from_slice(extra_args);
    let cli = Cli::parse_from(argv);
    Settings::init(cli).expect("initialise settings")
}

fn bench_serial_vs_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_stats/serial_vs_parallel");
    for &records in &[1_000usize, 10_000] {
        let file = make_ndjson(records);
        let path = file.path().to_path_buf();
        group.throughput(Throughput::Elements(records as u64));

        let serial = settings(&path, &["--quiet"]);
        group.bench_with_input(BenchmarkId::new("serial", records), &path, |b, path| {
            b.iter(|| path.json_stats(&serial).expect("stats"));
        });

        let parallel = settings(&path, &["--quiet", "--parallel"]);
        group.bench_with_input(BenchmarkId::new("parallel", records), &path, |b, path| {
            b.iter(|| path.json_stats(&parallel).expect("stats"));
        });
    }
    group.finish();
}

fn bench_jsonpath(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_stats/jsonpath");
    let records = 10_000usize;
    let file = make_ndjson(records);
    let path = file.path().to_path_buf();
    group.throughput(Throughput::Elements(records as u64));

    let no_query = settings(&path, &["--quiet"]);
    group.bench_with_input(BenchmarkId::new("no_query", records), &path, |b, path| {
        b.iter(|| path.json_stats(&no_query).expect("stats"));
    });

    let query = settings(&path, &["--quiet", "--jsonpath", "$.nested.a[*]"]);
    group.bench_with_input(BenchmarkId::new("query", records), &path, |b, path| {
        b.iter(|| path.json_stats(&query).expect("stats"));
    });

    group.finish();
}

criterion_group!(benches, bench_serial_vs_parallel, bench_jsonpath);
criterion_main!(benches);
