use anyhow::{ensure, Context};
use state::*;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Layer, Registry};

use grammers_client::session::Session;
use std::fs::OpenOptions;
use walkdir::WalkDir;

mod cmds;
mod console;
mod state;

mod paths {
  use std::path::PathBuf;

  pub const SESSIONS_FOLDER: &str = "sessions";
  pub const HISTORY_FILE: &str = ".auto-shit-history";
  pub const CONFIG_FILE: &str = "auto-shit.toml";

  pub fn session(id: i64) -> PathBuf {
    [SESSIONS_FOLDER, &id.to_string()].iter().collect()
  }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  
  Config::init()?;
  setup_subscriber()?;

  println!("Loading telegram sessions");
  let mut load_jobs = Vec::new();
  for entry in WalkDir::new(paths::SESSIONS_FOLDER).max_depth(1) {
    let process = async {
      let entry = entry.context("Failed to read dir")?;
      let metadata = entry.metadata().context("File has no metadata")?;
      if !metadata.is_file() {
        return Ok(());
      }

      let Some(peer_id) = entry
        .file_name()
        .to_str()
        .context("File name is not a valid str")?
        .parse::<i64>()
        .ok()
      else {
        return Ok(());
      };

      let client =
        setup_client(Session::load_file(entry.path()).context("Failed to load session")?)
          .await
          .context("Failed to setup telegram client")?;

      ensure!(
        client
          .is_authorized()
          .await
          .context("Failed to check if authorized")?,
        "{peer_id} is not authorized"
      );

      let me = client.get_me().await.context("Failed to call get me")?;
      ensure!(me.id() == peer_id, "session peer id is broken");

      CLIENTS_MAP.insert(me.id(), AutoShitClient { client, me }.into());

      anyhow::Ok(())
    };
    let load_job = tokio::spawn(process);
    load_jobs.push(load_job);
  }
  for job in load_jobs {
    if let Err(err) = job.await.context("Failed to await job")? {
      tracing::error!("Failed to load client session: {err}")
    }
  }

  println!(
    "{} telegram accounts are logged in: {}",
    CLIENTS_MAP.len(),
    list_clients().await,
  );

  let console = console::console().await?;

  console.job.await?;

  Ok(())
}

fn setup_subscriber() -> anyhow::Result<()> {
  let env_filter = EnvFilter::builder()
    .with_default_directive(LevelFilter::INFO.into())
    .with_env_var("AUTO_SHIT_LOG")
    .from_env()
    .context("Failed to parse `AUTO_SHIT_LOG` environment variable")?;
  let log = OpenOptions::new()
    .append(true)
    .create(true)
    .open("autoshit.log")?;
  println!("Log saved at: autoshit.log");
  let subscriber = Registry::default().with(
    tracing_subscriber::fmt::layer()
      .with_writer(log)
      .with_filter(env_filter),
  );
  tracing::subscriber::set_global_default(subscriber)?;
  Ok(())
}
