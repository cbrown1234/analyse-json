# analyse-json
CLI tool to analyse Newline Delimited (ND)JSON files and return useful metadata
about the structure to help you understand the contents

```
Usage: analyse-json [OPTIONS] [FILE_PATH]

Arguments:
  [FILE_PATH]  File to process, expected to contain a single JSON object or Newline Delimited (ND) JSON objects

Options:
  -g, --glob <GLOB>                   Process all files identified by this glob pattern
  -n, --lines <LINES>                 Limit inspection to the first n lines
      --jsonpath <JSONPATH>           JSONpath query to filter/limit the inspection to e.g. `'$.a_key.an_array[0]'`
      --inspect-arrays                Walk the elements of arrays grouping elements paths together under `$.path.to.array[*]`? See also `--explode-arrays`
      --explode-arrays                Walk the elements of arrays treating arrays like a map of their enumerated elements? (E.g. $.path.to.array[0], $.path.to.array[1], ...) See also `--inspect-arrays`
      --merge                         Include combined results for all files when using glob
      --parallel                      Use multi-threaded version of the processing
  -q, --quiet                         Silence error logging
      --generate-completions <SHELL>  Output shell completions for the chosen shell to stdout [possible values: bash, elvish, fish, powershell, zsh]
  -h, --help                          Print help
  -V, --version                       Print version
```

## Installation
### via cargo
#### Prerequisites
You need to have cargo installed
https://doc.rust-lang.org/cargo/getting-started/installation.html
#### Install

```shell
cargo install analyse-json
```

### Prebuild binaries

[Prebuild binaries for some systems can be found on github](https://github.com/cbrown1234/analyse-json/releases)

## Changelog

[Changelog is available on github](https://github.com/cbrown1234/analyse-json/blob/master/CHANGELOG.md)
