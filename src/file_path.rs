use std::{fs::Metadata, path::PathBuf, sync::Arc};

use crate::img::ImageMetadata;

enum Lazy<T> {
    Some(T),
    NotInitialized(Box<dyn FnOnce() -> Option<T>>),
    Err,
}

impl<T> Lazy<T>
where
    T: Clone,
{
    pub fn new(f: impl FnOnce() -> Option<T> + 'static) -> Self {
        Lazy::NotInitialized(Box::new(f))
    }

    pub fn get(&mut self) -> Option<T> {
        match self {
            Lazy::Some(x) => Some(x.clone()),
            Lazy::NotInitialized(_) => {
                let f = match std::mem::replace(self, Lazy::Err) {
                    Lazy::NotInitialized(f) => f,
                    _ => unimplemented!(),
                };
                match f() {
                    None => {
                        *self = Lazy::Err;

                        None
                    }
                    Some(x) => {
                        *self = Lazy::Some(x.clone());

                        Some(x)
                    }
                }
            }
            Lazy::Err => None,
        }
    }
}

pub struct FilePath {
    pub source_path: PathBuf,
    pub path: PathBuf,
    pub full_path: PathBuf,
    metadata: Lazy<Arc<Metadata>>,
    img_metadata: Lazy<ImageMetadata>,
}

impl FilePath {
    pub fn new(source_path: impl Into<PathBuf>, path: impl Into<PathBuf>) -> Self {
        FilePath::new_internal(source_path.into(), path.into())
    }

    fn new_internal(source_path: PathBuf, path: PathBuf) -> Self {
        let full_path = source_path.join(&path);
        let full_path2 = full_path.clone();

        FilePath {
            source_path,
            path,
            full_path: full_path.clone(),
            metadata: Lazy::new(move || std::fs::metadata(&full_path).ok().map(Arc::new)),
            img_metadata: Lazy::new(move || ImageMetadata::for_path(&full_path2)),
        }
    }

    pub fn metadata(&mut self) -> Option<Arc<Metadata>> {
        self.metadata.get()
    }

    pub fn img_metadata(&mut self) -> Option<ImageMetadata> {
        self.img_metadata.get()
    }
}
