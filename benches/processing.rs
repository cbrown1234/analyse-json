use std::path::PathBuf;

use analyse_json::json::ndjson::JSONStats;
use analyse_json::{Cli, Settings};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_processing(c: &mut Criterion) {
    let path = PathBuf::from("test_data/realistic.ndjson");
    if !path.exists() {
        eprintln!("Skipping benchmarks: test_data/realistic.ndjson not found");
        return;
    }

    let line_count = std::fs::read_to_string(&path).unwrap().lines().count() as u64;

    let mut group = c.benchmark_group("json_stats");
    group.throughput(Throughput::Elements(line_count));

    for (name, parallel) in [("sequential", false), ("parallel", true)] {
        group.bench_with_input(BenchmarkId::new(name, "realistic"), &parallel, |b, &par| {
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

criterion_group!(benches, bench_processing);
criterion_main!(benches);
