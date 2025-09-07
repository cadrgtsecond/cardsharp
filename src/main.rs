#![deny(clippy::pedantic)]

use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, hash_map::Entry},
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

use crate::fsrs::Grade;

mod base64;
mod fsrs;
mod ui;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Card {
    id: u64,
    #[serde(skip)]
    title: String,
    #[serde(skip)]
    body: String,
}

fn data_path() -> anyhow::Result<PathBuf> {
    let mut home = PathBuf::from(std::env::var("HOME")?);
    home.push(".local/share/cardsharp");

    Ok(std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home))
}

/// Parses a title, returning the base64 id and the actual title
fn parse_title(mut title: &str) -> Option<(u64, &str)> {
    if title.starts_with("REVIEW:") {
        title = &title[7..];
    }
    title = title.trim_start();
    if !title.starts_with("__") {
        return None;
    }
    title = &title[2..];
    let (id, title) = title.split_once(char::is_whitespace)?;
    Some((base64::from_base64(id.as_bytes().try_into().ok()?)?, title))
}

fn find_cards() -> anyhow::Result<Vec<Card>> {
    // TODO: Support other searching commands, such as `grep -R` or `ag`
    let grep_result = Command::new("rg")
        .arg("-g")
        .arg("*.{md,adoc,txt}")
        .arg("-0b")
        .arg("REVIEW:")
        .stdin(Stdio::null())
        .output()?;
    let grep_result = String::from_utf8_lossy(&grep_result.stdout);
    let mut data = grep_result.split('\n').filter_map(|s| {
        let (filename, rest) = s.split_once('\0')?;
        let (byte_off_str, _) = rest.split_once(':')?;

        Some((Path::new(filename), byte_off_str.parse::<u64>().ok()?))
    });

    let Some((mut prev_path, _)) = data.next() else {
        return Ok(vec![]);
    };
    let mut file = BufReader::new(File::open(prev_path)?);
    let mut written = 0;
    data.map(|(cardpath, off)| -> anyhow::Result<_> {
        if cardpath != prev_path {
            prev_path = cardpath;
            written = 0;
            file = BufReader::new(File::open(cardpath)?);
        }

        file.seek(SeekFrom::Start(written + off))?;
        let mut title = String::new();
        file.read_line(&mut title)?;

        let mut body = String::new();
        loop {
            let mut next_line = String::new();
            file.read_line(&mut next_line)?;
            if next_line.starts_with("REVIEW:") || next_line == "---\n" || next_line.is_empty() {
                break;
            }
            body.push_str(&next_line);
        }

        let (id, title) = if let Some((id, title)) = parse_title(&title) {
            (id, String::from(title))
        } else {
            // TODO: Implement automated testing of adding ID's
            println!("New card in found in {}", cardpath.to_string_lossy());
            let id: u64 = rand::random();
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(cardpath)?;
            let mut rest = String::new();
            file.seek(SeekFrom::Start(written + off + 7))?;
            file.read_to_string(&mut rest)?;

            file.seek(SeekFrom::Start(written + off + 7))?;
            written += file.write(b" __")? as u64;
            written += file.write(&base64::to_base64(id))? as u64;
            _ = file.write(rest.as_bytes())?;
            (
                id,
                String::from(&rest[0..rest.find('\n').unwrap_or(rest.len())]),
            )
        };
        Ok(Card { id, title, body })
    })
    .collect()
}

#[derive(Debug, Serialize, Deserialize)]
struct CardParams {
    pub last_review: SystemTime,
    pub fsrs: fsrs::FSRSParams,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Data {
    #[serde(default)]
    fsrs_params: HashMap<String, CardParams>,
}

pub fn open_data() -> anyhow::Result<File> {
    let mut path = data_path()?;
    std::fs::create_dir_all(&path)?;

    path.push("cards.json");
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?)
}

pub fn load_data(file: &mut File) -> anyhow::Result<Data> {
    Ok(serde_json::from_reader(file).unwrap_or_else(|_| Data::default()))
}
pub fn save_data(file: &mut File, data: &Data) -> anyhow::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    serde_json::to_writer(file, data)?;
    Ok(())
}

#[derive(Debug, Parser)]
#[command(version)]
enum Commands {
    /// Review all cards due
    Review {
        /// Target retention for study
        ///
        /// Clamped between 0.0 and 1.0
        #[arg(short, long, default_value = "0.9")]
        retention: f32,
    },

    /// Initializes all cards under the current directory, state, and config
    ///
    /// This is equivalent to using `review` and immediately quitting.
    /// Used mainly to add id's after `REVIEW:` without doing it manually
    Init {},
}

fn main() -> anyhow::Result<()> {
    let command = Commands::parse();
    match command {
        Commands::Init {} => {
            let _cards = find_cards()?;
            let _file = open_data()?;
        }
        Commands::Review { retention } => {
            let cards = find_cards()?;
            let mut file = open_data()?;
            let mut data = load_data(&mut file)?;

            execute!(std::io::stdout(), EnterAlternateScreen)?;
            crossterm::terminal::enable_raw_mode()?;

            loop {
                let mut iters = 0;
                for card in &cards {
                    let id = str::from_utf8(&base64::to_base64(card.id))
                        .expect("This is always valid utf8")
                        .to_string();
                    match data.fsrs_params.entry(id) {
                        Entry::Occupied(mut entry) => {
                            let CardParams { last_review, fsrs } = entry.get_mut();
                            // We add one extra day so that we don't have to review after a short time
                            let days_elapsed =
                                1.0 + last_review.elapsed()?.as_secs_f32() / (60.0 * 60.0 * 24.0);

                            if fsrs.recall_probability(days_elapsed) >= retention {
                                continue;
                            }
                            iters += 1;
                            let grade = ui::review_card(&card)?;

                            if grade != Grade::Again {
                                *fsrs = fsrs.update_successful(grade);
                            }
                        }
                        Entry::Vacant(entry) => {
                            iters += 1;
                            let params = ui::review_first_time(&card)?;
                            entry.insert(params);
                        }
                    }
                }
                if iters == 0 {
                    break;
                }
            }

            crossterm::terminal::disable_raw_mode()?;
            execute!(std::io::stdout(), LeaveAlternateScreen)?;
            save_data(&mut file, &data)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unreadable_literal)]
mod tests {
    use super::*;
    #[test]
    pub fn parsing_title() {
        // Valid formats
        assert_eq!(
            parse_title("REVIEW:__+KkJkFEm3+M test"),
            Some((17917863107911671779, " test"))
        );
        assert_eq!(
            parse_title("REVIEW:  __ayBz0QJqjYk"),
            Some((7719297102838926729, ""))
        );
        assert_eq!(
            parse_title("__/nr0HfQpvoM test"),
            Some((18337237242280001155, "test"))
        );

        // Invalid formats
        assert_eq!(parse_title("db02tXj37Co"), None);
        assert_eq!(parse_title("__9OgKjjs"), None);
    }
}
