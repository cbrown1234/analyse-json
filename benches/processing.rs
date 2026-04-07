use std::path::PathBuf;

use analyse_json::json::ndjson::JSONStats;
use analyse_json::{Cli, Settings};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_file(c: &mut Criterion, path: &PathBuf, name: &str) {
    if !path.exists() {
        eprintln!("Skipping benchmark {name}: {} not found", path.display());
        return;
    }

    let line_count = std::fs::read_to_string(path).unwrap().lines().count() as u64;

    let mut group = c.benchmark_group("json_stats");
    group.throughput(Throughput::Elements(line_count));

    for (mode, parallel) in [("sequential", false), ("parallel", true)] {
        group.bench_with_input(BenchmarkId::new(mode, name), &parallel, |b, &par| {
            b.iter(|| {
                let mut args = Cli::default();
                args.parallel = par;
                let settings = Settings::init(args).unwrap();
                path.json_stats(&settings).unwrap()
            });
        });
    }
    group.finish();
}

fn bench_processing(c: &mut Criterion) {
    bench_file(c, &PathBuf::from("test_data/realistic.ndjson"), "realistic");
    let large = std::env::var("BENCH_LARGE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("test_data_large/some_data_3.json"));
    bench_file(c, &large, "large");
}

criterion_group!(benches, bench_processing);
criterion_main!(benches);
