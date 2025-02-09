use anyhow::Context;
use dashmap::DashMap;
use grammers_client::types::User;

use grammers_client::{session::Session, Client, Config as TgConfig, InitParams};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::{Arc, LazyLock, OnceLock};

use crate::paths;

pub type ADashMap<K, V> = DashMap<K, V, ahash::RandomState>;

pub static CLIENTS_MAP: LazyLock<Arc<ADashMap<i64, Arc<AutoShitClient>>>> =
  LazyLock::new(|| Default::default());

pub static CONFIG: OnceLock<Arc<Config>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AutoShitClient {
  pub client: Client,
  pub me: User,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
  pub api_id: i32,
  pub api_hash: String,
}

impl Config {
  pub fn init() -> anyhow::Result<()> {
    let file = File::open(paths::CONFIG_FILE)
      .with_context(|| format!("Failed to open config file: {}", paths::CONFIG_FILE))?;
    let mut text = String::new();
    BufReader::new(file)
      .read_to_string(&mut text)
      .with_context(|| format!("Failed to read config from: {}", paths::CONFIG_FILE))?;
    let config = toml::from_str(&text).context("Failed to parse")?;
    CONFIG
      .set(Arc::new(config))
      .expect("Failed to set shared variable CONFIG");
    Ok(())
  }

  pub fn get() -> Arc<Config> {
    CONFIG.get().expect("Failed to get config").clone()
  }
}

pub async fn setup_client(session: Session) -> anyhow::Result<Client> {
  let config = Config::get();

  let client = Client::connect(TgConfig {
    session,
    api_id: config.api_id,
    api_hash: config.api_hash.clone(),
    params: InitParams {
      // Fetch the updates we missed while we were offline
      catch_up: true,
      ..Default::default()
    },
  })
  .await?;
  Ok(client)
}

pub async fn list_clients() -> String {
  CLIENTS_MAP
    .iter()
    .map(|r| {
      let (_, client) = r.pair();
      let name = client
        .me
        .username()
        .map(|u| format!("@{u}"))
        .unwrap_or_else(|| client.me.full_name());
      format!("{name} ({})", client.me.id())
    })
    .collect::<Vec<_>>()
    .join(", ")
}

pub fn get_clients_by_id(ids: &[i64]) -> anyhow::Result<Vec<Arc<AutoShitClient>>> {
  if ids.iter().any(|i| *i == 0) {
    let all = CLIENTS_MAP.iter().map(|i| i.value().clone()).collect();
    return Ok(all);
  }
  let mut vec = Vec::new();
  for id in ids {
    let refs = CLIENTS_MAP
      .get(id)
      .with_context(|| format!("There is no client with peer id = {id}"))?;
    vec.push(refs.value().clone());
  }
  Ok(vec)
}
