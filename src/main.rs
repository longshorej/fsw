extern crate notify;

const DRAIN_MS: u64 = 125;
const GIT_PATH: &str = ".git";

use notify::{raw_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::HashSet,
    env, fs, io, path, process,
    sync::mpsc::{channel, Receiver, Sender},
    thread, time,
};

enum Msg {
    PathEvent,
    ThreadFinished,
}

/// Handles events (file notifcations / process notifcations) and forks a new process
/// when required.
fn handle(sender: Sender<Msg>, receiver: Receiver<Msg>, command: String, args: Vec<String>) {
    let mut running = false;

    let _ = sender.send(Msg::ThreadFinished);
    let mut waiting = true;

    while let Ok(path) = receiver.recv() {
        let run = match path {
            Msg::PathEvent => {
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

            while receiver.try_recv().is_ok() {}

            // we've drained everything, so we'll kick off our
            // process. we do this in another thread so that
            // we can continue to drain our channel, which
            // prevents unbounded memory consumption

            {
                let sender = sender.clone();
                let command = command.clone();
                let args = args.clone();

                thread::spawn(move || {
                    let status = process::Command::new(&command)
                        .args(args)
                        .stdin(process::Stdio::null())
                        .stdout(process::Stdio::inherit())
                        .stderr(process::Stdio::inherit())
                        .status();

                    match status.map(|s| s.code()) {
                        Ok(Some(c)) => {
                            println!("fsw: {} exited with {}", command, c);
                        }

                        Ok(None) => {
                            println!("fsw: {} exited with unknown", command);
                        }

                        Err(e) => {
                            println!("fsw: {} failed with {}", command, e);
                        }
                    }

                    let _ = sender.send(Msg::ThreadFinished);
                });
            }
        }
    }
}

/// Recursively walk a directory, watching everything including the specified directory
/// if it isn't already watched.
///
/// @FIXME if directories are removed, they aren't removed from the set (memory leak)
fn watch_dir(
    git_dir: &path::Path,
    watcher: &mut RecommendedWatcher,
    watching: &mut HashSet<path::PathBuf>,
    dir: &path::PathBuf,
) -> io::Result<()> {
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    for entry in fs::read_dir(dir)? {
        let entry_path = entry?.path();

        if entry_path.is_dir()
            && !git_ignored(git_dir, &entry_path)
            && !watching.contains(&entry_path)
        {
            watch_dir(git_dir, watcher, watching, &entry_path)?;

            watching.insert(entry_path);
        }
    }

    Ok(())
}

/// Watches the working directory of the process, and sends a
/// PathEvent to the provided sender if a relevant file or
/// directory changes.
fn watch(sender: Sender<Msg>) -> io::Result<()> {
    let (tx, rx) = channel();

    let working_dir = env::current_dir()?;
    let git_dir = working_dir.join(GIT_PATH);

    let mut watcher = raw_watcher(tx).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let mut watching = HashSet::new();

    watch_dir(&git_dir, &mut watcher, &mut watching, &working_dir)?;

    watching.insert(working_dir);

    while let Ok(event) = rx.recv() {
        if let Some(path) = event.path {
            if !path.starts_with(&git_dir) && !git_ignored(&git_dir, &path) {
                sender
                    .send(Msg::PathEvent)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                if !watching.contains(&path) && path.is_dir() {
                    watch_dir(&git_dir, &mut watcher, &mut watching, &path)?;
                }
            }
        }
    }

    Ok(())
}

/// Determines if the provided path is ignored by git,
/// returning true if it is.
///
/// FIXME: link to git or smth instead of forking a process
fn git_ignored(git_dir: &path::Path, path: &path::Path) -> bool {
    if path.starts_with(&git_dir) {
        return false;
    }

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
        let args = args.collect::<Vec<String>>();
        println!("fsw: version: {}", env!("CARGO_PKG_VERSION"));
        println!(
            "fsw: usage: {} <cmd> [<arg>]...",
            args.get(0).map(|s| s.as_str()).unwrap_or("fsw")
        );
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
