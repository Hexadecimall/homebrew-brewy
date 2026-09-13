use std::{
    env, fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::Sender,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::model::{Catalog, Operation};
use serde::{Deserialize, Serialize};

const CACHE_SCHEMA: u32 = 1;
const METADATA_MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);
const FORMULA_API: &str = "https://formulae.brew.sh/api/formula.json";
const CASK_API: &str = "https://formulae.brew.sh/api/cask.json";

#[derive(Deserialize, Serialize)]
struct CatalogCache {
    schema: u32,
    metadata_updated: u64,
    catalog: Catalog,
}

#[derive(Debug)]
pub enum WorkerEvent {
    CatalogLoaded(Result<Catalog, String>),
    DetailLoaded {
        requested_name: String,
        requested_kind: crate::model::PackageKind,
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

pub fn load_detail(name: String, kind: crate::model::PackageKind, tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        let kind_arg = match kind {
            crate::model::PackageKind::Formula => "--formula",
            crate::model::PackageKind::Cask => "--cask",
        };
        let result = run_capture(&["info", "--json=v2", kind_arg, &name])
            .and_then(|json| Catalog::package_from_json(&json));
        let _ = tx.send(WorkerEvent::DetailLoaded {
            requested_name: name,
            requested_kind: kind,
            result,
        });
    });
}

pub fn cached_catalog() -> Option<Catalog> {
    load_cache().map(|cache| cache.catalog)
}

pub fn load_catalog(tx: Sender<WorkerEvent>, force_metadata: bool) {
    thread::spawn(move || {
        let result = collect_catalog(force_metadata);
        let _ = tx.send(WorkerEvent::CatalogLoaded(result));
    });
}

fn collect_catalog(force_metadata: bool) -> Result<Catalog, String> {
    let cached = load_cache();
    let refresh_metadata = force_metadata
        || cached.as_ref().is_none_or(|cache| {
            current_timestamp().saturating_sub(cache.metadata_updated) >= METADATA_MAX_AGE.as_secs()
        });

    let (formulae, casks, installed, outdated, taps, formula_metadata, cask_metadata) =
        thread::scope(|scope| {
            let formulae = scope.spawn(|| run_capture(&["formulae"]));
            let casks = scope.spawn(|| run_capture(&["casks"]));
            let installed = scope.spawn(|| run_capture(&["info", "--json=v2", "--installed"]));
            let outdated = scope.spawn(|| run_capture(&["outdated", "--json=v2"]));
            let taps = scope.spawn(|| run_capture(&["tap"]));
            let formula_metadata =
                refresh_metadata.then(|| scope.spawn(|| download_metadata(FORMULA_API)));
            let cask_metadata =
                refresh_metadata.then(|| scope.spawn(|| download_metadata(CASK_API)));
            (
                formulae
                    .join()
                    .unwrap_or_else(|_| Err("formula list worker failed".into())),
                casks
                    .join()
                    .unwrap_or_else(|_| Err("cask list worker failed".into())),
                installed
                    .join()
                    .unwrap_or_else(|_| Err("installed package worker failed".into())),
                outdated
                    .join()
                    .unwrap_or_else(|_| Err("outdated package worker failed".into())),
                taps.join()
                    .unwrap_or_else(|_| Err("tap list worker failed".into())),
                formula_metadata.and_then(|worker| worker.join().ok()?.ok()),
                cask_metadata.and_then(|worker| worker.join().ok()?.ok()),
            )
        });

    let installed = installed.unwrap_or_else(|_| r#"{"formulae":[],"casks":[]}"#.into());
    let outdated = outdated.unwrap_or_else(|_| r#"{"formulae":[],"casks":[]}"#.into());
    let mut catalog = Catalog::from_outputs(
        &formulae.unwrap_or_default(),
        &casks.unwrap_or_default(),
        &installed,
        &outdated,
        &taps.unwrap_or_default(),
    )?;
    if let Some(cache) = &cached {
        catalog.merge_cached_metadata(&cache.catalog);
    }

    let metadata_updated =
        if let (Some(formula_json), Some(cask_json)) = (formula_metadata, cask_metadata) {
            catalog.apply_metadata_outputs(&formula_json, &cask_json)?;
            current_timestamp()
        } else {
            cached.as_ref().map_or(0, |cache| cache.metadata_updated)
        };
    if catalog.packages.is_empty() {
        return Err("Homebrew returned an empty package catalog".into());
    }
    save_cache(&CatalogCache {
        schema: CACHE_SCHEMA,
        metadata_updated,
        catalog: catalog.clone(),
    });
    Ok(catalog)
}

fn download_metadata(url: &str) -> Result<String, String> {
    let output = Command::new("/usr/bin/curl")
        .args([
            "-fsSL",
            "--compressed",
            "--connect-timeout",
            "3",
            "--max-time",
            "20",
            url,
        ])
        .output()
        .map_err(|error| format!("could not download Homebrew metadata: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err("could not download Homebrew metadata".into())
    }
}

fn cache_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Caches")))?;
    Some(base.join("brewy/catalog-v1.json"))
}

fn load_cache() -> Option<CatalogCache> {
    let data = fs::read(cache_path()?).ok()?;
    let cache: CatalogCache = serde_json::from_slice(&data).ok()?;
    (cache.schema == CACHE_SCHEMA).then_some(cache)
}

fn save_cache(cache: &CatalogCache) {
    let Some(path) = cache_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let Ok(data) = serde_json::to_vec(cache) else {
        return;
    };
    if fs::write(&temporary, data).is_ok() {
        let _ = fs::rename(temporary, path);
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
