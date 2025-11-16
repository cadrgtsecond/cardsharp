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

use crate::{fsrs::Grade, CardBody};

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

pub fn review_card(card: &CardBody) -> anyhow::Result<Grade> {
    let mut stdout = std::io::stdout();
    let winsize = terminal::window_size()?;
    let question = card.front.trim();

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
        card.back.trim()
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
