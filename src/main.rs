use std::fs::{File, OpenOptions};

use anyhow::{Context as _, Result};

use crate::config::{Config, Rule, Source};
use crate::file_path::FilePath;
use fakemap::FakeMap;
use pathdiff::diff_paths;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use parking_lot::Mutex;

mod config;
mod date_time;
mod file_path;
mod img;
mod util;

#[derive(Default)]
struct Context {
    ignored: Vec<PathBuf>,
    copy_instructions: FakeMap<PathBuf, PathBuf>,
}

fn app() -> Result<()> {
    let config: Config = serde_yaml::from_reader(
        File::open("config.yaml").with_context(|| format!("cannot open config.yaml"))?,
    )
    .with_context(|| format!("Cannot parse config.yaml"))?;

    let mut index = Mutex::new(FakeMap::new());

    println!("Building indices...");
    for (name, source) in &config.sources {
        if source.disabled {
            continue;
        }

        println!("Building index for source '{}'...", name);

        let mut context = Default::default();

        walk_dir(&config, name, source, &source.path, &mut context)?;

        index.lock().insert(name.to_owned(), context.copy_instructions);

        println!("Building index for source '{}'... Done", name);
    }

    let index = index.into_inner();

    serde_yaml::to_writer(File::create("index.yaml").with_context(|| format!("cannot open index.yaml"))?, &index)?;

    println!("Building indices... Done (saved to index.yaml)");

    println!("Copying files...");

    for (source, from_to) in index.iter() {
        for (from, to) in from_to.iter() {
            let _ = std::fs::create_dir_all(to.parent().unwrap());
            if let Err(e) = std::fs::copy(from, to) {
                eprintln!("Failed to copy {} to {}: {}", from.display(), to.display(), e);
            }
        }
    }

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
