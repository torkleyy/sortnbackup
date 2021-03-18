use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use fakemap::FakeMap;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    date_time::DateTimeFormatString, file_path::FilePath, util::fix_cross_path,
};
use humansize::file_size_opts::{FileSizeOpts, BINARY, DECIMAL};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub file_groups: FakeMap<String, FileGroup>,
    pub sources: HashMap<String, Source>,
    pub targets: HashMap<String, PathBuf>,

    pub settings: Settings,
}

impl Config {
    pub fn file_group(&self, src_name: &str, fp: &mut FilePath) -> Option<(&str, &FileGroup)> {
        self.file_groups
            .iter()
            .map(|(k, v)| (k as &str, v))
            .filter(|(_, v)| v.sources.includes(src_name))
            .find(|(_k, v)| v.filter.matches(fp))
    }

    pub fn target(&self, target: &str) -> Result<&PathBuf> {
        self.targets
            .get(target)
            .ok_or_else(|| anyhow!("Unknown target: '{}'", target))
    }

    pub fn target_path(
        &self,
        target: &str,
        paths: &[PathElement],
        fp: &mut FilePath,
    ) -> Result<PathBuf> {
        PathElement::join_all(paths, fp, self.target(target)?.clone())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub file_size_style: FileSizeStyle,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum FileSizeStyle {
    #[serde(rename = "binary")]
    Binary,
    #[serde(rename = "decimal")]
    Decimal,
}

impl FileSizeStyle {
    pub fn to_file_size_opts(&self) -> &FileSizeOpts {
        match self {
            FileSizeStyle::Binary => &BINARY,
            FileSizeStyle::Decimal => &DECIMAL,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileGroup {
    pub sources: SourceFilter,
    pub filter: FileFilter,
    pub rule: Rule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    #[serde(default)]
    pub ignore_paths: Vec<PathBuf>,
    pub path: PathBuf,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum SourceFilter {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "except")]
    Except(Vec<String>),
    #[serde(rename = "only")]
    Only(Vec<String>),
}

impl SourceFilter {
    pub fn includes(&self, s: &str) -> bool {
        match self {
            SourceFilter::All => true,
            SourceFilter::Except(except) => !except.iter().any(|x| x == s),
            SourceFilter::Only(only) => only.iter().any(|x| x == s),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum FileFilter {
    #[serde(rename = "all")]
    All(Vec<FileFilter>),
    #[serde(rename = "any")]
    Any(Vec<FileFilter>),
    #[serde(rename = "not")]
    Not(Box<FileFilter>),
    #[serde(rename = "catch_all")]
    CatchAll,
    #[serde(rename = "in_folder")]
    InRootPath(String),
    #[serde(rename = "directly_in_folder")]
    ImmediateParent(String),
    #[serde(rename = "has_extension")]
    HasExtension(Vec<String>),
    #[serde(rename = "file_name_matches_regex")]
    FileNameMatchesRegex(#[serde(with = "serde_regex")] Regex),
    #[serde(rename = "path_matches_regex")]
    PathMatchesRegex(#[serde(with = "serde_regex")] Regex),
    #[serde(rename = "has_img_date_time")]
    HasImageDateTime,
    #[serde(rename = "has_img_metadata")]
    HasImageMetadata,
    #[serde(rename = "is_file")]
    IsFile,
    #[serde(rename = "is_dir")]
    IsDir,
    #[serde(rename = "img_size")]
    ImgSize { min: Option<u32>, max: Option<u32> },
}

impl FileFilter {
    pub fn matches(&self, file_path: &mut FilePath) -> bool {
        let path = &file_path.path;

        match self {
            FileFilter::All(v) => v.iter().all(|x| x.matches(file_path)),
            FileFilter::Any(v) => v.iter().any(|x| x.matches(file_path)),
            FileFilter::HasExtension(exts) => match path.extension().and_then(|s| s.to_str()) {
                None => false,
                Some(s) => exts.iter().any(|ext| ext.eq_ignore_ascii_case(s)),
            },
            FileFilter::FileNameMatchesRegex(r) => {
                match path.file_name().and_then(|s| s.to_str()) {
                    None => false,
                    Some(s) => r.is_match(s),
                }
            }
            FileFilter::PathMatchesRegex(r) => match path.to_str() {
                None => false,
                Some(s) => r.is_match(s),
            },
            FileFilter::ImgSize { min, max } => match file_path.img_metadata() {
                Some(meta) => {
                    min.map(|min| meta.dimensions.ensure_min(min))
                        .unwrap_or(true)
                        && max
                            .map(|max| meta.dimensions.ensure_max(max))
                            .unwrap_or(true)
                }
                None => false,
            },
            FileFilter::HasImageMetadata => file_path.img_metadata().is_some(),
            FileFilter::HasImageDateTime => file_path
                .img_metadata()
                .map(|x| x.date_time.is_some())
                .unwrap_or(false),
            FileFilter::InRootPath(folder) => {
                let folder = fix_cross_path(folder);
                let mut path = &file_path.path as &Path;
                while let Some(parent) = path.parent() {
                    if parent == &folder {
                        return true;
                    }

                    path = parent;
                }

                false
            }
            FileFilter::Not(f) => !f.matches(file_path),
            FileFilter::CatchAll => true,
            FileFilter::IsFile => file_path.full_path.is_file(),
            FileFilter::IsDir => file_path.full_path.is_dir(),
            FileFilter::ImmediateParent(p) => file_path.path.parent().unwrap() == Path::new(p),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum Rule {
    #[serde(rename = "ignore")]
    Ignore,
    #[serde(rename = "copy_exact")]
    CopyExact { target: String },
    #[serde(rename = "copy_to")]
    CopyTo {
        target: String,
        path: Vec<PathElement>,
    },
    #[serde(rename = "traverse")]
    Traverse,
    #[serde(rename = "log_file")]
    LogFile {
        target: String,
        log_file: Vec<PathElement>,
        full_path: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum PathElement {
    #[serde(rename = "file_name")]
    FileName(String),
    #[serde(rename = "original_path")]
    OriginalPath,
    #[serde(rename = "original_path_without_file_name")]
    OriginalPathWithoutFileName,
    #[serde(rename = "direct_parent_folder")]
    DirectParentFolder,
    #[serde(rename = "file_name_with_extension")]
    FileNameWithExtension,
    #[serde(rename = "file_name_without_extension")]
    FileNameWithoutExtension,
    #[serde(rename = "file_extension")]
    FileExtension,
    #[serde(rename = "img_date_time")]
    ImageDateTime(DateTimeFormatString),
    #[serde(rename = "access_time")]
    AccessTime(DateTimeFormatString),
    #[serde(rename = "created_time")]
    CreatedTime(DateTimeFormatString),
    #[serde(rename = "modified_time")]
    ModifiedTime(DateTimeFormatString),
}

impl PathElement {
    pub fn join_all(paths: &[PathElement], fp: &mut FilePath, base: PathBuf) -> Result<PathBuf> {
        Ok(paths
            .iter()
            .map(|p| p.to_path(fp))
            .collect::<Result<Vec<PathBuf>>>()?
            .into_iter()
            .fold(base, |mut buf, path| {
                buf.push(path);

                buf
            }))
    }

    pub fn to_path(&self, fp: &mut FilePath) -> Result<PathBuf> {
        self.to_path_inner(fp)
            .with_context(|| format!("failed to evaluate path element {:?}", self))
    }

    fn to_path_inner(&self, fp: &mut FilePath) -> Result<PathBuf> {
        Ok(match self {
            PathElement::FileName(s) => s.into(),
            PathElement::OriginalPathWithoutFileName => fp.path.parent().unwrap().to_owned(),
            PathElement::OriginalPath => fp.path.clone(),
            PathElement::DirectParentFolder => {
                fp.path.parent().unwrap().file_name().unwrap().into()
            }
            PathElement::FileNameWithExtension => fp.path.file_name().unwrap().into(),
            PathElement::FileNameWithoutExtension => fp.path.file_stem().unwrap().into(),
            PathElement::FileExtension => fp.path.extension().unwrap().into(),
            PathElement::ImageDateTime(fmt) => fmt
                .fmt_chrono(
                    &fp.img_metadata()
                        .ok_or(anyhow!("No image metadata"))?
                        .date_time
                        .ok_or(anyhow!("No image date/time"))?,
                )
                .into(),
            PathElement::AccessTime(fmt) => fmt
                .fmt_systime(fp.metadata().ok_or(anyhow!("No fs metadata"))?.accessed()?)
                .into(),
            PathElement::CreatedTime(fmt) => fmt
                .fmt_systime(fp.metadata().ok_or(anyhow!("No fs metadata"))?.created()?)
                .into(),
            PathElement::ModifiedTime(fmt) => fmt
                .fmt_systime(fp.metadata().ok_or(anyhow!("No fs metadata"))?.modified()?)
                .into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_path() {
        use FileFilter::InRootPath;

        let mut fp = FilePath::new("src", "lala/foo/bar");

        assert!(InRootPath("lala".to_owned()).matches(&mut fp));
        assert!(InRootPath("lala/foo".to_owned()).matches(&mut fp));

        assert!(!InRootPath("src".to_owned()).matches(&mut fp));
        assert!(!InRootPath("foo".to_owned()).matches(&mut fp));
        assert!(!InRootPath("bar".to_owned()).matches(&mut fp));
    }
}
