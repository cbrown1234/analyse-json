

# Test release to crates.io
release-dryrun LEVEL:
  cargo release {{LEVEL}}

# Release to crates.io
release-real LEVEL:
  cargo release {{LEVEL}} -x

build:
  # Required sudo apt-get install mingw-w64
  rustup target add x86_64-pc-windows-gnu
  cargo build --release --target x86_64-pc-windows-gnu
  cargo build --release

generate-test-data:
  mkdir -p test_data_large
  for i in $(seq 100000); do cat test_data/some_data_3.json >> test_data_large/some_data_3.json; done
