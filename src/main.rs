use bzip2::bufread::BzDecoder;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use rand::seq::{IteratorRandom, SliceRandom};
use ratatui::{layout::Layout, prelude::*, text::Line, widgets::*};
use std::{
    io::{self, stdout, Read},
    time::Instant,
};
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
    start: Option<Instant>,
    end: Option<Instant>,
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut test = Test::new();

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
        let mut out = Self {
            target: word,
            info,
            input: String::new(),
            spans: Vec::new(),
            start: None,
            end: None,
        };

        out.gen_spans();

        out
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

        self.gen_spans();

        false
    }

    fn delete(&mut self) -> bool {
        let res = self.input.pop().is_none();
        self.gen_spans();

        res
    }

    fn gen_spans(&mut self) {
        self.spans.clear();
        let mut target = self.target.chars();
        let mut input = self.input.chars();

        loop {
            self.spans.push(match (target.next(), input.next()) {
                (Some(t), Some(i)) if t == i => {
                    Span::styled(t.to_string(), Style::new().fg(Color::White))
                }
                (Some(t), Some(i)) if t != i => {
                    Span::styled(t.to_string(), Style::new().fg(Color::White).bg(Color::Red))
                }
                (Some(_), None) => Span::styled("_", Style::new().fg(Color::DarkGray)),
                (None, Some(i)) => Span::styled(i.to_string(), Style::new().fg(Color::Yellow)),
                _ => break,
            })
        }

        self.spans.push(Span::raw(" "));
    }
}

impl Test<'_> {
    fn new() -> Self {
        Self {
            words: Self::generate_words(64),
            index: 0,
            start: None,
            end: None,
        }
    }

    fn generate_words(n: usize) -> Vec<Word<'static>> {
        let mut rng = rand::thread_rng();

        // get words from compressed toml file
        let mut toml = String::new();
        let mut decompressor = BzDecoder::new(include_bytes!("../res/words.toml.bz2").as_slice());
        decompressor.read_to_string(&mut toml).unwrap();

        // create word vec
        let mut words = toml
            .parse::<Table>()
            .unwrap()
            .iter()
            .filter_map(|word| {
                let mut rng = rand::thread_rng();
                if let Some(toml::Value::Table(translations)) = word.1.get("pu_verbatim") {
                    if let Some(toml::Value::String(description)) = translations.get("en") {
                        return Some(Word::new(
                            // return word, Some(example)
                            word.0.to_string(),
                            Some(
                                description
                                    .split('\n')
                                    .map(|str| {
                                        let mut part = str.split(' ');
                                        "[".to_string()
                                            + part.next().unwrap_or_default().trim()
                                            + " " // put a space between part of speech and example
                                            + part
                                                .map(|ex| " ".to_string() + ex) // re-insert spaces
                                                .collect::<String>() // merge examples
                                                .split(',') // separate examples by commas
                                                .map(|ex| ex.trim())
                                                .choose(&mut rng) // choose a random example
                                                .unwrap_or_default()
                                            + "]\n "
                                    })
                                    .collect::<String>(),
                            ),
                        ));
                    }
                }
                None
            })
            .choose_multiple(&mut rng, n); // chose n words for the test

        words.shuffle(&mut rng); // randomize the order of the words

        words
    }

    fn push(&mut self, c: char) {
        if let Some(word) = self.words.get_mut(self.index) {
            if word.push(c) {
                self.index += 1;

                if self.end.is_none() && self.words.get(self.index).is_none() {
                    self.end = Some(Instant::now());
                }
            } else {
                if self.start.is_none() {
                    self.start = Some(Instant::now());
                }
            }
        }
    }

    fn delete(&mut self) {
        if let Some(word) = self.words.get_mut(self.index) {
            if word.delete() {
                if let Some(index) = self.index.checked_sub(1) {
                    self.index = index;
                }
            }
        }
    }

    fn get_text(&mut self) -> Text {
        let mut spans = Vec::new();

        for word in self.words.iter() {
            spans.append(&mut word.spans.clone());
        }
        spans.push(Span::raw("pini"));

        Text::from(Line::from(spans))
    }
}

fn handle_events(test: &mut Test) -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(50))? {
        match event::read()? {
            Event::Key(KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                code: KeyCode::Char('c'),
                ..
            }) => return Ok(true),
            Event::Key(KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                code: KeyCode::Char('r'),
                ..
            }) => *test = Test::new(),
            Event::Key(key) if key.kind == event::KeyEventKind::Press => match key.code {
                KeyCode::Backspace => test.delete(),
                KeyCode::Char(c) => test.push(c),
                _ => (),
            },
            _ => (),
        }
    }
    Ok(false)
}

fn ui(frame: &mut Frame, test: &mut Test) {
    let layout = Layout::vertical([
        Constraint::Percentage(30),
        Constraint::Fill(1),
        Constraint::Length(4),
    ])
    .split(frame.size());

    // definition
    frame.render_widget(
        Paragraph::new(Text::from(match test.words.get(test.index) {
            Some(word) => word.info.clone().unwrap_or_default(),
            _ => "".to_string(),
        }))
        .block(Block::default().padding(Padding {
            top: 1,
            left: 8,
            right: 8,
            bottom: 0,
        }))
        .centered()
        .bg(Color::Rgb(0, 20, 20))
        .wrap(Wrap { trim: true }),
        layout[0],
    );

    // test words
    frame.render_widget(
        Paragraph::new(test.get_text())
            .block(Block::default().padding(Padding {
                left: 2,
                right: 2,
                top: 1,
                bottom: 1,
            }))
            .bg(Color::Rgb(0, 10, 10))
            .wrap(Wrap { trim: true }),
        layout[1],
    );

    // test status/information
    frame.render_widget(
        Paragraph::new(Text::from(
            match (test.start, test.end) {
                (None, None) => "Test has not started".into(),
                (None, Some(_)) => {
                    "What did you do? It appears you ended the test without starting".into()
                }
                (Some(start), None) => format!(
                    "{} words per minute",
                    (60.0 * (test.index + 1) as f32 / start.elapsed().as_secs_f32()).round()
                ),
                (Some(start), Some(end)) => format!(
                    "Test finished: {} words per minute",
                    (60.0 * test.words.len() as f32 / end.duration_since(start).as_secs_f32())
                        .round()
                ),
            } + "\nctrl+c quit\nctrl+r restart",
        ))
        .centered()
        .bg(Color::Rgb(0, 10, 10)),
        layout[2],
    )
}
