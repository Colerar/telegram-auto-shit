use crate::state::*;
use crate::{console::*, paths, state::CLIENTS_MAP};
use anyhow::{ensure, Context};
use clap::Parser;
use grammers_client::types::InputReactions;

use grammers_client::session::Session;
use std::fs::{self, File};

#[derive(Debug, Parser)]
#[clap(rename_all = "kebab-case")]
#[command(name = "")] // This name will show up in clap's error messages, so it is important to set it to "".
enum Commands {
  /// Login into Telegram with phone number verification.
  Login {
    /// Optional. You will be prompted if you don't provide it.
    #[arg(short, long)]
    phone: Option<String>,
  },
  /// Add reaction to messages
  BatchReaction(BatchReaction),
}

#[derive(Debug, Parser)]
pub struct BatchReaction {
  /// Channel username
  channel: String,
  /// Reaction string, usually a emoji
  #[arg(short, long, default_value = "🤡")]
  reaction: String,
  /// Offset message ID
  #[arg(short, long)]
  start: Option<i32>,
  /// Limitation of message count
  #[arg(short, long)]
  limit: Option<usize>,
  /// Client peer id
  #[arg(short, long, value_delimiter = ' ', num_args = 0..)]
  client: Vec<i64>,
}

pub async fn handle_command(args: &[String]) -> anyhow::Result<()> {
  let args = std::iter::once("").chain(args.iter().map(String::as_str));
  let c = Commands::try_parse_from(args)?;
  match c {
    Commands::Login { phone } => login_command(phone).await?,
    Commands::BatchReaction(arg) => batch_reaction_command(arg).await?,
  };
  Ok(())
}

async fn login_command(phone: Option<String>) -> anyhow::Result<()> {
  let phone = match phone {
    Some(phone) => phone,
    None => prompt("Enter phone number: ")?,
  };

  println!("Connecting to Telegram...");
  let client = setup_client(Session::new()).await?;
  println!("Connected!");
  if !client.is_authorized().await? {
    println!("Signing in...");
    tracing::info!("Signing in for {phone}");
    let token = client
      .request_login_code(&phone)
      .await
      .context("Failed to request login code")?;

    let code = prompt("Enter code: ")?;

    client
      .sign_in(&token, &code)
      .await
      .context("Failed to sign in")?;
    println!("Saving session file and exiting...");
    let me = client.get_me().await.context("Failed to call get me api")?;
    let path = paths::session(me.id());
    if let Some(parent) = path.parent() {
      if !parent.exists() {
        fs::create_dir_all(parent)
          .with_context(|| format!("Failed to create folder `{parent:?}`"))?;
      } else {
        ensure!(parent.is_dir(), "`{parent:?}` is not a valid dir path",);
      }
    }
    File::create(&path).with_context(|| format!("Failed to create file: {path:?}"))?;
    client.session().save_to_file(path)?;
    println!("Signed in!");
    tracing::info!("Signing in successfully peer id: {}", me.id());
  }

  Ok(())
}

async fn batch_reaction_command(arg: BatchReaction) -> anyhow::Result<()> {
  ensure!(
    !CLIENTS_MAP.is_empty(),
    "No account for operation, please login first"
  );
  let clients = if arg.client.is_empty() && CLIENTS_MAP.len() == 1 {
    vec![CLIENTS_MAP.iter().next().unwrap().clone()]
  } else {
    get_clients_by_id(&arg.client)?
  };
  ensure!(
    !clients.is_empty(),
    "Please specify which account to operate"
  );
  for client in clients {
    let client = &client.client;
    let maybe_chat = client.resolve_username(&arg.channel).await?;

    let Some(chat) = maybe_chat else {
      anyhow::bail!("Chat not found");
    };

    let mut messages = client.iter_messages(&chat);
    if let Some(start) = arg.start {
      messages = messages.offset_id(start);
    }
    if let Some(limit) = arg.limit {
      messages = messages.limit(limit);
    }
    while let Some(message) = messages.next().await? {
      client
        .send_reactions(&chat, message.id(), InputReactions::emoticon("🤡"))
        .await?;
      println!("Sent reaction to message {}", message.id());
      tokio::time::sleep(tokio::time::Duration::from_millis(rand::random_range(
        1000..=20000,
      )))
      .await;
    }
  }

  Ok(())
}
