use std::io;
use std::io::BufRead;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;

pub trait ToChannelReceiver {
    fn to_channel_receiver(self, bound: usize) -> Receiver<String>;
}

impl ToChannelReceiver for io::Stdin {
    fn to_channel_receiver(self, bound: usize) -> Receiver<String> {
        spawn_stdin_channel(self, bound)
    }
}

// TODO: Handle errors in a better way
// https://stackoverflow.com/questions/30012995/how-can-i-read-non-blocking-from-stdin
/// Setup background thread to read input from stdin into a channel
pub fn spawn_stdin_channel(stdin: io::Stdin, bound: usize) -> Receiver<String> {
    let (tx, rx) = mpsc::sync_channel::<String>(bound);
    thread::spawn(move || {
        for line in stdin.lock().lines() {
            let line = line.unwrap();
            tx.send(line).unwrap();
        }
    });
    rx
}
