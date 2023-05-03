use std::ops::Neg;
use rust_decimal::Decimal;
use crate::Result;
use rust_decimal::prelude::ToPrimitive;
use std::str::FromStr;
use rust_decimal::MathematicalOps;
use crate::Error::ParseError;

pub(in crate) fn into_precision(s: String) -> Result<isize> {
    let d = Decimal::from_str(&s)?;

    if d.gt(&Decimal::ONE){
        match d.log10().neg().to_isize() {
            Some(p) => return Ok(p),
            None => return Err(ParseError("Failed to convert precision".to_string()))
        }
    }
    Ok(d.scale() as isize)
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