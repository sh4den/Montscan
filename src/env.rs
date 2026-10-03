use std::str::FromStr;

pub fn get(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

pub fn get_either(primary: &str, fallback: &str) -> Option<String> {
    get(primary).or_else(|| get(fallback))
}

pub fn string(key: &str, default: &str) -> String {
    get(key).unwrap_or_else(|| default.to_string())
}

pub fn flag(key: &str, default: bool) -> bool {
    get(key).map_or(default, |v| v == "true")
}

pub fn parse<T: FromStr>(key: &str, default: T) -> T {
    get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}
