use std::{
    ffi::OsStr,
    fs::File,
    io::{self, BufRead},
};

use flate2::read::GzDecoder;

use crate::Cli;

pub fn get_bufreader(
    _args: &Cli,
    file_path: &std::path::PathBuf,
) -> Result<Box<dyn BufRead + Send>, io::Error> {
    let extension = file_path.extension().and_then(OsStr::to_str);
    let file = File::open(file_path)?;
    if extension == Some("gz") {
        let file = GzDecoder::new(file);
        Ok(Box::new(io::BufReader::new(file)))
    } else {
        Ok(Box::new(io::BufReader::new(file)))
    }
}
