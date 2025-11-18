#![deny(clippy::pedantic)]

use base64::{Engine, prelude::BASE64_STANDARD};
use clap::Parser;
use crossterm::{
    execute,
    style::Stylize,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use crate::fsrs::FSRSParams;

mod fsrs;
mod ui;

/// Cards have 6 byte identifiers.
/// This is so that they can be conveniently represented in base64 as 8 characters
#[derive(Debug, Copy, Clone)]
struct CardId(pub [u8; 6]);

impl CardId {
    fn as_int(self) -> u64 {
        let mut res = 0;
        for b in self.0 {
            res |= u64::from(b);
            res <<= 8;
        }
        res
    }
}

struct CardBody {
    id: CardId,
    front: String,
    back: String,
}

/// Initializes any uninitialized cards with their own Id.
/// Returns a list of Ids
fn initialize_card_bodies(data: &mut String) -> Vec<CardId> {
    let is: Vec<usize> = data
        .lines()
        .filter(|i| i.starts_with("REVIEW:"))
        .map(|i| (i.as_ptr() as usize) - (data.as_ptr() as usize))
        .collect();
    is.iter()
        .rev()
        .map(|i| {
            let newid = CardId(rand::random());
            data.insert_str(*i + "REVIEW".len(), "--");
            data.insert_str(*i + "REVIEW--".len(), &BASE64_STANDARD.encode(newid.0));
            newid
        })
        .collect()
}

/// Loads cards from the given string representing a file
fn load_card_bodies(data: &str) -> Vec<CardBody> {
    let mut res = vec![];
    let mut lines = data.lines().peekable();
    while let Some(i) = lines.next() {
        let Some(i) = i.strip_prefix("REVIEW--") else {
            continue;
        };

        let Some((id, i)) = i.find(':').map(|idx| i.split_at(idx)) else {
            continue;
        };
        let front = i[1..].to_string();

        let Ok(id) = BASE64_STANDARD.decode(id) else {
            continue;
        };
        let Ok(id) = id.try_into() else { continue };

        let mut back = String::new();
        while let Some(i) = lines.next_if(|l| !l.starts_with("REVIEW--")) {
            back.push_str(i);
            back.push('\n');
        }

        res.push(CardBody {
            id: CardId(id),
            front,
            back,
        });
    }
    res
}

fn init_database(sqlite: &mut rusqlite::Connection) -> anyhow::Result<()> {
    sqlite.execute(
        "create table if not exists review(
             card int,
             last_reviewed int,
             stability real,
             difficulty real
        )",
        (),
    )?;
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
        /// List of files to look for cards
        files: Vec<PathBuf>,
    },

    /// Initializes all the given cards in the database
    ///
    /// usually unnecessary to do manually, as all commands automatically do this by default
    Init { files: Vec<PathBuf> },

    /// Lists all the cards in the given file
    Cards { files: Vec<PathBuf> },
}

fn update_review_data(
    sqlite: &mut rusqlite::Connection,
    id: CardId,
    fsrs: FSRSParams,
) -> anyhow::Result<()> {
    sqlite.execute(
        "insert into review(card, last_reviewed, stability, difficulty)
                                     values (?1, ?2, ?3, ?4)",
        (
            id.as_int(),
            SystemTime::UNIX_EPOCH.elapsed()?.as_secs(),
            fsrs.stability,
            fsrs.difficulty,
        ),
    )?;
    Ok(())
}

fn load_file(file: &Path) -> anyhow::Result<String> {
    let mut file = OpenOptions::new().read(true).write(true).open(file)?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;

    let ids = initialize_card_bodies(&mut data);
    for i in ids {
        eprintln!("Initialized new card!: {}", BASE64_STANDARD.encode(i.0));
    }

    file.seek(SeekFrom::Start(0))?;
    file.write_all(data.as_bytes())?;
    Ok(data)
}

fn load_card_data(
    sqlite: &mut rusqlite::Connection,
    id: CardId,
) -> Option<(SystemTime, FSRSParams)> {
    sqlite
        .query_row(
            "select last_reviewed, stability, difficulty from review
                 where card = ?1
                 order by last_reviewed desc
                 limit 1",
            [id.as_int()],
            |row| {
                Ok((
                    SystemTime::UNIX_EPOCH + Duration::from_secs(row.get(0)?),
                    FSRSParams {
                        stability: row.get(1)?,
                        difficulty: row.get(2)?,
                    },
                ))
            },
        )
        .ok()
}

fn main() -> anyhow::Result<()> {
    let command = Commands::parse();
    match command {
        Commands::Init { files } => {
            for file in &files {
                _ = load_file(file)?
            }
        }
        Commands::Review { retention, files } => {
            let mut cards = Vec::new();
            for file in &files {
                let data = load_file(file)?;
                cards.append(&mut load_card_bodies(&data));
            }

            let mut sqlite = rusqlite::Connection::open("db.sqlite3")?;
            init_database(&mut sqlite)?;

            execute!(std::io::stdout(), EnterAlternateScreen)?;
            crossterm::terminal::enable_raw_mode()?;

            'main: loop {
                let mut iters = 0;
                for card in &cards {
                    let res = load_card_data(&mut sqlite, card.id);
                    let fsrs = match res {
                        Some((last_reviewed, fsrs)) => {
                            let days_elapsed =
                                last_reviewed.elapsed()?.as_secs_f32() / (60.0 * 60.0 * 24.0);

                            if fsrs.recall_probability(days_elapsed) >= retention {
                                continue;
                            }
                            let Some(grade) = ui::review_card(card)? else {
                                break 'main;
                            };
                            fsrs.update_successful(grade)
                        }
                        None => {
                            let Some(grade) = ui::review_card(card)? else {
                                break 'main;
                            };
                            FSRSParams::from_initial_grade(grade)
                        }
                    };
                    iters += 1;
                    update_review_data(&mut sqlite, card.id, fsrs)?;
                }
                if iters == 0 {
                    break;
                }
            }

            crossterm::terminal::disable_raw_mode()?;
            execute!(std::io::stdout(), LeaveAlternateScreen)?;
        }
        Commands::Cards { files } => {
            let mut cards = Vec::new();
            for file in &files {
                let data = load_file(file)?;
                cards.append(&mut load_card_bodies(&data));
            }

            let mut sqlite = rusqlite::Connection::open("db.sqlite3")?;
            init_database(&mut sqlite)?;

            for (i, card) in cards.iter().enumerate() {
                println!("{}. {}", (i + 1).to_string(), card.front.trim().bold());
                let res = load_card_data(&mut sqlite, card.id);
                match res {
                    Some((last_reviewed, fsrs)) => {
                        let days_elapsed =
                            last_reviewed.elapsed()?.as_secs_f32() / (60.0 * 60.0 * 24.0);
                        let recall = fsrs.recall_probability(days_elapsed);
                        println!(
                            "stability: {:.2?}\ndifficulty: {:.2?}\npredicted recall: {:.2}%",
                            fsrs.stability,
                            fsrs.difficulty,
                            recall * 100.0
                        );
                    }
                    None => {
                        println!("{}", "Not yet reviewed".dark_grey());
                    }
                }

                println!();
            }
        }
    }
    Ok(())
}
