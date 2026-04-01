use std::str::FromStr;

use regex::Regex;

/// Finds the first float in a string and parses it into a generic type T.
pub fn find_first_float<T>(text: &str) -> Option<T>
where
    T: FromStr,
{
    Regex::new(r"[-+]?\d*\.?\d+([eE][-+]?\d+)?")
        .expect("Error during the regex creation")
        .find(text)
        .and_then(|m| m.as_str().parse::<T>().ok())
}
