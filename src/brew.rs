use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc::Sender,
    thread,
};

use crate::model::{Catalog, Operation};

#[derive(Debug)]
pub enum WorkerEvent {
    CatalogLoaded(Result<Catalog, String>),
    DetailLoaded {
        requested_name: String,
        result: Result<crate::model::Package, String>,
    },
    Log(String),
    OperationStarted(String),
    OperationFinished {
        summary: String,
        success: bool,
    },
    QueueFinished,
}

pub fn load_detail(name: String, tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        let result = run_capture(&["info", "--json=v2", &name])
            .and_then(|json| Catalog::package_from_json(&json));
        let _ = tx.send(WorkerEvent::DetailLoaded {
            requested_name: name,
            result,
        });
    });
}

pub fn load_catalog(tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        let result = collect_catalog();
        let _ = tx.send(WorkerEvent::CatalogLoaded(result));
    });
}

fn collect_catalog() -> Result<Catalog, String> {
    let formulae = run_capture(&["formulae"])?;
    let casks = run_capture(&["casks"])?;
    let installed = run_capture(&["info", "--json=v2", "--installed"])?;
    let outdated = run_capture(&["outdated", "--json=v2"])
        .unwrap_or_else(|_| r#"{"formulae":[],"casks":[]}"#.into());
    let taps = run_capture(&["tap"]).unwrap_or_default();
    Catalog::from_outputs(&formulae, &casks, &installed, &outdated, &taps)
}

fn run_capture(args: &[&str]) -> Result<String, String> {
    let output = Command::new("brew")
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .args(args)
        .output()
        .map_err(|error| format!("could not run brew: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if message.is_empty() {
            format!("brew {} failed", args.join(" "))
        } else {
            message
        })
    }
}

pub fn run_operations(operations: Vec<Operation>, tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        for operation in operations {
            let summary = operation.summary();
            let _ = tx.send(WorkerEvent::OperationStarted(summary.clone()));
            let args = operation.command();
            let success = stream_command(&args, &tx);
            let _ = tx.send(WorkerEvent::OperationFinished { summary, success });
            if !success {
                break;
            }
        }
        let _ = tx.send(WorkerEvent::QueueFinished);
    });
}

fn stream_command(args: &[String], tx: &Sender<WorkerEvent>) -> bool {
    let command_line = format!("$ brew {}", args.join(" "));
    let _ = tx.send(WorkerEvent::Log(command_line));
    let mut child = match Command::new("brew")
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ANALYTICS", "1")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = tx.send(WorkerEvent::Log(format!("failed to start brew: {error}")));
            return false;
        }
    };

    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(spawn_reader(stdout, tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(spawn_reader(stderr, tx.clone()));
    }
    let success = child.wait().is_ok_and(|status| status.success());
    for reader in readers {
        let _ = reader.join();
    }
    success
}

fn spawn_reader<R: std::io::Read + Send + 'static>(
    reader: R,
    tx: Sender<WorkerEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            let _ = tx.send(WorkerEvent::Log(line));
        }
    })
}
