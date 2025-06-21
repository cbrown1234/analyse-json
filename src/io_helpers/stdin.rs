use std::io;
use std::io::BufRead;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;

pub trait BackgroundRead {
    fn background_read_lines(self, bound: usize) -> Receiver<io::Result<String>>;
}

impl BackgroundRead for io::Stdin {
    // https://stackoverflow.com/questions/30012995/how-can-i-read-non-blocking-from-stdin
    /// Setup background thread to read input from stdin into a channel
    fn background_read_lines(self, bound: usize) -> Receiver<io::Result<String>> {
        let (tx, rx) = mpsc::sync_channel::<io::Result<String>>(bound);
        thread::spawn(move || {
            for line in self.lock().lines() {
                if tx.send(line).is_err() {
                    break;
                };
            }
        });
        rx
    }
}
