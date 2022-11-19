# analyse-json
CLI tool to analyse Newline Delimited (ND)JSON files and return useful metadata
about the structure to help you understand the contents

```
USAGE:
    analyse-json [OPTIONS] [FILE_PATH]

ARGS:
    <FILE_PATH>

OPTIONS:
        --explode-arrays         Walk the elements of arrays?
    -g, --glob <GLOB>            Process all files identified by this glob pattern
    -h, --help                   Print help information
        --jsonpath <JSONPATH>    JSONpath query to filter/limit the inspection to
        --merge                  Include combined results for all files when using glob
    -n, --lines <LINES>          Limit inspection to the first n lines
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

## Features TODO:
* ~~Add multi-file handling (dir/glob)~~
* ~~Add JSON path search~~ (Can be improved though)
* ~~Add parallelism to improve performance~~
* Statistical JSON schema inference
* Co-occurrence of fields matrix
* ~~"Addition" for file stats (Enables multi-file aggregation of stats)~~
* ~~Better terminal output~~
  * ~~Coloured~~
  * ~~tty dependent behaviour~~
* Switch to a better JSON query implementation, considering:
  * Consider for the query language
    * https://github.com/freestrings/jsonpath
    * https://github.com/yamafaktory/jql
    * https://github.com/jmespath/jmespath.rs
    * https://github.com/onelson/jq-rs
  * Wrapper to keep track of source lines for 0/1 to many queries?
* Refactor to implement using iterators
