use std::fs::{File, OpenOptions};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::config::{Config, Rule, Source};
use crate::file_path::FilePath;
use fakemap::FakeMap;
use pathdiff::diff_paths;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use parking_lot::Mutex;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

mod config;
mod date_time;
mod file_path;
mod img;
mod util;

#[derive(Default, Deserialize, Serialize)]
struct Context {
    ignored: Vec<PathBuf>,
    copy_instructions: FakeMap<PathBuf, PathBuf>,
}

type Index = HashMap<String, Context>;

type Progress = HashMap<String, AtomicU32>;

fn app() -> Result<()> {
    let config: Config = serde_yaml::from_reader(
        File::open("config.yaml").with_context(|| format!("cannot open config.yaml"))?,
    )
    .with_context(|| format!("Cannot parse config.yaml"))?;

    let index = build_index(&config)?;
    let progress = index.keys().map(|source| (source.clone(), AtomicU32::new(0))).collect();

    copy_files(&index, progress)?;

    Ok(())
}

fn build_index(config: &Config) -> Result<Index> {
    println!("Building indices...");
    let index = config.sources.par_iter().map(|(name, source)| {
        if source.disabled {
            return Ok((name.to_owned(), Default::default()));
        }

        println!("Building index for source '{}'...", name);

        let mut context = Default::default();

        walk_dir(&config, name, source, &source.path, &mut context)?;

        println!("Building index for source '{}'... Done", name);

        Ok((name.to_owned(), context))
    }).collect::<Result<HashMap<String, Context>>>()?;

    serde_yaml::to_writer(File::create("index.yaml").with_context(|| format!("cannot open index.yaml"))?, &index)?;

    println!("Building indices... Done (saved to index.yaml)");

    Ok(index)
}

fn copy_files(index: &Index, progress: Progress) -> Result<()> {
    println!("Copying files...");

    index.par_iter().for_each(|(source, context)| {
        let src_progress: &AtomicU32 = &progress[source];

        for (from, to) in context.copy_instructions.iter().skip(src_progress.load(Ordering::SeqCst)) {
            let _ = std::fs::create_dir_all(to.parent().unwrap());
            if let Err(e) = std::fs::copy(from, to) {
                eprintln!("Failed to copy {} to {}: {}", from.display(), to.display(), e);
            }
            src_progress.fetch_add(1, Ordering::SeqCst);

            // TODO: store progress
        }
    });

    println!("Copying files... Done");

    Ok(())


}

fn walk_dir(
    config: &Config,
    src_name: &str,
    src: &Source,
    dir_path: &Path,
    context: &mut Context,
) -> Result<()> {
    for entry in WalkDir::new(dir_path).min_depth(1).max_depth(1) {
        if let Ok(entry) = entry {
            let path = entry.into_path();
            let sub_path = diff_paths(&path, &src.path).unwrap();

            if src.ignore_paths.iter().any(|x| *x == sub_path) {
                //println!("[{}]: Ignore {}", src_name, sub_path.display());
                context.ignored.push(sub_path);
                continue;
            }

            println!("[{}]: {}", src_name, sub_path.display());
            let mut fp = FilePath::new(&src.path, sub_path);
            assert_eq!(path, fp.full_path);

            let rule = if let Some((_group_name, file_group)) = config.file_group(src_name, &mut fp)
            {
                &file_group.rule
            } else {
                if fp.full_path.is_dir() {
                    &Rule::Traverse
                } else {
                    &Rule::Ignore
                }
            };

            match rule {
                Rule::Ignore => {
                    context.ignored.push(fp.full_path);
                }
                Rule::CopyExact { target } => {
                    let to = config.target(target)?.join(&fp.path);
                    context.copy_instructions.insert(fp.full_path, to);
                }
                Rule::CopyTo { target, path } => {
                    let to = config.target_path(target, path, &mut fp)?;
                    context.copy_instructions.insert(fp.full_path, to);
                }
                Rule::Traverse => {
                    walk_dir(config, src_name, src, &path, context)?;
                }
                Rule::LogFile {
                    target,
                    log_file,
                    full_path,
                } => {
                    use std::io::Write;

                    let log_file = config.target_path(target, log_file, &mut fp)?;
                    let mut file = OpenOptions::new().create(true).append(true).open(&log_file).with_context(|| format!("Failed to open log file at {}", log_file.display()))?;
                    let log_line = if *full_path {
                        fp.full_path.display()
                    } else {
                        fp.path.display()
                    };
                    writeln!(file, "{}", log_line).with_context(|| format!("Failed to write to log file {}", log_file.display()))?;
                }
            }
        } else {
            eprintln!("{}", entry.err().unwrap());
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = app() {
        eprintln!("error: {}", e);
        e.chain()
            .skip(1)
            .for_each(|c| eprintln!("caused by: {}", c));
    };
}
