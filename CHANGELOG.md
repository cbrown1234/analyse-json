# Changelog

All notable changes to this project will be documented in this file.

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Changed
- Update README

## [0.6.0] - 2024-01-20

### Changed
- Switched to `serde_json_path` for JSON path querying
- Added anyhow for error handling
- Enforce mutually exclusive array handling options
- Updated to clap v4
- Updated dependencies

### Fixed
- Typo in error message

## [0.5.5] - 2023-07-12

### Added
- More documentation to the lib

### Fixed
- Previously `--merge` did not include invalid lines info

## [0.5.4] - 2023-06-20

## [0.5.3] - 2023-06-20

### Added
- Progress spinner showing lines parsed

## [0.5.2] - 2023-01-10

### Changed
- Improved documentation

## [0.5.1] - 2023-01-09

### Added
- `--generate-completions` flag to generate shell completion scripts

## [0.5.0] - 2023-01-04

### Added
- `--inspect-arrays` flag for alternative (less verbose than `--explode-arrays`) way of returning paths inside arrays

### Changed
- Added additional newlines to make output slightly easier to read

## [0.4.1] - 2022-12-16

### Changed

- Update documentation

## [0.4.0] - 2022-12-15

### Added
- `--parallel` flag to toggle on parallel processing
- Full support for multiple return values from `--jsonpath` query
- `--quiet` flag to silence errors

### Changed
- Refactored use of iterators
- Improved error handing within iterators
- Improved error logging
- Line numbers for file now start at 1

### Fixed
- stdout to non-tty (pretty) printing json

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
[Unreleased]: https://github.com/cbrown1234/analyse-json/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/cbrown1234/analyse-json/compare/v0.5.5...v0.6.0
[0.5.5]: https://github.com/cbrown1234/analyse-json/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/cbrown1234/analyse-json/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/cbrown1234/analyse-json/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/cbrown1234/analyse-json/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/cbrown1234/analyse-json/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/cbrown1234/analyse-json/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/cbrown1234/analyse-json/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/cbrown1234/analyse-json/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/cbrown1234/analyse-json/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/cbrown1234/analyse-json/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/cbrown1234/analyse-json/releases/tag/v0.1.0
