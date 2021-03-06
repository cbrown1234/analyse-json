# Changelog

All notable changes to this project will be documented in this file.

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.3.0] - 2022-07-02

### Added
- Aggregated stats when using glob via `--merged`
- This changelog

### Changed
- Improved tty contextual behaviour for stdin & stdout
  - (Non-tty stdout) Coloured output
  - (tty stdout) JSON output format
  - (tty stdin) Fixed ability to run with no options
- Refactored path walking functionality in lib
- Improved README and other docs
  - Installation instructions

## [0.2.0] - 2022-05-24

### Added
- Option (`--explode-arrays`) to walk arrays
- pre-commit for some dev automation

### Changed
- Improved Readme
- Refactored JSON walking

### Fixed
- `--jsonpath` filtering for files (previously only worked for stdin)

## [0.1.0] - 2022-05-15

Initial Release

<!-- next-url -->
[Unreleased]: https://github.com/cbrown1234/analyse-json/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/cbrown1234/analyse-json/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/cbrown1234/analyse-json/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/cbrown1234/analyse-json/releases/tag/v0.1.0
