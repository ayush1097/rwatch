use serde::Deserialize;
use std::fs;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub refresh_interval: String,
    pub process_interval: String,
    pub network_interval: String,
    pub log_level: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string("rwatch.toml")?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
    pub fn process_duration(&self) -> Result<Duration, Box<dyn std::error::Error>> {
        Ok(humantime::parse_duration(&self.process_interval)?)
    }
    pub fn network_duration(&self) -> Result<Duration, Box<dyn std::error::Error>> {
        Ok(humantime::parse_duration(&self.network_interval)?)
    }
    pub fn refresh_duration(&self) -> Result<Duration, Box<dyn std::error::Error>> {
        Ok(humantime::parse_duration(&self.refresh_interval)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_toml_config() {
        let toml_str = r#"
            refresh_interval = "1s"
            process_interval = "500ms"
            network_interval = "2s"
            log_level = "info"
        "#;
        let config: Config = toml::from_str(toml_str).expect("Valid config should parse");
        assert_eq!(config.refresh_interval, "1s");
        assert_eq!(config.process_interval, "500ms");
        assert_eq!(config.network_interval, "2s");
        assert_eq!(config.log_level, "info");

        assert_eq!(config.refresh_duration().unwrap(), Duration::from_secs(1));
        assert_eq!(
            config.process_duration().unwrap(),
            Duration::from_millis(500)
        );
        assert_eq!(config.network_duration().unwrap(), Duration::from_secs(2));
    }

    #[test]
    fn fails_on_invalid_duration_format() {
        let toml_str = r#"
            refresh_interval = "invalid_time"
            process_interval = "1s"
            network_interval = "1s"
            log_level = "debug"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.refresh_duration().is_err());
    }

    #[test]
    fn fails_on_missing_required_field() {
        let toml_str = r#"
            refresh_interval = "1s"
            process_interval = "1s"
            network_interval = "1s"
        "#;
        let result: Result<Config, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn fails_on_invalid_toml_syntax() {
        let toml_str = "invalid = [toml unclosed";
        let result: Result<Config, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }
}
