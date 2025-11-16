#![deny(clippy::pedantic)]

use base64::{Engine, prelude::BASE64_STANDARD};
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
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
    loop {
        let Some(i) = lines.next() else { break };

        if !i.starts_with("REVIEW--") {
            continue;
        }
        let i = &i["REVIEW--".len()..];

        let Some(end) = i.find(':') else { continue };
        let id = &i.as_bytes()[0..end];
        let i = &i[end + 1..];

        let Ok(id) = BASE64_STANDARD.decode(id) else {
            continue;
        };
        let Ok(id) = id.try_into() else { continue };
        let id = CardId(id);

        let front = i.to_string();
        let mut back = String::new();
        loop {
            let Some(i) = lines.peek() else { break };
            if i.starts_with("REVIEW--") {
                break;
            }
            let Some(i) = lines.next() else { break };
            back.push_str(i);
            back.push('\n');
        }

        res.push(CardBody { id, front, back });
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
        files: Vec<PathBuf>,
    },

    /// Initializes all specified files in the database
    /// usually unnecessary
    Init { files: Vec<PathBuf> },
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

fn main() -> anyhow::Result<()> {
    let command = Commands::parse();
    match command {
        Commands::Init { files } => {
            for file in files {
                let mut file = OpenOptions::new().read(true).write(true).open(file)?;
                let mut data = String::new();
                file.read_to_string(&mut data)?;

                let ids = initialize_card_bodies(&mut data);
                for i in ids {
                    eprintln!("Initialized new card!: {}", BASE64_STANDARD.encode(i.0));
                }

                file.seek(SeekFrom::Start(0))?;
                file.write_all(data.as_bytes())?;
            }
        }
        Commands::Review { retention, files } => {
            let cards: Vec<Vec<CardBody>> = files
                .into_iter()
                .map(|file| {
                    let mut file = OpenOptions::new().read(true).write(true).open(file)?;
                    let mut data = String::new();
                    file.read_to_string(&mut data)?;

                    let ids = initialize_card_bodies(&mut data);
                    for i in ids {
                        eprintln!("Initialized new card!: {}", BASE64_STANDARD.encode(i.0));
                    }
                    Ok(load_card_bodies(&data))
                })
                .collect::<anyhow::Result<_>>()?;
            let cards: Vec<_> = cards.into_iter().flatten().collect();

            let mut sqlite = rusqlite::Connection::open("db.sqlite3")?;
            init_database(&mut sqlite)?;

            execute!(std::io::stdout(), EnterAlternateScreen)?;
            crossterm::terminal::enable_raw_mode()?;

            loop {
                let mut iters = 0;
                for card in &cards {
                    let (last_reviewed, fsrs) = sqlite
                        .query_row(
                            "select last_reviewed, stability, difficulty from review
                                 where card = ?1
                                 order by last_reviewed desc
                                 limit 1",
                            [card.id.as_int()],
                            |row| {
                                Ok((
                                    SystemTime::UNIX_EPOCH + Duration::from_secs(row.get(0)?),
                                    Some(FSRSParams {
                                        stability: row.get(1)?,
                                        difficulty: row.get(2)?,
                                    }),
                                ))
                            },
                        )
                        .unwrap_or_else(|_| (SystemTime::now(), None));
                    let days_elapsed =
                        last_reviewed.elapsed()?.as_secs_f32() / (60.0 * 60.0 * 24.0);

                    match fsrs {
                        Some(fsrs) if fsrs.recall_probability(days_elapsed) < retention => {
                            let grade = ui::review_card(card)?;

                            let fsrs = fsrs.update_successful(grade);
                            iters += 1;
                            update_review_data(&mut sqlite, card.id, fsrs)?;
                        }
                        None => {
                            let grade = ui::review_card(card)?;
                            let fsrs = FSRSParams::from_initial_grade(grade);
                            iters += 1;
                            update_review_data(&mut sqlite, card.id, fsrs)?;
                        }
                        _ => {}
                    }
                }
                if iters == 0 {
                    break;
                }
            }

            crossterm::terminal::disable_raw_mode()?;
            execute!(std::io::stdout(), LeaveAlternateScreen)?;
        }
    }
    Ok(())
}
