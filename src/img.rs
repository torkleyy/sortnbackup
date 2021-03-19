use std::{fs::File, io::BufReader, path::Path};

use chrono::{Local, TimeZone};
use exif::{Exif, Field, In, Tag, Value};
use immeta::Dimensions;
use serde::{Deserialize, Serialize};

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
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub dimensions: ImageDimensions,
}

impl ImageMetadata {
    fn from_immeta(path: &Path) -> anyhow::Result<Self> {
        Ok(ImageMetadata {
            dimensions: immeta::load_from_file(path)?.dimensions().into(),
            date_time: None,
            camera_make: None,
            camera_model: None,
        })
    }

    pub fn for_path(path: &Path) -> Option<Self> {
        Self::for_path_inner(path).ok()
    }

    fn for_path_inner(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let exif = match exif::Reader::new().read_from_container(&mut BufReader::new(&file)) {
            Ok(x) => x,
            Err(exif::Error::NotFound(_)) => return Self::from_immeta(path),
            Err(e) => return Err(e.into()),
        };

        let dimensions = match get_exif_dimensions(&exif) {
            Some(d) => d,
            None => Self::from_immeta(path).map(|m| m.dimensions)?,
        };

        Ok(ImageMetadata {
            dimensions,
            date_time: get_date_time(
                exif.get_field(Tag::DateTime, In::PRIMARY)
                    .or(exif.get_field(Tag::DateTimeOriginal, In::PRIMARY))
                    .or(exif.get_field(Tag::DateTimeDigitized, In::PRIMARY)),
            ),
            camera_make: get_string(exif.get_field(Tag::Make, In::PRIMARY)),
            camera_model: get_string(exif.get_field(Tag::Model, In::PRIMARY)),
        })
    }
}

fn get_exif_dimensions(exif: &Exif) -> Option<ImageDimensions> {
    let width = exif
        .get_field(Tag::ImageWidth, In::PRIMARY)?
        .value
        .get_uint(0)?;
    let height = exif
        .get_field(Tag::ImageLength, In::PRIMARY)?
        .value
        .get_uint(0)?;

    Some(ImageDimensions { width, height })
}

fn get_date_time(value: Option<&Field>) -> Option<chrono::DateTime<Local>> {
    Some(
        Local
            .datetime_from_str(get_str(value)?, "%Y:%m:%d %H:%M:%S")
            .unwrap()
            .into(),
    )
}

fn get_str(value: Option<&Field>) -> Option<&str> {
    match &value.as_ref()?.value {
        Value::Ascii(v) => std::str::from_utf8(v.get(0)?).ok(),
        _ => None,
    }
}

fn get_string(value: Option<&Field>) -> Option<String> {
    get_str(value).map(ToOwned::to_owned)
}
