use config::{Config, ConfigError, Environment, File};
use log::info;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};

#[derive(Debug, Serialize, Deserialize)]
#[allow(unused)]
pub struct Gotmpl {
    pub binary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(unused)]
pub struct Pandoc {
    pub binary: Option<String>,
    pub reference_doc: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(unused)]
pub struct Settings {
    pub gotmpl: Gotmpl,
    pub pandoc: Pandoc,
}

impl Settings {
    pub fn new(path: &Path) -> Result<Self, ConfigError> {
        info!("Current directory: {}", path.to_string_lossy().into_owned());

        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            // Start off by merging in the "default" configuration file
            .add_source(File::with_name("config/default"))
            // Add in the current environment file
            // Default to 'development' env
            // Note that this file is _optional_
            .add_source(File::with_name(&format!("config/{}", run_mode)).required(false))
            // Add in a local configuration file
            // This file shouldn't be checked in to git
            .add_source(File::with_name("config/local").required(false))
            // Add in settings from the environment (with a prefix of APP)
            // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
            .add_source(Environment::with_prefix("app"))
            // You may also programmatically change settings
            // .set_override("database.url", "postgres://")?
            .add_source(
                File::with_name(path.to_string_lossy().into_owned().as_str()).required(false),
            )
            .build()?;

        // Now that we're done, let's access our configuration
        info!("debug: {:?}", s.get_bool("debug"));

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_deserialize()
    }

    pub fn save(&self, path: &Path) -> Result<&Self, ConfigError> {
        let toml_string = toml::to_string(self).expect("Could not encode TOML value");
        let prefix = path.parent().unwrap();
        fs::create_dir_all(prefix).unwrap();
        fs::write(path, toml_string).expect("failed to write configuration file!");

        Ok(self)
    }
}
