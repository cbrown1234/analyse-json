use std::io;
use std::io::BufRead;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;

pub fn spawn_stdin_channel(bound: usize) -> Receiver<String> {
    // https://stackoverflow.com/questions/30012995/how-can-i-read-non-blocking-from-stdin
    let (tx, rx) = mpsc::sync_channel::<String>(bound);
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let line = line.unwrap();
            tx.send(line).unwrap();
        }
    });
    rx
}
