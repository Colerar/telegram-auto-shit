use rustyline::{error::ReadlineError, CompletionType, Config, DefaultEditor, EditMode};

use anyhow::Context;
use tokio::task::JoinHandle;

use std::{
  fs::File,
  io,
  io::{BufRead, Write},
};

use crate::{cmds::handle_command, paths};

pub struct Console {
  pub job: JoinHandle<bool>,
}

pub async fn console() -> anyhow::Result<Console> {
  let config = Config::builder()
    .history_ignore_space(true)
    .completion_type(CompletionType::List)
    .edit_mode(EditMode::Emacs)
    .build();

  let mut rl = DefaultEditor::with_config(config).context("Failed to create editor")?;
  match File::create_new(paths::HISTORY_FILE) {
    Ok(_) => {},
    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}, // file exists, ignore
    Err(e) => return Err(e).with_context(|| format!("Failed to create `{}`", paths::HISTORY_FILE)),
  };
  rl.load_history(paths::HISTORY_FILE)
    .with_context(|| format!("Failed to load `{}`", paths::HISTORY_FILE))?;

  let job = tokio::spawn(async move {
    let p = format!("> ");
    loop {
      let readline = rl.readline(&p);
      match readline {
        Ok(line) => {
          match shlex::split(&line) {
            Some(args) => {
              if args.is_empty() {
                println!("Empty line");
                continue;
              }
              if let Err(err) = handle_command(&args).await {
                println!("Failed to parse command: {err:?}")
              }
            },
            None => {
              println!("Failed to split");
            },
          }
          if let Err(err) = rl.add_history_entry(line.as_str()) {
            eprintln!("add_history_entry failed: {err:?}");
          }
        },
        Err(ReadlineError::Interrupted) => {
          println!("^C Interrupted");
          continue;
        },
        Err(ReadlineError::Eof) => {
          return true;
        },
        Err(err) => {
          eprintln!("Failed to read line, error: {err:?}");
          return false;
        },
      }
      if let Err(err) = rl.save_history(".auto-shit-history") {
        eprintln!("save_history failed: {err:?}");
      }
    }
  });

  Ok(Console { job })
}

pub fn prompt(message: &str) -> anyhow::Result<String> {
  let stdout = io::stdout();
  let mut stdout = stdout.lock();
  stdout.write_all(message.as_bytes())?;
  stdout.flush()?;

  let stdin = io::stdin();
  let mut stdin = stdin.lock();

  let mut line = String::new();
  stdin.read_line(&mut line)?;
  Ok(line)
}
