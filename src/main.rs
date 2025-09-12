#![deny(clippy::pedantic)]

use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::Path,
    process::{Command, Stdio},
    time::SystemTime,
};

use crate::data::ReviewParams;

mod base64;
mod data;
mod fsrs;
mod ui;

#[derive(Debug, Clone)]
struct Card {
    id: u64,
    title: String,
    body: String,
    review_params: Option<ReviewParams>,
}

fn find_cards(params: &HashMap<String, ReviewParams>) -> anyhow::Result<Vec<Card>> {
    // TODO: Support other searching commands, such as `grep -R` or `ag`
    let grep_result = Command::new("rg")
        .arg("-g")
        .arg("*.{md,adoc,txt}")
        .arg("-0b")
        .arg("REVIEW: __[a-zA-Z0-9+/]{11}=")
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
    Ok(data
        .filter_map(|(cardpath, off)| {
            if cardpath != prev_path {
                prev_path = cardpath;
                file = BufReader::new(File::open(cardpath).ok()?);
            }
            file.seek(SeekFrom::Start(off)).ok()?;
            let mut title = String::new();
            file.read_line(&mut title).ok()?;

            let title = title
                .strip_prefix("REVIEW:")?
                .trim_start()
                .strip_prefix("__")?;

            let (id_str, title) = title.split_once(char::is_whitespace).unwrap_or((title, ""));
            let id = base64::from_base64(id_str)?;

            let mut body = String::new();
            loop {
                let mut line = String::new();
                file.read_line(&mut line).ok()?;
                if line == "---\n" || line.starts_with("REVIEW:") {
                    break;
                }
                body.push_str(&line);
            }
            Some(Card {
                id,
                title: title.to_string(),
                body,
                review_params: params.get(id_str).cloned(),
            })
        })
        .collect())
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

    /// Initializes all cards under the current directory, adding id's to them if they did not exist
    Init {},
}

fn main() -> anyhow::Result<()> {
    let command = Commands::parse();
    match command {
        Commands::Init {} => {
            // TODO: Actually initialize card
        }
        Commands::Review { retention } => {
            let mut file = data::open_data()?;
            let mut data = data::load_data(&mut file);
            let mut cards = find_cards(&data.review_params)?;

            execute!(std::io::stdout(), EnterAlternateScreen)?;
            crossterm::terminal::enable_raw_mode()?;

            loop {
                let mut iters = 0;
                for card in &mut cards {
                    let id = base64::to_base64(card.id);
                    let new_params =
                        if let Some(ReviewParams { last_review, fsrs }) = card.review_params {
                            // We add one extra day so that we don't have to review after a short time
                            let days_elapsed =
                                1.0 + last_review.elapsed()?.as_secs_f32() / (60.0 * 60.0 * 24.0);

                            if fsrs.recall_probability(days_elapsed) >= retention {
                                continue;
                            }
                            iters += 1;
                            let grade = ui::review_card(card)?;

                            ReviewParams {
                                last_review: SystemTime::now(),
                                fsrs: fsrs.update_successful(grade),
                            }
                        } else {
                            iters += 1;
                            ui::review_first_time(card)?
                        };
                    data.review_params.insert(id, new_params);
                }
                if iters == 0 {
                    break;
                }
            }

            crossterm::terminal::disable_raw_mode()?;
            execute!(std::io::stdout(), LeaveAlternateScreen)?;
            data::save_data(&mut file, &data)?;
        }
    }
    Ok(())
}
