use std::{collections::HashMap, fs::{File, OpenOptions}, io::{Seek, SeekFrom}, path::PathBuf, time::SystemTime};

use serde::{Deserialize, Serialize};

use crate::fsrs::FSRSParams;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Data {
    #[serde(default)]
    pub review_params: HashMap<String, ReviewParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewParams {
    pub last_review: SystemTime,
    pub fsrs: FSRSParams,
}

fn data_path() -> anyhow::Result<PathBuf> {
    let mut home = PathBuf::from(std::env::var("HOME")?);
    home.push(".local/share/cardsharp");

    Ok(std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home))
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

pub fn load_data(file: &mut File) -> Data {
    serde_json::from_reader(file).unwrap_or_else(|_| Data::default())
}

pub fn save_data(file: &mut File, data: &Data) -> anyhow::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    serde_json::to_writer(file, data)?;
    Ok(())
}

