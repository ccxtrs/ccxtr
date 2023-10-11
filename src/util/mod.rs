use std::ops::Neg;
use std::str::FromStr;

use chrono::{TimeZone, Utc};

use crate::error::{Error, Result};

mod collections;
pub(crate) mod channel;

pub(crate) fn into_precision(s: String) -> Result<isize> {
    let d = f64::from_str(&s)?;

    if d > 1_f64 {
        return Ok(d.log10().neg() as isize);
    }

    let mut precision = 0;
    for c in s.chars() {
        if c == '0' {
            precision += 1;
        } else if c == '.' {
            precision = 0;
        } else {
            break;
        }
    }
    Ok(precision + 1isize)
}

pub(crate) fn parse_float64(s: &str) -> Result<f64> {
    let d = f64::from_str(s)?;
    Ok(d)
}

#[cfg(test)]
mod test {
    #[test]
    fn test_into_precision() {
        assert_eq!(super::into_precision("0.00000001".to_string()).unwrap(), 8);
        assert_eq!(super::into_precision("0.001".to_string()).unwrap(), 3);
        assert_eq!(super::into_precision("10".to_string()).unwrap(), -1);
    }
}


pub(crate) fn timestamp_format(ts: i64, format: &str) -> Result<String> {
    match Utc.timestamp_millis_opt(ts) {
        chrono::LocalResult::None => Err(Error::InvalidTimestamp(ts)),
        chrono::LocalResult::Single(t) => Ok(t.format(format).to_string()),
        chrono::LocalResult::Ambiguous(_, _) => Err(Error::InvalidTimestamp(ts)),
    }
}