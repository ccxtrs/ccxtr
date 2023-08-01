use std::ops::Neg;
use crate::Result;
use std::str::FromStr;

pub(in crate) fn into_precision(s: String) -> Result<isize> {
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
    Ok(precision + 1 as isize)
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