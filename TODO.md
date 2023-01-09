## Features TODO:
* ~~Add multi-file handling (dir/glob)~~
* ~~Add JSON path search~~ (Can be improved though)
* ~~Add parallelism to improve performance~~ ~~(Removed when refactoring as iterator adapters, will reconsider later)~~
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
* ~~Shell completion support~~
 * ~~Runtime creation~~
 * Build time creating