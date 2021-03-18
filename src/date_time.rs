use std::{convert::TryFrom, time::SystemTime};

use chrono::{
    format::{Item, StrftimeItems},
    Local,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "String")]
pub struct DateTimeFormatString(String);

impl DateTimeFormatString {
    /*
    pub fn fmt_exif(&self, dt: exif::DateTime) -> String {
        let dt: DateTime<Local> = DateTime::from_utc(
            NaiveDateTime::new(
                NaiveDate::from_ymd(dt.year as i32, dt.month as u32, dt.day as u32),
                NaiveTime::from_hms_nano(
                    dt.hour as u32,
                    dt.minute as u32,
                    dt.second as u32,
                    dt.nanosecond.unwrap_or(0) as u32,
                ),
            ),
            FixedOffset::east(60 * dt.offset.unwrap_or(0) as i32),
        );

        self.fmt_chrono(&dt)
    }
    */

    pub fn fmt_systime(&self, dt: SystemTime) -> String {
        self.fmt_chrono(&dt.into())
    }

    pub fn fmt_chrono(&self, dt: &chrono::DateTime<Local>) -> String {
        dt.format(&self.0).to_string()
    }
}

impl TryFrom<String> for DateTimeFormatString {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if is_valid_format_str(&value) {
            Ok(DateTimeFormatString(value))
        } else {
            Err(format!("Invalid data/time format string: '{}'\nCheck https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html for specification", value))
        }
    }
}

fn is_valid_format_str(s: &str) -> bool {
    StrftimeItems::new(s).all(|x| x != Item::Error)
}
