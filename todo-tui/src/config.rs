use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use keyring::Entry;
use serde::{Deserialize, Serialize};
use todo_client::{generate_recovery_phrase, normalize_phrase_for_storage};

const DEFAULT_ENDPOINT: &str = "api.todo.omfj.no";
const KEYCHAIN_SERVICE: &str = "todo";
const KEYCHAIN_ACCOUNT: &str = "phrase";
const DEV_KEYCHAIN_ACCOUNT: &str = "phrase_dev";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub endpoint: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_string(),
        }
    }
}

pub struct AppConfig {
    pub endpoint: String,
    pub phrase: String,
}

pub fn load_or_create_config() -> anyhow::Result<AppConfig> {
    let path = config_path()?;
    let mut config = read_config(&path)?;
    let mut changed = false;

    if config.general.endpoint.trim().is_empty() {
        config.general.endpoint = DEFAULT_ENDPOINT.to_string();
        changed = true;
    }

    if let Ok(endpoint) = std::env::var("ENDPOINT_URL") {
        config.general.endpoint = endpoint;
        changed = true;
    }

    if changed || !path.exists() {
        write_config(&path, &config)?;
    }

    let phrase = load_or_create_phrase(is_development())?;

    Ok(AppConfig {
        endpoint: config.general.endpoint,
        phrase,
    })
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

fn load_or_create_phrase(is_development: bool) -> anyhow::Result<String> {
    if let Ok(phrase) = std::env::var("TODO_PHRASE") {
        return normalize_phrase_for_storage(&phrase)
            .ok_or_else(|| anyhow::anyhow!("TODO_PHRASE must be a valid 12-word phrase"));
    }

    if let Some(phrase) = load_phrase_from_keychain(is_development)?
        && let Some(phrase) = normalize_phrase_for_storage(&phrase)
    {
        return Ok(phrase);
    }

    let phrase = prompt_for_recovery_phrase()?;
    store_phrase_in_keychain(is_development, &phrase)?;
    Ok(phrase)
}

fn load_phrase_from_keychain(is_development: bool) -> anyhow::Result<Option<String>> {
    let entry = Entry::new(KEYCHAIN_SERVICE, keychain_account(is_development))?;
    match entry.get_password() {
        Ok(phrase) => Ok(Some(phrase)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn store_phrase_in_keychain(is_development: bool, phrase: &str) -> anyhow::Result<()> {
    let entry = Entry::new(KEYCHAIN_SERVICE, keychain_account(is_development))?;
    entry.set_password(phrase)?;
    Ok(())
}

fn prompt_for_recovery_phrase() -> anyhow::Result<String> {
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
    println!("Saved locally in the OS keychain for this device.");
    print!("Press Enter to continue...");
    io::stdout().flush()?;

    input.clear();
    io::stdin().read_line(&mut input)?;

    Ok(phrase)
}

fn config_file_name() -> &'static str {
    if is_development() {
        "config.dev.toml"
    } else {
        "config.toml"
    }
}

fn config_path() -> anyhow::Result<PathBuf> {
    let filename = config_file_name();
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

fn keychain_account(is_development: bool) -> &'static str {
    if is_development {
        DEV_KEYCHAIN_ACCOUNT
    } else {
        KEYCHAIN_ACCOUNT
    }
}

fn is_development() -> bool {
    std::env::var("DEVELOPMENT")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false)
}
