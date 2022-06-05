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

Features TODO:
* ~~Add multi-file handling (dir/glob)~~
* ~~Add JSON path search~~ (Can be improved though)
* ~~Add parallelism to improve performance~~
* Statistical JSON schema inferance
* Co-occurance of fields matrix
* ~~"Addition" for file stats (Enables multi-file aggregation of stats)~~
