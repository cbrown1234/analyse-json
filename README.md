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

## Features TODO:
* ~~Add multi-file handling (dir/glob)~~
* ~~Add JSON path search~~ (Can be improved though)
* ~~Add parallelism to improve performance~~ (Removed when refactoring as iterator adapters, will reconsider later)
* Statistical JSON schema inference
* Co-occurrence of fields matrix
* ~~"Addition" for file stats (Enables multi-file aggregation of stats)~~
* ~~Better terminal output~~
  * ~~Coloured~~
  * ~~tty dependent behaviour~~
* ~~Switch to a better JSON query implementation, considering:~~
  * ~~Consider for the query language~~
    * ~~https://github.com/freestrings/jsonpath~~ (using this for now)
    * ~~https://github.com/yamafaktory/jql~~
    * ~~https://github.com/jmespath/jmespath.rs~~
    * ~~https://github.com/onelson/jq-rs~~
  * ~~Wrapper to keep track of source lines for 0/1 to many queries?~~
* ~~Refactor to implementation using iterator adapters~~ (First pass done, need some improvement)
  * consider itertools::process_results as a top level runner
* Check for existance (and type) of particular jsonpath
* ~~Enumerate lines from 1 rather than 0 (Maybe part of larger json wrapper struct refactor)~~
* ~~Refactor ndjson to use error wrappers with location info rather than tuples~~
* ~~Integrate graceful iterator adapter error handling (`Rc<RefCell<Vec<_>>>`) into filestats~~
