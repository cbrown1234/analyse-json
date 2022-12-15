# analyse-json
CLI tool to analyse Newline Delimited (ND)JSON files and return useful metadata
about the structure to help you understand the contents

```
USAGE:
    analyse-json [OPTIONS] [FILE_PATH]

ARGS:
    <FILE_PATH>    File to process, expected to contain a single JSON object or Newline
                   Delimited (ND) JSON objects

OPTIONS:
        --explode-arrays         Walk the elements of arrays?
    -g, --glob <GLOB>            Process all files identified by this glob pattern
    -h, --help                   Print help information
        --jsonpath <JSONPATH>    JSONpath query to filter/limit the inspection to
        --merge                  Include combined results for all files when using glob
    -n, --lines <LINES>          Limit inspection to the first n lines
        --parallel               Use parallel version of the processing
    -q, --quiet                  Silence error logging
    -V, --version                Print version information
```

## Installation
### via cargo
#### Prerequisites
You need to have cargo installed
https://doc.rust-lang.org/cargo/getting-started/installation.html
#### Install
```
cargo install analyse-json
```

## Changelog

[Changelog is available on github](https://github.com/cbrown1234/analyse-json/blob/master/CHANGELOG.md)
