//! Explorer configuration

pub struct Config {
    pub theme: String,
}

impl Config {
    pub fn new() -> Self {
        Self {
            theme: "Papirus".to_string(),
        }
    }
}
