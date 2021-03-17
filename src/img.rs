use std::{fs::File, io::BufReader, path::Path, str::FromStr};

use chrono::Local;
use exif::{Field, In, Tag, Value};
use serde::{Deserialize, Serialize};
use std::fs::read;
use immeta::Dimensions;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}

impl ImageDimensions {
    pub fn ensure_min(&self, min: u32) -> bool {
        self.width >= min && self.height >= min
    }

    pub fn ensure_max(&self, max: u32) -> bool {
        self.width <= max && self.height <= max
    }
}

impl From<immeta::Dimensions> for ImageDimensions {
    fn from(d: Dimensions) -> Self {
        ImageDimensions {
            width: d.width,
            height: d.height,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ImageMetadata {
    pub date_time: Option<chrono::DateTime<Local>>,
    pub dimensions: ImageDimensions,
}

impl ImageMetadata {
    fn from_immeta(path: &Path) -> Option<Self> {
        Some(ImageMetadata {
            dimensions: immeta::load_from_file(path).ok()?.dimensions().into(),
            date_time: None,
        })
    }

    pub fn for_path(path: &Path) -> Option<Self> {
        let file = File::open(path).ok()?;
        let exif = match exif::Reader::new()
            .read_from_container(&mut BufReader::new(&file)) {
            Ok(x) => x,
            Err(exif::Error::NotFound(_)) => {
                return Self::from_immeta(path)
            }
            _ => return None,
        };

        let width = exif
            .get_field(Tag::ImageWidth, In::PRIMARY)?
            .value
            .get_uint(0)?;
        let height = exif
            .get_field(Tag::ImageLength, In::PRIMARY)?
            .value
            .get_uint(0)?;

        Some(ImageMetadata {
            dimensions: ImageDimensions { width, height },
            date_time: get_date_time(exif.get_field(Tag::DateTime, In::PRIMARY)),
        })
    }
}

fn get_date_time(value: Option<&Field>) -> Option<chrono::DateTime<Local>> {
    match &value.as_ref()?.value {
        Value::Ascii(v) => {
            let s = std::str::from_utf8(v.get(0)?).ok()?;

            chrono::DateTime::from_str(s).ok()
        }
        _ => None,
    }
}
