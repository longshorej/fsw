extern crate notify;

const DRAIN_MS: u64 = 250;

use notify::{raw_watcher, RecursiveMode, Watcher};
use std::{
    env, io, path, process,
    sync::mpsc::{channel, Receiver, Sender},
    thread, time,
};

enum Msg {
    PathEvent(path::PathBuf),
    ThreadFinished,
}

fn handle(sender: Sender<Msg>, receiver: Receiver<Msg>, command: String, args: Vec<String>) {
    let mut running = false;

    let _ = sender.send(Msg::ThreadFinished);
    let mut waiting = true;

    while let Ok(path) = receiver.recv() {
        let run = match path {
            Msg::PathEvent(_) => {
                if running {
                    waiting = true;
                    false
                } else {
                    true
                }
            }

            Msg::ThreadFinished => {
                running = false;
                waiting
            }
        };

        if run {
            running = true;
            waiting = false;
            // we've found a file that isn't ignored, so
            // we'll wait a bit for any other fs events, and
            // then drain them all.

            thread::sleep(time::Duration::from_millis(DRAIN_MS));

            while let Ok(_) = receiver.try_recv() {}

            // we've drained everything, so we'll kick off our
            // process. we do this in another thread so that
            // we can continue to drain our channel, which
            // prevents unbounded memory consumption

            {
                let sender = sender.clone();
                let command = command.clone();
                let args = args.clone();
                let handle = process::Command::new(&command)
                    .args(args)
                    .stdin(process::Stdio::null())
                    .stdout(process::Stdio::inherit())
                    .stderr(process::Stdio::inherit())
                    .spawn();

                thread::spawn(move || {
                    handle_child(command, handle);

                    let _ = sender.send(Msg::ThreadFinished);
                });
            }
        }
    }
}

fn handle_child(command: String, result: io::Result<process::Child>) {
    match result.and_then(|mut s| s.wait()).map(|s| s.code()) {
        Ok(Some(c)) => {
            println!("{} exited with {}", command, c);
        }

        Ok(None) => {
            println!("{} exited with unknown", command);
        }

        Err(e) => {
            println!("{} failed with {}", command, e);
        }
    }
}

fn watch(sender: Sender<Msg>) -> io::Result<()> {
    let (tx, rx) = channel();

    let working_dir = env::current_dir()?;
    let git_dir = working_dir.join(".git");

    let mut watcher = raw_watcher(tx).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    watcher
        .watch(working_dir, RecursiveMode::Recursive)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    while let Ok(event) = rx.recv() {
        if let Some(path) = event.path {
            if !path.starts_with(&git_dir) && !git_ignored(&path) {
                sender
                    .send(Msg::PathEvent(path))
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }
        }
    }

    Ok(())
}

fn git_ignored(path: &path::Path) -> bool {
    if let Some(s) = path.to_str() {
        process::Command::new("git")
            .args(&["check-ignore", s])
            .stdin(process::Stdio::null())
            .stdout(process::Stdio::null())
            .stderr(process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        false
    }
}

fn main() {
    let mut args = env::args();

    if args.len() < 2 {
        process::exit(1);
    }

    let cmd = args.nth(1).unwrap();
    let args = args.collect::<Vec<String>>();

    let (sender, receiver) = channel();

    {
        let sender = sender.clone();

        thread::spawn(move || handle(sender, receiver, cmd, args));
    }

    if let Err(_e) = watch(sender) {
        process::exit(1);
    }
}
