use std::io::{Stdout, Write};

use crossterm::{
    cursor::MoveTo,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType, WindowSize},
};

use crate::{CardBody, fsrs::Grade};

pub fn hide_cloze(ques: &str) -> String {
    let mut hidden = false;
    ques.chars()
        .filter_map(|c| {
            if c == '_' {
                hidden = !hidden;
                None
            } else if hidden {
                Some('_')
            } else {
                Some(c)
            }
        })
        .collect()
}

fn title(stdout: &mut Stdout, winsize: &WindowSize) -> anyhow::Result<()> {
    let header_text = "CARDSHARP\r\n\n";
    execute!(
        stdout,
        MoveTo(
            (winsize.columns - header_text.len().try_into().unwrap_or(u16::MAX)) / 2,
            0
        ),
        Print(header_text.red()),
    )?;
    Ok(())
}

fn print_question(stdout: &mut Stdout, question: &str) -> anyhow::Result<()> {
    execute!(
        stdout,
        Print("REVIEW: ".yellow()),
        Print(format!("{question}\r\n\n"))
    )?;
    Ok(())
}

pub fn review_card(card: &CardBody) -> anyhow::Result<Option<Grade>> {
    let mut stdout = std::io::stdout();
    let mut winsize = terminal::window_size()?;
    let front = card.front.trim();
    let back = card.back.trim();

    loop {
        execute!(&mut stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        title(&mut stdout, &winsize)?;
        print_question(&mut stdout, &hide_cloze(front))?;
        print!("Press any key to show backside....");
        stdout.flush()?;

        match crossterm::event::read()? {
            Event::Key(
                KeyEvent {
                    code: KeyCode::Esc | KeyCode::Char('q'),
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                },
            ) => return Ok(None),
            Event::Key(_) => break,
            Event::Resize(_, _) => {
                winsize = terminal::window_size()?;
            }
            _ => {}
        }
    }

    loop {
        execute!(&mut stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        title(&mut stdout, &winsize)?;
        print_question(&mut stdout, front)?;
        print!("{}\r\n\n1:again\t2: hard\t3/space: good\t4: easy", back);
        stdout.flush()?;

        match crossterm::event::read()? {
            Event::Key(event) => {
                let grade = match event.code {
                    KeyCode::Char('1') => Grade::Again,
                    KeyCode::Char('2') => Grade::Hard,
                    KeyCode::Char('3') => Grade::Good,
                    KeyCode::Char('4' | ' ') => Grade::Easy,
                    KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                    _ => continue,
                };
                return Ok(Some(grade));
            }
            Event::Resize(_, _) => {
                winsize = terminal::window_size()?;
            }
            _ => {}
        }
    }
}
