use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    path::PathBuf,
    process::{Command, Stdio},
};

mod fsrs;

#[derive(Debug, Serialize, Deserialize)]
pub struct Card {
    id: u64,
    fsrs_params: fsrs::FSRSParams,
}

pub fn data_path() -> anyhow::Result<PathBuf> {
    let mut home = PathBuf::from(std::env::var("HOME")?);
    home.push(".local/share/cardsharp");

    Ok(std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home))
}

/// Intialize state, returning a list of flashcards
pub fn initialize_cards() -> anyhow::Result<Vec<Card>> {
    let data_path = data_path()?;
    std::fs::create_dir_all(&data_path)?;

    let mut card_data = data_path.clone();
    card_data.push("cards.json");
    _ = card_data;

    let actual_cards = Command::new("rg")
        .arg("-g")
        .arg("*.{md,adoc,txt}")
        .arg("-0b")
        .arg("REVIEW:")
        .stdin(Stdio::null())
        .output()?;
    let content = String::from_utf8_lossy(&actual_cards.stdout);
    let data: Vec<_> = content
        .split('\n')
        .filter_map(|s| {
            let (filename, rest) = s.split_once('\0')?;
            let (byte_off_str, content) = rest.split_once(":")?;
            let byte_off: u64 = byte_off_str.parse().ok()?;
            Some((filename, byte_off, content))
        })
        .collect();

    Ok(vec![])
}

fn main() -> anyhow::Result<()> {
    let cards = initialize_cards()?;

    println!("Hello, world! {:?}", cards);
    Ok(())
}
