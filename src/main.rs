use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

mod base64;
mod fsrs;

#[derive(Debug, Serialize, Deserialize)]
pub struct CardContent {
    id: u64,
    #[serde(skip)]
    title: String,
    #[serde(skip)]
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

/// Parses the base64 ID string from a given title
pub fn parse_id_from_title(mut title: &str) -> Option<u64> {
    if title.starts_with("REVIEW:") {
        title = &title[7..];
    }
    title = title.trim_start();
    if !title.starts_with("__") {
        return None;
    }
    title = &title[2..];
    base64::from_base64(
        title
            .split_whitespace()
            .next()?
            .as_bytes()
            .try_into()
            .ok()?,
    )
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

        let id = match parse_id_from_title(&title) {
            Some(x) => x,
            None => {
                println!("New card in found in {}", cardpath.to_string_lossy());
                let id: u64 = rand::random();
                let mut file = OpenOptions::new().read(true).write(true).open(&cardpath)?;
                let mut rest = String::new();
                file.seek(SeekFrom::Start(written + off + 7))?;
                file.read_to_string(&mut rest)?;

                file.seek(SeekFrom::Start(written + off + 7))?;
                written += file.write(b" __")? as u64;
                written += file.write(&base64::to_base64(id))? as u64;
                file.write(rest.as_bytes())? as u64;
                id
            }
        };
        Ok(CardContent { id, title, body })
    })
    .collect()
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
