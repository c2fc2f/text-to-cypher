use std::{str::FromStr, sync::LazyLock};

use regex::Regex;

static FLOAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d+\.\d+").expect("Error during the regex creation")
});

/// Finds the first float in a string and parses it into a generic type T.
pub fn find_first_float<T: FromStr>(text: &str) -> Option<T> {
    FLOAT_RE
        .find(text)
        .and_then(|m| m.as_str().parse::<T>().ok())
}

/// Finds the last float in a string and parses it into a generic type T.
pub fn find_last_float<T: FromStr>(text: &str) -> Option<T> {
    FLOAT_RE
        .find_iter(text)
        .last()
        .and_then(|m| m.as_str().parse::<T>().ok())
}
