use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use todo_client::{generate_recovery_phrase, normalize_phrase_for_storage};

const DEFAULT_ENDPOINT: &str = "api.todo.omfj.no";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub phrase: String,
    pub endpoint: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            phrase: String::new(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
        }
    }
}

pub fn load_or_create_config() -> anyhow::Result<Config> {
    let path = config_path()?;
    let mut config = read_config(&path)?;
    let mut changed = false;

    if config.general.endpoint.trim().is_empty() {
        config.general.endpoint = DEFAULT_ENDPOINT.to_string();
        changed = true;
    }

    if config.general.phrase.trim().is_empty()
        || normalize_phrase_for_storage(&config.general.phrase).is_none()
    {
        config.general.phrase = prompt_for_recovery_phrase(&path)?;
        changed = true;
    } else if let Some(phrase) = normalize_phrase_for_storage(&config.general.phrase)
        && phrase != config.general.phrase
    {
        config.general.phrase = phrase;
        changed = true;
    }

    if changed || !path.exists() {
        write_config(&path, &config)?;
    }

    Ok(config)
}

fn read_config(path: &PathBuf) -> anyhow::Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }

    let contents = fs::read_to_string(path)?;
    toml::from_str(&contents).map_err(Into::into)
}

fn write_config(path: &PathBuf, config: &Config) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, toml::to_string_pretty(config)?)?;
    Ok(())
}

fn prompt_for_recovery_phrase(path: &PathBuf) -> anyhow::Result<String> {
    println!("Paste an existing recovery phrase, or press Enter to generate a new one:");
    print!("> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let phrase = input.trim().to_string();
    if !phrase.is_empty() {
        return normalize_phrase_for_storage(&phrase)
            .ok_or_else(|| anyhow::anyhow!("recovery phrase must be a valid 12-word phrase"));
    }

    let phrase = generate_recovery_phrase();

    println!("Recovery phrase generated for local encryption:\n");
    println!("{phrase}\n");
    println!(
        "Store this phrase somewhere safe. You need it to decrypt this data on another device."
    );
    println!("Saved locally in {} for this device.", path.display());
    print!("Press Enter to continue...");
    io::stdout().flush()?;

    input.clear();
    io::stdin().read_line(&mut input)?;

    Ok(phrase)
}

fn config_path() -> anyhow::Result<PathBuf> {
    let is_development = std::env::var("DEVELOPMENT")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);

    let filename = if is_development {
        "config.dev.toml"
    } else {
        "config.toml"
    };
    Ok(xdg_config_home()?.join("todo").join(filename))
}

fn xdg_config_home() -> anyhow::Result<PathBuf> {
    if let Ok(path) = std::env::var("XDG_CONFIG_HOME")
        && !path.trim().is_empty()
    {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var("HOME")?;
    Ok(PathBuf::from(home).join(".config"))
}
