use crate::json::paths::ValuePaths;
use crate::json::ValueType;
use crate::{get_bufreader, Cli, Settings};

use super::IndexMap;
use dashmap::DashMap;
use grep_cli::is_tty_stdout;
use owo_colors::{OwoColorize, Stream};
use rayon::iter::ParallelBridge;
use rayon::prelude::ParallelIterator;
use serde::{Deserialize, Serialize};
pub use serde_json::Value;
use std::cell::RefCell;
use std::error::{self, Error};
use std::fs::File;
use std::iter::{Enumerate, Sum};
use std::ops::Add;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::{
    fmt,
    io::{self, prelude::*},
};

type IdJSON = (String, Value);
type IdJSONIter<'a> = Box<dyn Iterator<Item = IdJSON> + 'a>;
type IdErr = (String, Box<dyn error::Error>);

pub fn parse_ndjson_bufreader<'a>(
    _args: &Cli,
    reader: impl BufRead + 'a,
    errors: &Rc<RefCell<Vec<IdErr>>>,
) -> Result<IdJSONIter<'a>, Box<dyn Error>> {
    let json_iter = reader.lines();

    let json_iter = json_iter.to_enumerated_err_filtered(Rc::clone(errors));

    let json_iter = json_iter.map(|(i, json_candidate)| {
        (
            i.to_string(),
            serde_json::from_str::<Value>(&json_candidate),
        )
    });
    let json_iter = json_iter.to_err_filtered(Rc::clone(errors));

    Ok(Box::new(json_iter))
}

pub fn parse_ndjson_file<'a>(
    args: &Cli,
    file: File,
    errors: &Rc<RefCell<Vec<IdErr>>>,
) -> Result<IdJSONIter<'a>, Box<dyn Error>> {
    let reader = io::BufReader::new(file);
    parse_ndjson_bufreader(args, reader, errors)
}

pub fn parse_ndjson_file_path<'a>(
    args: &Cli,
    file_path: &PathBuf,
    errors: &Rc<RefCell<Vec<IdErr>>>,
) -> Result<IdJSONIter<'a>, Box<dyn Error>> {
    let reader = get_bufreader(args, file_path)?;
    parse_ndjson_bufreader(args, reader, errors)
}

pub struct ErrFiltered<I> {
    iter: I,
    errors: Rc<RefCell<Vec<IdErr>>>,
}

impl<E: 'static + Error, T, I: Iterator<Item = (String, Result<T, E>)>> ErrFiltered<I> {
    pub fn new(iter: I, errors: Rc<RefCell<Vec<IdErr>>>) -> Self {
        Self { iter, errors }
    }
}

impl<E: 'static + Error, T, I> Iterator for ErrFiltered<I>
where
    I: Iterator<Item = (String, Result<T, E>)>,
{
    type Item = (String, T);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, next_item) = self.iter.next()?;
            match next_item {
                Ok(item) => break Some((id, item)),
                Err(e) => {
                    self.errors.borrow_mut().push((id, Box::new(e)));
                }
            }
        }
    }
}

pub trait IntoErrFiltered<E: 'static + Error, T>:
    Iterator<Item = (String, Result<T, E>)> + Sized
{
    fn to_err_filtered(self, errors: Rc<RefCell<Vec<IdErr>>>) -> ErrFiltered<Self> {
        ErrFiltered::new(self, errors)
    }
}

impl<E: 'static + Error, T, I: Iterator<Item = (String, Result<T, E>)>> IntoErrFiltered<E, T>
    for I
{
}

pub struct EnumeratedErrFiltered<I> {
    iter: Enumerate<I>,
    errors: Rc<RefCell<Vec<IdErr>>>,
}

impl<E: 'static + Error, T, I: Iterator<Item = Result<T, E>>> EnumeratedErrFiltered<I> {
    pub fn new(iter: I, errors: Rc<RefCell<Vec<IdErr>>>) -> Self {
        Self {
            iter: iter.enumerate(),
            errors,
        }
    }
}

impl<E: 'static + Error, T, I> Iterator for EnumeratedErrFiltered<I>
where
    I: Iterator<Item = Result<T, E>>,
{
    type Item = (usize, T);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (i, next_item) = self.iter.next()?;
            match next_item {
                Ok(item) => break Some((i, item)),
                Err(e) => {
                    self.errors.borrow_mut().push((i.to_string(), Box::new(e)));
                }
            }
        }
    }
}

pub trait IntoEnumeratedErrFiltered<E: 'static + Error, T>:
    Iterator<Item = Result<T, E>> + Sized
{
    fn to_enumerated_err_filtered(
        self,
        errors: Rc<RefCell<Vec<IdErr>>>,
    ) -> EnumeratedErrFiltered<Self> {
        EnumeratedErrFiltered::new(self, errors)
    }
}

impl<E: 'static + Error, T, I: Iterator<Item = Result<T, E>>> IntoEnumeratedErrFiltered<E, T>
    for I
{
}

// TODO: extract stats to separate struct or add "file" id to *_lines
#[derive(Debug, PartialEq, Eq, Default, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub keys_count: IndexMap<String, usize>,
    pub line_count: usize,
    pub bad_lines: Vec<usize>,
    pub keys_types_count: IndexMap<String, usize>,
    pub empty_lines: Vec<usize>,
}

impl FileStats {
    pub fn new() -> FileStats {
        FileStats {
            keys_count: IndexMap::new(),
            line_count: 0,
            bad_lines: Vec::new(),
            keys_types_count: IndexMap::new(),
            empty_lines: Vec::new(),
        }
    }

    pub fn key_occurance(&self) -> IndexMap<String, f64> {
        self.keys_count
            .iter()
            .map(|(k, v)| (k.to_owned(), 100f64 * *v as f64 / self.line_count as f64))
            .collect()
    }

    pub fn key_type_occurance(&self) -> IndexMap<String, f64> {
        self.keys_types_count
            .iter()
            .map(|(k, v)| (k.to_owned(), 100f64 * *v as f64 / self.line_count as f64))
            .collect()
    }
}

impl fmt::Display for FileStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stream = Stream::Stdout;
        writeln!(f, "Keys:\n{:#?}\n", self.keys_count.keys())?;
        writeln!(f, "Key occurance counts:\n{:#?}", self.keys_count)?;
        writeln!(f, "Key occurance rate:")?;
        for (k, v) in self.key_occurance() {
            writeln!(f, "{}: {}%", k, v)?;
        }
        writeln!(f, "Key type occurance rate:")?;
        for (k, v) in self.key_type_occurance() {
            writeln!(f, "{}: {}%", k, v)?;
        }
        if !self.bad_lines.is_empty() {
            writeln!(
                f,
                "{}\n{:?}",
                "Corrupted lines:".if_supports_color(stream, |text| text.red()),
                self.bad_lines.if_supports_color(stream, |text| text.red())
            )?;
        }
        if !self.empty_lines.is_empty() {
            writeln!(
                f,
                "{}\n{:?}",
                "Empty lines:".if_supports_color(stream, |text| text.red()),
                self.empty_lines
                    .if_supports_color(stream, |text| text.red())
            )?;
        }
        Ok(())
    }
}

impl FileStats {
    pub fn print(&self) -> std::result::Result<(), serde_json::Error> {
        if is_tty_stdout() {
            println!("{}", self);
            Ok(())
        } else {
            let json_out = serde_json::to_string_pretty(self)?;
            println!("{}", json_out);
            Ok(())
        }
    }
}

impl Add for FileStats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut output = self;

        for (k, v) in rhs.keys_count {
            let counter = output.keys_count.entry(k).or_insert(0);
            *counter += v
        }

        for (k, v) in rhs.keys_types_count {
            let counter = output.keys_types_count.entry(k).or_insert(0);
            *counter += v
        }

        output.line_count += rhs.line_count;

        // Not sure these are compatible
        output.bad_lines = Vec::new();
        output.empty_lines = Vec::new();

        output
    }
}

impl Add<&Self> for FileStats {
    type Output = Self;

    fn add(self, rhs: &Self) -> Self::Output {
        self.add(rhs.clone())
    }
}

impl<'a> Sum<&'a Self> for FileStats {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |acc, x| acc + x)
    }
}

// https://stackoverflow.com/questions/26368288/how-do-i-stop-iteration-and-return-an-error-when-iteratormap-returns-a-result
fn until_err<T, E>(err: &mut &mut Result<(), E>, item: Result<T, E>) -> Option<T> {
    match item {
        Ok(item) => Some(item),
        Err(e) => {
            **err = Err(e);
            None
        }
    }
}

#[derive(Debug, Clone)]
struct EmptyQueryResultError;

impl fmt::Display for EmptyQueryResultError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "query returned no results")
    }
}

impl Error for EmptyQueryResultError {}

pub fn expand_jsonpath_query<'a>(
    settings: &'a Settings,
    json_iter: impl Iterator<Item = IdJSON> + 'a,
    errors: &Rc<RefCell<Vec<IdErr>>>,
) -> IdJSONIter<'a> {
    let select_errors = Rc::clone(errors);
    let missing = Rc::clone(errors);
    let json_iter_out: IdJSONIter<'a>;
    if let Some(ref selector) = settings.jsonpath_selector {
        let path = settings.args.jsonpath.to_owned();
        let path = path.expect("must exist for jsonpath_selector to exist");
        let expanded = json_iter.flat_map(move |(ref id, ref json)| {
            let mut select_errored = false;
            let selected = selector.select(json).unwrap_or_else(|e| {
                select_errors
                    .borrow_mut()
                    .push((id.to_owned(), Box::new(e)));
                select_errored = true;
                vec![]
            });
            if selected.is_empty() && !select_errored {
                missing
                    .borrow_mut()
                    .push((id.to_owned(), Box::new(EmptyQueryResultError)))
            }
            selected
                .into_iter()
                .map(|json| (format!("{id}:{path}"), json.to_owned()))
                .enumerate()
                .map(|(i, (id, json))| (format!("{id}[{i}]"), json))
                .collect::<Vec<_>>()
        });
        json_iter_out = Box::new(expanded);
    } else {
        json_iter_out = Box::new(json_iter);
    }
    json_iter_out
}

pub fn apply_settings<'a>(
    settings: &'a Settings,
    json_iter: impl Iterator<Item = IdJSON> + 'a,
    errors: &Rc<RefCell<Vec<IdErr>>>,
) -> IdJSONIter<'a> {
    let args = &settings.args;

    let json_iter = limit(args, json_iter);
    expand_jsonpath_query(settings, json_iter, errors)
}

pub fn process_json_iterable(
    settings: &Settings,
    json_iter: impl Iterator<Item = IdJSON>,
    errors: &Rc<RefCell<Vec<IdErr>>>,
) -> FileStats {
    let mut fs = FileStats::new();
    let args = &settings.args;

    let json_iter = apply_settings(settings, json_iter, errors);

    for (_id, json) in json_iter {
        fs.line_count += 1;

        for value_path in json.value_paths(args.explode_arrays) {
            let path = value_path.jsonpath();
            let counter = fs.keys_count.entry(path.to_owned()).or_insert(0);
            *counter += 1;

            let type_ = value_path.value.value_type();
            let path_type = format!("{}::{}", path, type_);
            let counter = fs.keys_types_count.entry(path_type).or_insert(0);
            *counter += 1;
        }
    }
    fs
}

#[deprecated]
pub fn parse_json_iterable_par<E>(
    args: &Cli,
    json_iter: impl Iterator<Item = Result<String, E>> + Send,
) -> Result<FileStats, Box<dyn error::Error>>
where
    E: 'static + Error + Send,
{
    let keys_count: DashMap<String, usize> = DashMap::new();
    let keys_types_count: DashMap<String, usize> = DashMap::new();
    let mut bad_lines: Vec<usize> = Vec::new();
    let bad_lines_mutex = Mutex::new(&mut bad_lines);
    let line_count = AtomicUsize::new(0);
    let mut empty_lines: Vec<usize> = Vec::new();
    let empty_lines_mutex = Mutex::new(&mut empty_lines);

    let json_iter = parse_iter(args, json_iter);
    let jsonpath = args.jsonpath_selector()?;

    // Bubble up upstream errors
    let mut err = Ok(());
    let json_iter = json_iter.scan(&mut err, until_err);

    json_iter
        .enumerate()
        .par_bridge()
        .map(|(i, json_candidate)| (i, serde_json::from_str(&json_candidate)))
        .inspect(|(i, j): &(usize, Result<Value, serde_json::Error>)| {
            let line_num = i + 1;
            if j.is_err() {
                let mut bad_lines = bad_lines_mutex.lock().unwrap();
                bad_lines.push(line_num);
            }
            line_count.fetch_max(line_num, Ordering::Release);
        })
        .filter(|(_i, j)| j.is_ok())
        .map(|(i, j)| (i, j.unwrap()))
        .for_each(|(i, mut json)| {
            let mut continue_ = false;
            if let Some(ref selector) = jsonpath {
                let json_list = selector.select(&json).expect("Failed to parse json");
                let mut json_list = json_list.iter();
                if let Some(&json_1) = json_list.next() {
                    // TODO: handle multiple search results
                    assert_eq!(None, json_list.next());
                    json = json_1.to_owned()
                } else {
                    let line_num = i + 1;
                    let mut empty_lines = empty_lines_mutex.lock().unwrap();
                    empty_lines.push(line_num);
                    // continue; doesn't work in for_each
                    continue_ = true;
                }
            }
            if !continue_ {
                for value_path in json.value_paths(args.explode_arrays) {
                    let path = value_path.jsonpath();
                    let mut counter = keys_count.entry(path.to_owned()).or_insert(0);
                    *counter.value_mut() += 1;

                    let type_ = value_path.value.value_type();
                    let path_type = format!("{}::{}", path, type_);
                    let mut counter = keys_types_count.entry(path_type).or_insert(0);
                    *counter.value_mut() += 1;
                }
            }
        });

    err?;

    let fs = FileStats {
        keys_count: keys_count
            .into_read_only()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        line_count: line_count.load(Ordering::Acquire),
        keys_types_count: keys_types_count
            .into_read_only()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        bad_lines,
        empty_lines,
    };
    Ok(fs)
}

// // TODO: impliment method to handle
// trait Stats {
//     fn stats(&self) -> FileStats;
// }

// impl<T: impl Iterator<Item = Result<String, E>> + Send> Stats for T {
//     fn stats(&self) {
//         parse_json_iterable_par(&self)
//     }
// }

pub fn limit<'a, I, T>(args: &Cli, iter: I) -> Box<dyn Iterator<Item = T> + 'a>
where
    I: Iterator<Item = T> + 'a,
{
    if let Some(n) = args.lines {
        Box::new(iter.take(n))
    } else {
        Box::new(iter)
    }
}

pub fn parse_iter<E, I>(args: &Cli, iter: I) -> impl Iterator<Item = Result<String, E>>
where
    I: Iterator<Item = Result<String, E>>,
{
    if let Some(n) = args.lines {
        iter.take(n)
    } else {
        iter.take(usize::MAX)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn simple_ndjson() {
        let mut tmpfile: File = tempfile::tempfile().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key2": 123}}"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let reader = io::BufReader::new(tmpfile);

        let expected: Vec<IdJSON> = vec![
            (0.to_string(), json!({"key1": 123})),
            (1.to_string(), json!({"key2": 123})),
            (2.to_string(), json!({"key1": 123})),
        ];

        let args = Cli::default();
        let errors = Rc::new(RefCell::new(vec![]));

        let json_iter = parse_ndjson_bufreader(&args, reader, &errors).unwrap();
        assert_eq!(expected, json_iter.collect::<Vec<IdJSON>>());
        assert!(errors.borrow().is_empty())
    }

    #[test]
    fn bad_ndjson_file() {
        let mut tmpfile: File = tempfile::tempfile().unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        writeln!(tmpfile, r#"not valid json"#).unwrap();
        writeln!(tmpfile, r#"{{"key1": 123}}"#).unwrap();
        tmpfile.seek(SeekFrom::Start(0)).unwrap();
        let reader = io::BufReader::new(tmpfile);

        let expected: Vec<IdJSON> = vec![
            (0.to_string(), json!({"key1": 123})),
            (2.to_string(), json!({"key1": 123})),
        ];

        let args = Cli::default();
        let errors = Rc::new(RefCell::new(vec![]));

        let json_iter = parse_ndjson_bufreader(&args, reader, &errors).unwrap();
        assert_eq!(expected, json_iter.collect::<Vec<IdJSON>>());
        assert!(errors.borrow().len() == 1)
    }

    #[test]
    fn simple_expand_jsonpath_query() {
        let json_iter_in: Vec<IdJSON> = vec![
            (0.to_string(), json!({"key1": [1, 2, 3]})),
            (1.to_string(), json!({"key2": 123})),
            (2.to_string(), json!({"key1": [4, 5]})),
        ];
        let json_iter_in = json_iter_in.iter().cloned();

        let mut args = Cli::default();
        args.jsonpath = Some("$.key1[*]".to_string());
        let settings = Settings::init(args).unwrap();
        let errors = Rc::new(RefCell::new(vec![]));

        let expected: Vec<IdJSON> = vec![
            ("0:$.key1[*][0]".to_string(), json!(1)),
            ("0:$.key1[*][1]".to_string(), json!(2)),
            ("0:$.key1[*][2]".to_string(), json!(3)),
            ("2:$.key1[*][0]".to_string(), json!(4)),
            ("2:$.key1[*][1]".to_string(), json!(5)),
        ];

        let json_iter = expand_jsonpath_query(&settings, json_iter_in, &errors);
        assert_eq!(expected, json_iter.collect::<Vec<IdJSON>>());
        assert!(errors.borrow().len() == 1)
    }

    #[test]
    fn simple_process_json_iterable() {
        let json_iter_in: Vec<IdJSON> = vec![
            (0.to_string(), json!({"key1": 123})),
            (1.to_string(), json!({"key2": 123})),
            (2.to_string(), json!({"key1": 123})),
        ];
        let json_iter_in = json_iter_in.iter().cloned();

        let args = Cli::default();
        let settings = Settings::init(args).unwrap();
        let errors = Rc::new(RefCell::new(vec![]));

        let expected = FileStats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let file_stats = process_json_iterable(&settings, json_iter_in, &errors);
        assert_eq!(expected, file_stats);
        assert!(errors.borrow().is_empty())
    }

    #[test]
    fn simple_ndjson_iterable_par() {
        let iter: Vec<Result<String, std::io::Error>> = vec![
            Ok(r#"{"key1": 123}"#.to_string()),
            Ok(r#"{"key2": 123}"#.to_string()),
            Ok(r#"{"key1": 123}"#.to_string()),
        ];
        let iter = iter.into_iter();

        let expected = FileStats {
            keys_count: IndexMap::from([("$.key1".to_string(), 2), ("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 2),
                ("$.key2::Number".to_string(), 1),
            ]),
            ..Default::default()
        };

        let args = Cli::default();
        let file_stats = parse_json_iterable_par(&args, iter).unwrap();
        assert_eq!(expected, file_stats);
    }

    #[test]
    fn simple_ndjson_iterable_par_jsonpath() {
        let iter: Vec<Result<String, std::io::Error>> = vec![
            Ok(r#"{"key1": 123}"#.to_string()),
            Ok(r#"{"a": {"key2": 123}}"#.to_string()),
            Ok(r#"{"key1": 123}"#.to_string()),
        ];
        let iter = iter.into_iter();

        let expected = FileStats {
            keys_count: IndexMap::from([("$.key2".to_string(), 1)]),
            line_count: 3,
            keys_types_count: IndexMap::from([("$.key2::Number".to_string(), 1)]),
            empty_lines: vec![1, 3],
            ..Default::default()
        };

        let mut args = Cli::default();
        args.jsonpath = Some("$.a".to_string());
        let file_stats = parse_json_iterable_par(&args, iter).unwrap();
        assert_eq!(expected, file_stats);
    }

    #[test]
    fn add_filestats() {
        let lhs = FileStats {
            keys_count: IndexMap::from([("$.key1".to_string(), 3), ("$.key2".to_string(), 2)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 3),
                ("$.key2::Number".to_string(), 2),
            ]),
            ..Default::default()
        };
        let rhs = FileStats {
            keys_count: IndexMap::from([("$.key3".to_string(), 3), ("$.key2".to_string(), 2)]),
            line_count: 3,
            keys_types_count: IndexMap::from([
                ("$.key3::Number".to_string(), 3),
                ("$.key2::Number".to_string(), 2),
            ]),
            ..Default::default()
        };
        let expected = FileStats {
            keys_count: IndexMap::from([
                ("$.key1".to_string(), 3),
                ("$.key2".to_string(), 4),
                ("$.key3".to_string(), 3),
            ]),
            line_count: 6,
            keys_types_count: IndexMap::from([
                ("$.key1::Number".to_string(), 3),
                ("$.key2::Number".to_string(), 4),
                ("$.key3::Number".to_string(), 3),
            ]),
            ..Default::default()
        };

        let actual = lhs + rhs;

        assert_eq!(actual, expected)
    }
}
