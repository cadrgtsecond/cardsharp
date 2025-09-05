use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::PathBuf,
    process::{Command, Stdio},
};

mod fsrs;

#[derive(Debug, Serialize, Deserialize)]
pub struct CardContent {
    id: u64,
    title: String,
    body: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Card {
    content: CardContent,
    fsrs_params: fsrs::FSRSParams,
}

pub fn data_path() -> anyhow::Result<PathBuf> {
    let mut home = PathBuf::from(std::env::var("HOME")?);
    home.push(".local/share/cardsharp");

    Ok(std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home))
}

pub fn find_card_content() -> anyhow::Result<Vec<CardContent>> {
    // TODO: Support other searching commands, such as `grep -R` or `ag`
    let grep_result = Command::new("rg")
        .arg("-g")
        .arg("*.{md,adoc,txt}")
        .arg("-0b")
        .arg("REVIEW:")
        .stdin(Stdio::null())
        .output()?;
    let grep_result = String::from_utf8_lossy(&grep_result.stdout);
    let data = grep_result
        .split('\n')
        .filter_map(|s| {
            let (filename, rest) = s.split_once('\0')?;
            let (byte_off_str, _content) = rest.split_once(":")?;

            Some((PathBuf::from(filename), byte_off_str.parse().ok()?))
        });

    let mut acc = None;
    for (cardpath, off) in data {
        // This trickery is to avoid opening the same file multiple times
        let (mut file, filename) = match acc {
            // filepath == filename
            Some((file @ _, filepath)) => (file, filepath),
            _ => (BufReader::new(File::open(&cardpath)?), cardpath.clone()),
        };

        file.seek(SeekFrom::Start(off))?;
        let mut line = String::new();
        file.read_line(&mut line)?;

        let mut content = String::new();
        loop {
            let mut next_line = String::new();
            file.read_line(&mut next_line);
            if next_line.starts_with("REVIEW:") || next_line == "---\n" || next_line == "" {
                break;
            }
            content.push_str(&next_line);
        }

        println!("{line}<<<<\n{content}---");

        acc = Some((file, filename));
    };
    Ok(vec![])
}

/// Intialize state, returning a list of flashcards
pub fn initialize_cards() -> anyhow::Result<Vec<Card>> {
    let data_path = data_path()?;
    std::fs::create_dir_all(&data_path)?;

    let mut card_data = data_path.clone();
    card_data.push("cards.json");
    _ = card_data;
    let content = find_card_content();

    Ok(vec![])
}

fn main() -> anyhow::Result<()> {
    let cards = initialize_cards()?;

    println!("Hello, world! {:?}", cards);
    Ok(())
}
