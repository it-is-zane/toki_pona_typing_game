use std::{
    io::{self, stdout},
    time::Instant,
};

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use rand::{rngs::ThreadRng, seq::IteratorRandom};
use ratatui::{prelude::*, text::Line, widgets::*};
use toml::Table;

struct Word<'a> {
    target: String,
    info: Option<String>, // e.g. Definition
    input: String,
    spans: Vec<Span<'a>>,
    start: Option<Instant>,
    end: Option<Instant>,
}

struct Test<'a> {
    words: Vec<Word<'a>>,
    index: usize,
    rng: ThreadRng,
    start: Option<Instant>,
    end: Option<Instant>,
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut test = Test::new();
    for word in test.words.iter_mut() {
        word.gen_spans();
    }

    let mut should_quit = false;
    while !should_quit {
        terminal.draw(|frame| ui(frame, &mut test))?;
        should_quit = handle_events(&mut test)?;
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

impl Word<'_> {
    fn new(word: String, info: Option<String>) -> Self {
        Self {
            target: word,
            info: None,
            input: String::new(),
            spans: Vec::new(),
            start: None,
            end: None,
        }
    }

    fn push(&mut self, c: char) -> bool {
        if self.input.is_empty() {
            self.start = Some(Instant::now());
        }

        if c == ' ' {
            self.end = Some(Instant::now());
            return true;
        }

        self.input.push(c);

        false
    }

    fn delete(&mut self) -> bool {
        self.input.pop().is_none()
    }

    fn gen_spans(&mut self) {
        if self.target == self.input {
            self.spans.push(Span::styled(
                self.target.clone() + " ",
                Style::new().fg(Color::White),
            ));
        } else {
            self.spans.push(Span::styled(
                self.target.clone() + " ",
                Style::new().fg(Color::Red),
            ));
        }
    }
}

impl Test<'_> {
    fn new() -> Self {
        Self {
            words: Self::generate_words(64),
            index: 0,
            rng: rand::thread_rng(),
            start: None,
            end: None,
        }
    }

    fn generate_words(n: usize) -> Vec<Word<'static>> {
        include_str!("../res/words.toml")
            .parse::<Table>()
            .unwrap()
            .iter()
            .filter_map(|word| {
                if let Some(toml::Value::Table(translations)) = word.1.get("pu_verbatim") {
                    if let Some(toml::Value::String(description)) = translations.get("en") {
                        return Some(Word::new(word.0.to_string(), Some(description.to_string())));
                    }
                }
                None
            })
            .choose_multiple(&mut rand::thread_rng(), n)
    }

    fn push(&mut self, c: char) {
        if let Some(word) = self.words.get_mut(self.index) {
            if word.push(c) {
                self.index += 1;
            }
        }
    }

    fn delete(&mut self) {
        if let Some(word) = self.words.get_mut(self.index) {
            if word.delete() {
                self.index -= 1;
            }
        }
    }

    fn get_text(&mut self) -> Text {
        let mut spans = Vec::new();

        for word in self.words.iter() {
            spans.append(&mut word.spans.clone());
        }

        Text::from(Line::from(spans))
    }
}

fn handle_events(test: &mut Test) -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press && key.code == KeyCode::Char('q') {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn ui(frame: &mut Frame, test: &mut Test) {
    frame.render_widget(
        Paragraph::new(test.get_text())
            .block(Block::default().padding(Padding {
                left: 2,
                right: 2,
                top: 1,
                bottom: 1,
            }))
            .wrap(Wrap { trim: true }),
        frame.size(),
    );
}
