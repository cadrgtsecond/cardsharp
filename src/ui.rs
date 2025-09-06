use std::{
    io::{Stdout, Write},
    time::SystemTime,
};

use crossterm::{
    cursor::MoveTo,
    event::{Event, KeyCode},
    execute,
    style::{Color, Colors, Print, SetColors},
    terminal::{self, Clear, ClearType, WindowSize},
};

use crate::{
    Card, CardParams,
    fsrs::{FSRSParams, Grade},
};

const SPACES: &str = "\r\n\n";

#[allow(clippy::cast_possible_truncation)]
fn title(stdout: &mut Stdout, winsize: &WindowSize) -> anyhow::Result<()> {
    let header_text = "CARDSHARP\r\n\n";
    execute!(
        stdout,
        MoveTo((winsize.columns - header_text.len() as u16) / 2, 0),
        SetColors(Colors::new(Color::Black, Color::Red)),
        Print(header_text),
        SetColors(Colors::new(Color::Reset, Color::Reset)),
    )?;
    Ok(())
}

fn print_question(stdout: &mut Stdout, question: &str) -> anyhow::Result<()> {
    execute!(
        stdout,
        SetColors(Colors::new(Color::Yellow, Color::Black)),
        Print("REVIEW: "),
        SetColors(Colors::new(Color::Reset, Color::Reset)),
        Print(format!("{question}{SPACES}"))
    )?;
    Ok(())
}

fn review_card(card: &Card) -> anyhow::Result<Grade> {
    let mut stdout = std::io::stdout();
    let winsize = terminal::window_size()?;
    let question = card.title.trim();

    execute!(&mut stdout, MoveTo(0, 0))?;
    execute!(&mut stdout, Clear(ClearType::All))?;
    title(&mut stdout, &winsize)?;
    print_question(&mut stdout, question)?;
    print!("Press any key to show backside....");
    stdout.flush()?;

    loop {
        if let Event::Key(_) = crossterm::event::read()? {
            break;
        }
    }

    execute!(stdout, MoveTo(0, 0))?;
    execute!(stdout, Clear(ClearType::All))?;
    title(&mut stdout, &winsize)?;
    print_question(&mut stdout, question)?;

    print!(
        "{}{SPACES}1:again\t2: hard\t3/space: good\t4: easy",
        card.body.trim()
    );
    stdout.flush()?;

    let grade;
    loop {
        let ev = crossterm::event::read()?;
        match ev {
            Event::Key(event) => {
                grade = match event.code {
                    KeyCode::Char('1') => Grade::Again,
                    KeyCode::Char('2') => Grade::Hard,
                    KeyCode::Char('3') => Grade::Good,
                    KeyCode::Char('4' | ' ') => Grade::Easy,
                    _ => continue,
                };
                break;
            }
            Event::Resize(_, _) => todo!(),
            _ => {}
        }
    }
    Ok(grade)
}

pub fn review_again(
    CardParams { last_review, fsrs }: &mut CardParams,
    card: &Card,
    retention: f32,
) -> anyhow::Result<()> {
    let days_elapsed = last_review.elapsed()?.as_secs_f32() / (60.0 * 60.0 * 24.0);
    let r = fsrs.recall_probability(days_elapsed);
    if r < retention {
        let grade = review_card(card)?;
        if grade as u8 > 1 {
            *fsrs = fsrs.update_successful(grade);
        }
    }
    Ok(())
}

pub fn review_first_time(card: &Card) -> anyhow::Result<CardParams> {
    let grade = review_card(card)?;

    Ok(CardParams {
        last_review: SystemTime::now(),
        fsrs: FSRSParams::from_initial_grade(grade),
    })
}
