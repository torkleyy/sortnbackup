use std::{
    fs,
    fs::canonicalize,
    io,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context};
use sysinfo::{DiskExt, RefreshKind, System, SystemExt};

pub fn fix_cross_path(path: &str) -> PathBuf {
    let mut buf = [0; 4];
    path.replace("/", std::path::MAIN_SEPARATOR.encode_utf8(&mut buf))
        .into()
}

pub fn dir_size(path: &Path) -> anyhow::Result<u64> {
    fn dir_size(path: &Path) -> io::Result<u64> {
        fn dir_size(mut dir: fs::ReadDir) -> io::Result<u64> {
            dir.try_fold(0, |acc, file| {
                let file = file?;
                let size = match file.metadata()? {
                    data if data.is_dir() => dir_size(fs::read_dir(file.path())?)?,
                    data => data.len(),
                };
                Ok(acc + size)
            })
        }

        dir_size(fs::read_dir(path)?)
    }

    dir_size(path).with_context(|| anyhow!("failed to calc dir size of {}", path.display()))
}

pub fn is_root_path_of(path: &Path, root: &Path) -> bool {
    let mut path = path;
    while let Some(parent) = path.parent() {
        if parent == root {
            return true;
        }

        path = parent;
    }

    false
}

pub struct DiskInfo {
    pub name: String,
    pub available: u64,
    pub capacity: u64,
    pub mount_point: PathBuf,
}

pub fn find_disk(path: &Path) -> Option<DiskInfo> {
    let path = canonicalize(path).ok()?;

    let sys = System::new_with_specifics(RefreshKind::new().with_disks().with_disks_list());
    sys.get_disks()
        .iter()
        .filter_map(|disk| {
            let root = canonicalize(disk.get_mount_point()).ok()?;

            if is_root_path_of(&path, &root) {
                Some(DiskInfo {
                    name: disk.get_name().to_string_lossy().into_owned(),
                    available: disk.get_available_space(),
                    capacity: disk.get_total_space(),
                    mount_point: disk.get_mount_point().to_owned(),
                })
            } else {
                None
            }
        })
        .next()
}
