use serde::{Deserialize, Serialize};
use std::{fs::File, path::PathBuf};

mod fsrs;

#[derive(Debug, Serialize, Deserialize)]
struct Card {
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

fn main() -> anyhow::Result<()> {
    let data_path = data_path()?;
    std::fs::create_dir_all(&data_path)?;

    let mut card_data = data_path.clone();
    card_data.push("cards.json");

    let card_file = File::open(card_data)?;
    let cards = serde_json::from_reader::<_, Vec<Card>>(card_file);
    println!("{:?}", cards);

    println!("Hello, world! {:?}", data_path);
    Ok(())
}
