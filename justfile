

# Test release to crates.io
release-dryrun LEVEL:
  cargo release {{LEVEL}}

# Release to crates.io
release-real LEVEL:
  cargo release {{LEVEL}} -x
