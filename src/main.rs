use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, hash_map::Entry},
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Stdin, Stdout, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::SystemTime,
};

use crate::fsrs::{FSRSParams, Grade};

mod base64;
mod fsrs;

#[derive(Debug, Serialize, Deserialize)]
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

fn initialize_cards() -> anyhow::Result<Vec<Card>> {
    // TODO: Support other searching commands, such as `grep -R` or `ag`
    let grep_result = Command::new("rg")
        .arg("-g")
        .arg("*.{md,adoc,txt}")
        .arg("-0b")
        .arg("REVIEW:")
        .stdin(Stdio::null())
        .output()?;
    let grep_result = String::from_utf8_lossy(&grep_result.stdout);
    let data = grep_result.split('\n').filter_map(|s| {
        let (filename, rest) = s.split_once('\0')?;
        let (byte_off_str, _content) = rest.split_once(":")?;

        Some((PathBuf::from(filename), byte_off_str.parse::<u64>().ok()?))
    });

    let mut written = 0;
    let mut prev_path = None;
    // ⊔ would make my life a whole lot easier
    // TODO: Avoid opening the same file multiple times
    data.map(|(cardpath, off)| -> anyhow::Result<_> {
        let mut file = BufReader::new(File::open(&cardpath)?);
        if Some(&cardpath) != prev_path.as_ref() {
            written = 0;
            prev_path = Some(cardpath.clone());
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

        let (id, title) = match parse_title(&title) {
            Some((id, title)) => (id, String::from(title)),
            None => {
                // TODO: Implement automated testing of adding ID's
                println!("New card in found in {}", cardpath.to_string_lossy());
                let id: u64 = rand::random();
                let mut file = OpenOptions::new().read(true).write(true).create(true).open(&cardpath)?;
                let mut rest = String::new();
                file.seek(SeekFrom::Start(written + off + 7))?;
                file.read_to_string(&mut rest)?;

                file.seek(SeekFrom::Start(written + off + 7))?;
                written += file.write(b" __")? as u64;
                written += file.write(&base64::to_base64(id))? as u64;
                file.write(rest.as_bytes())? as u64;
                (
                    id,
                    String::from(&rest[0..rest.find('\n').unwrap_or(rest.len())]),
                )
            }
        };
        Ok(Card { id, title, body })
    })
    .collect()
}

#[derive(Debug, Serialize, Deserialize)]
struct CardParams {
    last_review: SystemTime,
    fsrs: fsrs::FSRSParams,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Data {
    #[serde(default)]
    fsrs_params: HashMap<String, CardParams>,
}

#[derive(Debug)]
struct State {
    data: Data,
    data_file: File,
    cards: Vec<Card>,
}

impl State {
    fn new() -> anyhow::Result<State> {
        let mut data_path = data_path()?;
        std::fs::create_dir_all(&data_path)?;

        data_path.push("cards.json");

        let data_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(data_path)?;
        let cards = initialize_cards()?;
        let data = serde_json::from_reader(&data_file).unwrap_or_else(|_| Data::default());

        Ok(State {
            data,
            data_file,
            cards,
        })
    }

    fn save(&mut self) -> anyhow::Result<()> {
        self.data_file.seek(SeekFrom::Start(0))?;
        serde_json::to_writer(&mut self.data_file, &self.data)?;
        Ok(())
    }
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

fn review_card(card: &Card) -> anyhow::Result<Grade> {
    let mut stdin = BufReader::new(std::io::stdin());
    let mut stdout = std::io::stdout();

    print!("{}\nPress enter to show backside...", card.title.trim());
    stdout.flush()?;

    let mut buf = String::new();
    stdin.read_line(&mut buf)?;
    _ = buf;

    println!("{}", card.body.trim());
    println!("1:again\t2: hard\t3/space: good\t4: easy\nEnter grade:");
    std::thread::sleep(std::time::Duration::from_secs(2));

    let mut buf = String::new();
    stdin.read_line(&mut buf)?;
    Ok(match buf.trim() {
        "1" => Grade::Again,
        "2" => Grade::Hard,
        "3" => Grade::Good,
        "4" => Grade::Easy,
        _ => Grade::Good,
    })
}

fn review_again(
    CardParams { last_review, fsrs }: &mut CardParams,
    card: &Card,
    retention: f32,
) -> anyhow::Result<()> {
    let days_elapsed = last_review.elapsed()?.as_secs_f32() / (60.0 * 60.0 * 24.0);
    println!("{}", days_elapsed);
    let r = fsrs.recall_probability(days_elapsed);
    println!("{}", r);
    if r < retention {
        let grade = review_card(card)?;
        if grade as u8 > 1 {
            *fsrs = fsrs.update_successful(grade);
        }
    };
    Ok(())
}

fn review_first_time(card: &Card) -> anyhow::Result<CardParams> {
    let grade = review_card(card)?;

    Ok(CardParams {
        last_review: SystemTime::now(),
        fsrs: FSRSParams::from_initial_grade(grade),
    })
}

fn main() -> anyhow::Result<()> {
    let command = Commands::parse();
    match command {
        Commands::Init {} => {
            let mut state = State::new()?;
            state.save()?;
        }
        Commands::Review { retention } => {
            let mut state = State::new()?;
            for card in &state.cards {
                let id = str::from_utf8(&base64::to_base64(card.id))
                    .expect("This is always valid utf8")
                    .to_string();
                match state.data.fsrs_params.entry(id) {
                    Entry::Occupied(mut entry) => {
                        review_again(entry.get_mut(), &card, retention.clamp(0.0, 1.0));
                    }
                    Entry::Vacant(entry) => {
                        let params = review_first_time(&card)?;
                        println!("{:?}", params);
                        entry.insert(params);
                    }
                }
            }
            state.save()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    pub fn parsing_title() {
        // Valid formats
        assert_eq!(
            parse_id_from_title("REVIEW:__+KkJkFEm3+M"),
            Some(17917863107911671779)
        );
        assert_eq!(
            parse_id_from_title("REVIEW:  __ayBz0QJqjYk"),
            Some(7719297102838926729)
        );
        assert_eq!(
            parse_id_from_title("__/nr0HfQpvoM"),
            Some(18337237242280001155)
        );

        // Invalid formats
        assert_eq!(parse_id_from_title("db02tXj37Co"), None);
        assert_eq!(parse_id_from_title("__9OgKjjs"), None);
    }
}
