use grep_cli::is_tty_stdout;
use owo_colors::{OwoColorize, Stream};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::iter::Sum;
use std::ops::Add;

use crate::json::IndexMap;

/// Container for the data collected about the JSONs along the way
#[derive(Debug, PartialEq, Eq, Default, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub keys_count: IndexMap<String, usize>,
    pub line_count: usize,
    pub bad_lines: Vec<String>,
    pub keys_types_count: IndexMap<String, usize>,
    pub empty_lines: Vec<String>,
    // TODO: Add this: pub json_count: usize,
}

impl Stats {
    pub fn new() -> Stats {
        Stats {
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

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stream = Stream::Stdout;
        writeln!(f, "Keys:\n{:#?}\n", self.keys_count.keys())?;
        writeln!(f, "Key occurance counts:\n{:#?}", self.keys_count)?;
        writeln!(f, "\nKey occurance rate:")?;
        for (k, v) in self.key_occurance() {
            writeln!(f, "{}: {:.3}%", k, v)?;
        }
        writeln!(f, "\nKey type occurance rate:")?;
        for (k, v) in self.key_type_occurance() {
            writeln!(f, "{}: {:.3}%", k, v)?;
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

impl Stats {
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

#[derive(Debug, PartialEq, Eq, Default, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub file_path: String,
    pub stats: Stats,
}

impl FileStats {
    pub fn new(file_path: String, stats: Stats) -> Self {
        Self { file_path, stats }
    }
}

impl Add for FileStats {
    type Output = Stats;

    fn add(self, rhs: Self) -> Self::Output {
        let mut output = self.stats;

        for (k, v) in rhs.stats.keys_count {
            let counter = output.keys_count.entry(k).or_insert(0);
            *counter += v
        }

        for (k, v) in rhs.stats.keys_types_count {
            let counter = output.keys_types_count.entry(k).or_insert(0);
            *counter += v
        }

        output.line_count += rhs.stats.line_count;

        output.bad_lines = output
            .bad_lines
            .into_iter()
            .map(|line_id| format!("{}:{line_id}", self.file_path))
            .collect();
        output.bad_lines.extend(
            rhs.stats
                .bad_lines
                .into_iter()
                .map(|line_id| format!("{}:{line_id}", rhs.file_path)),
        );

        output.empty_lines = output
            .empty_lines
            .into_iter()
            .map(|line_id| format!("{}:{line_id}", self.file_path))
            .collect();
        output.empty_lines.extend(
            rhs.stats
                .empty_lines
                .into_iter()
                .map(|line_id| format!("{}:{line_id}", rhs.file_path)),
        );

        output
    }
}

impl Add<&Self> for FileStats {
    type Output = Stats;

    fn add(self, rhs: &Self) -> Self::Output {
        self.add(rhs.clone())
    }
}

impl<'a> Sum<&'a FileStats> for Stats {
    fn sum<I: Iterator<Item = &'a FileStats>>(iter: I) -> Stats {
        iter.fold(Self::default(), |acc, x| acc + x)
    }
}

impl Add<FileStats> for Stats {
    type Output = Self;

    fn add(self, rhs: FileStats) -> Self::Output {
        let mut output = self;

        for (k, v) in rhs.stats.keys_count {
            let counter = output.keys_count.entry(k).or_insert(0);
            *counter += v
        }

        for (k, v) in rhs.stats.keys_types_count {
            let counter = output.keys_types_count.entry(k).or_insert(0);
            *counter += v
        }

        output.line_count += rhs.stats.line_count;

        output.bad_lines.extend(
            rhs.stats
                .bad_lines
                .into_iter()
                .map(|line_id| format!("{}:{line_id}", rhs.file_path)),
        );

        output.empty_lines.extend(
            rhs.stats
                .empty_lines
                .into_iter()
                .map(|line_id| format!("{}:{line_id}", rhs.file_path)),
        );

        output
    }
}

impl Add<&FileStats> for Stats {
    type Output = Self;

    fn add(self, rhs: &FileStats) -> Self::Output {
        self.add(rhs.clone())
    }
}
