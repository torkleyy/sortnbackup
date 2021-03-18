use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{stdin, stdout, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use anyhow::{Context as _, Result};
use fakemap::FakeMap;
use humansize::FileSize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use parking_lot::{Condvar, Mutex};
use pathdiff::diff_paths;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{
    cli::cli_options,
    config::{Config, Rule, Source},
    file_path::FilePath,
    util::find_disk,
};

mod cli;
mod config;
mod date_time;
mod file_path;
mod img;
mod util;

#[derive(Default, Deserialize, Serialize)]
struct Context {
    ignored: Vec<PathBuf>,
    copy_instructions: FakeMap<PathBuf, CopyInstruction>,
    file_size_per_target: HashMap<String, u64>,
}

#[derive(Deserialize, Serialize)]
struct CopyInstruction {
    to: PathBuf,
    file_size: u64,
}

type Index = HashMap<String, Context>;

type Progress = HashMap<String, AtomicU32>;

fn read_config() -> Result<Config> {
    serde_yaml::from_reader(
        File::open("config.yaml").with_context(|| format!("cannot open config.yaml"))?,
    )
    .with_context(|| format!("cannot parse config.yaml"))
}

fn read_index() -> Result<Index> {
    serde_yaml::from_reader(
        File::open("index.yaml").with_context(|| format!("cannot open index.yaml"))?,
    )
    .with_context(|| format!("Cannot parse index.yaml"))
}

fn read_progress() -> Result<Progress> {
    serde_yaml::from_reader(
        File::open("progress.yaml").with_context(|| format!("cannot open progress.yaml"))?,
    )
    .with_context(|| format!("Cannot parse progress.yaml"))
}

fn app() -> Result<()> {
    let options = cli_options();

    let config: Config = read_config()?;

    let index = if options.continue_ {
        read_index()
            .with_context(|| format!("cannot continue backup because index cannot be read"))?
    } else {
        build_index(&config).with_context(|| format!("failed to build index"))?
    };

    let progress = if options.continue_ {
        read_progress()?
    } else {
        index
            .keys()
            .map(|source| (source.clone(), AtomicU32::new(0)))
            .collect()
    };

    let fmt_size = |size: u64| {
        size.file_size(config.settings.file_size_style.to_file_size_opts())
            .unwrap()
    };

    println!("Summary:");
    println!();

    for (source, context) in &index {
        if config
            .sources
            .get(source)
            .map(|s| s.disabled)
            .unwrap_or(false)
        {
            continue;
        }

        println!("Source '{}'", source);
        let total: u64 = context.file_size_per_target.values().cloned().sum();
        println!("\tData to copy [all targets]: {}", fmt_size(total));
        if total > 0 {
            println!("\tData per target:");
            for (target, size) in context.file_size_per_target.iter() {
                println!("\t\tTo target '{}': {}", target, fmt_size(*size));
            }
        }
    }

    println!();

    index
        .values()
        .flat_map(|context| context.file_size_per_target.iter())
        .fold(HashMap::new(), |mut map, (target, size)| {
            *map.entry(target.clone()).or_default() += *size;

            map
        })
        .iter()
        .for_each(|(target, size)| {

            if let Some(disk_info) = config.targets.get(target).and_then(|target| find_disk(target)) {
                println!("Target '{}' [{}] (free space: {})", target, fmt_size(*size), fmt_size(disk_info.available));

                if disk_info.available < *size {
                    eprintln!("WARNING: Free disk space on target's disk ({}) is less than data to copy (would need {} more)", disk_info.mount_point.display(), fmt_size(*size - disk_info.available));
                    if options.continue_ {
                        println!("Note: Continue option is enabled, space may be sufficient due to already copied files");
                    } else if disk_info.capacity >= *size {
                        println!("Note: Disk has enough (total) capacity (might want to free up space)");
                    }
                }
            } else {
                println!("Target '{}' [{}] (free space unknown)", target, fmt_size(*size));
            }
        });

    let total = index
        .values()
        .flat_map(|context| context.copy_instructions.values().map(|ci| ci.file_size))
        .sum();

    println!();
    if options.continue_ {
        let remaining = index
            .iter()
            .flat_map(|(src, context)| {
                context
                    .copy_instructions
                    .values()
                    .skip(progress[src].load(Ordering::SeqCst) as usize)
                    .map(|ci| ci.file_size)
            })
            .sum();

        println!(
            "Total data to copy (remaining): {} of {}",
            fmt_size(remaining),
            fmt_size(total)
        );
    } else {
        println!("Total data to copy: {}", fmt_size(total));
    }

    if !options.yes {
        print!("Continue? [y/N] ");
        stdout().flush().unwrap();

        let mut buf = String::new();
        stdin().read_line(&mut buf).unwrap();

        let input = buf.trim();
        let is_yes = input.starts_with("Y") || input.starts_with("y");

        if !is_yes {
            println!("Cancelled");
            return Ok(());
        }
    }

    copy_files(&index, progress, total)?;

    Ok(())
}

fn build_index(config: &Config) -> Result<Index> {
    println!("Building indices...");

    let multi_progress_bar = MultiProgress::new();
    let sty = ProgressStyle::default_spinner()
        // For more spinners check out the cli-spinners project:
        // https://github.com/sindresorhus/cli-spinners/blob/master/spinners.json
        .tick_strings(&[
            "▹▹▹▹▹",
            "▸▹▹▹▹",
            "▹▸▹▹▹",
            "▹▹▸▹▹",
            "▹▹▹▸▹",
            "▹▹▹▹▸",
            "▪▪▪▪▪",
        ])
        .template("{spinner:.blue} {msg}");

    let index = config
        .sources
        .par_iter()
        .map(|(name, source)| {
            if source.disabled {
                return Ok((name.to_owned(), Default::default()));
            }

            println!("Building index for source '{}'...", name);

            let pb = multi_progress_bar.add(ProgressBar::new_spinner());
            pb.set_style(sty.clone());
            pb.set_message(&format!("{}...", name));

            let mut context = Default::default();

            walk_dir(&config, name, source, &source.path, &mut context, &pb)?;

            pb.finish_with_message(&format!("{}... Done", name));

            println!("Building index for source '{}'... Done", name);

            Ok((name.to_owned(), context))
        })
        .collect::<Result<HashMap<String, Context>>>()?;

    serde_yaml::to_writer(
        File::create("index.yaml").with_context(|| format!("cannot create index.yaml"))?,
        &index,
    )?;

    multi_progress_bar.join_and_clear().unwrap();

    println!("Building indices... Done (saved to index.yaml)");

    Ok(index)
}

fn copy_files(index: &Index, progress: Progress, total_size: u64) -> Result<()> {
    println!("Copying files...");

    let mutex = Mutex::new(());
    let finished = Condvar::new();
    let finished = &finished;
    let progress = &progress;

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .progress_chars("#>-"));
    let pb = &pb;

    rayon::scope(|scope| {
        scope.spawn(move |_scope| {
            while finished
                .wait_for(&mut mutex.lock(), Duration::from_secs(15))
                .timed_out()
            {
                if let Ok(file) = File::create("progress.yaml") {
                    let _ = serde_yaml::to_writer(file, &progress);
                }
            }
        });

        index.par_iter().for_each(move |(source, context)| {
            let src_progress: &AtomicU32 = &progress[source];

            for (from, instr) in context
                .copy_instructions
                .iter()
                .skip(src_progress.load(Ordering::SeqCst) as usize)
            {
                let instr: &CopyInstruction = instr;
                let to = &instr.to;
                let _ = std::fs::create_dir_all(to.parent().unwrap());
                if let Err(e) = std::fs::copy(from, to) {
                    eprintln!(
                        "Failed to copy {} to {}: {}",
                        from.display(),
                        to.display(),
                        e
                    );
                }
                src_progress.fetch_add(1, Ordering::SeqCst);
                pb.inc(instr.file_size);
            }
        });

        finished.notify_all();
    });

    let _ = std::fs::remove_file("progress.yaml");
    let _ = std::fs::remove_file("index.yaml");

    pb.finish_with_message("copied");

    println!("Copying files... Done");

    Ok(())
}

fn walk_dir(
    config: &Config,
    src_name: &str,
    src: &Source,
    dir_path: &Path,
    context: &mut Context,
    pb: &ProgressBar,
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

            //println!("[{}]: {}", src_name, sub_path.display());
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

            pb.tick();

            match rule {
                Rule::Ignore => {
                    context.ignored.push(fp.full_path);
                }
                Rule::CopyExact { target } => {
                    let to = config.target(target)?.join(&fp.path);
                    let file_size = fp.metadata().map(|m| m.len()).unwrap_or(0);
                    *context
                        .file_size_per_target
                        .entry(target.clone())
                        .or_default() += file_size;
                    context
                        .copy_instructions
                        .insert(fp.full_path, CopyInstruction { to, file_size });
                }
                Rule::CopyTo { target, path } => {
                    let to = config.target_path(target, path, &mut fp)?;
                    let file_size = fp.metadata().map(|m| m.len()).unwrap_or(0);
                    *context
                        .file_size_per_target
                        .entry(target.clone())
                        .or_default() += file_size;
                    context
                        .copy_instructions
                        .insert(fp.full_path, CopyInstruction { to, file_size });
                }
                Rule::Traverse => {
                    walk_dir(config, src_name, src, &path, context, pb)?;
                }
                Rule::LogFile {
                    target,
                    log_file,
                    full_path,
                } => {
                    let log_file = config.target_path(target, log_file, &mut fp)?;
                    let mut file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&log_file)
                        .with_context(|| {
                            format!("Failed to open log file at {}", log_file.display())
                        })?;
                    let log_line = if *full_path {
                        fp.full_path.display()
                    } else {
                        fp.path.display()
                    };
                    writeln!(file, "{}", log_line).with_context(|| {
                        format!("Failed to write to log file {}", log_file.display())
                    })?;
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
