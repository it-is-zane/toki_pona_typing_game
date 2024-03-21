use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use directories::ProjectDirs;
use rand::seq::IteratorRandom;
use ratatui::{prelude::*, widgets::*};
use std::{
    io::{self, stdout},
    ops::Not,
    time::Instant,
};
use toml::Table;

#[derive(Default)]
struct Test {
    prompt: String,
    input: String,
    time_data: Vec<Instant>,
    word_count: i32,
    char_accuracy: Option<u16>,
    word_accuracy: Option<u16>,
    word_data: Option<Table>,
}

impl Test {
    fn new(prompt: &str) -> Test {
        Test {
            prompt: prompt.to_string(),
            word_count: prompt.split(' ').count() as i32,
            word_data: Some(include_str!("../res/words.toml").parse().unwrap()),
            ..Default::default()
        }
    }

    fn push(&mut self, c: char) {
        if c == ' '
            || self
                .prompt
                .chars()
                .nth(self.input.len())
                .unwrap_or_default()
                != ' '
        {
            self.time_data.push(Instant::now());
            self.input.push(c);
        }
    }

    fn delete(&mut self) -> () {
        _ = self.input.pop();
    }

    fn randomize_words(&mut self) {
        self.word_count = 50;

        let mut rng = rand::thread_rng();
        /* let words = include_str!("../res/words.txt")
        .split('\n')
        .filter(|x| !" ".contains(x))
        .choose_multiple(&mut rng, self.word_count as usize); */

        let words = self
            .word_data
            .as_ref()
            .unwrap()
            .iter()
            .filter(|word| match word.1.get("deprecated") {
                Some(toml::Value::Boolean(deprecated)) => deprecated.not(),
                _ => false,
            })
            .filter(|word| word.1.get("pu_verbatim").is_some())
            .choose_multiple(&mut rng, self.word_count as usize);

        self.prompt.clear();

        for word in words {
            if let Some(toml::Value::String(word)) = word.1.get("word") {
                self.prompt.push_str(format!("{} ", word.trim()).as_str());
            }
        }

        assert!(self.prompt.len() > 0);

        self.prompt = self.prompt.trim().to_string();
    }

    fn calculate_accuracy(&mut self) {
        let mut char_accuracy = 0;

        for (input, prompt) in self.input.chars().zip(self.prompt.chars()) {
            if input == prompt {
                char_accuracy += 1;
            }
        }

        char_accuracy *= 100;
        char_accuracy /= self.input.len() as u16;

        let mut word_accuracy = 0;

        for word in self.prompt.split(" ") {
            if self.input.split(" ").find(|x| x == &word).is_some() {
                word_accuracy += 1;
            }
        }

        word_accuracy *= 100;
        word_accuracy /= self.word_count as u16;

        self.char_accuracy = Some(char_accuracy);
        self.word_accuracy = Some(word_accuracy);
    }
}

#[derive(PartialEq)]
enum GameState {
    Playing,
    Finished,
    Quit,
}

fn main() -> io::Result<()> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "ZaneCG", "Typing Test") {
        _ = proj_dirs.data_dir(); // get app data dir
        _ = proj_dirs.config_dir(); // get app config dir
    }

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut game_state = GameState::Playing;

    while game_state != GameState::Quit {
        let mut test = Test::new("");
        test.randomize_words();

        while game_state == GameState::Playing {
            terminal.draw(|frame| game_ui(frame, &test))?;
            game_state = handle_game_events(&mut test)?;

            if test.prompt.len() == test.input.len() {
                game_state = GameState::Finished;
                test.calculate_accuracy();
            }
        }
        while game_state == GameState::Finished {
            terminal.draw(|frame| finished_ui(frame, &test))?;
            game_state = handle_finished_events()?;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn handle_finished_events() -> io::Result<GameState> {
    if !event::poll(std::time::Duration::from_millis(50))? {
        return Ok(GameState::Finished);
    }

    let (modifiers, code) = match event::read()? {
        Event::Key(KeyEvent {
            modifiers, code, ..
        }) => (modifiers, code),
        _ => (KeyModifiers::NONE, KeyCode::Null),
    };

    match (modifiers, code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('d')) => {
            return Ok(GameState::Quit)
        }
        (_, KeyCode::Char('r')) => return Ok(GameState::Playing),
        (_, KeyCode::Char('q')) => return Ok(GameState::Quit),
        _ => {}
    }

    Ok(GameState::Finished)
}

fn handle_game_events(test: &mut Test) -> io::Result<GameState> {
    if !event::poll(std::time::Duration::from_millis(50))? {
        return Ok(GameState::Playing);
    }

    let (modifiers, code) = match event::read()? {
        Event::Key(KeyEvent {
            modifiers, code, ..
        }) => (modifiers, code),
        _ => (KeyModifiers::NONE, KeyCode::Null),
    };

    match (modifiers, code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('d')) => {
            return Ok(GameState::Quit)
        }
        (_, KeyCode::Char(c)) => test.push(c),
        (_, KeyCode::Backspace) => _ = test.delete(),
        _ => {}
    }

    Ok(GameState::Playing)
}

fn finished_ui(frame: &mut Frame, test: &Test) {
    frame.render_widget(
        Paragraph::new(format!(
            "Words per minute: {}\
                \nCharacter Accuracy {}%\
                \nWord Accuracy {}%\
                \n\nq    to quit\
                \nr to restart",
            60.0 * test.word_count as f32
                / test
                    .time_data
                    .last()
                    .unwrap()
                    .duration_since(*test.time_data.first().unwrap())
                    .as_secs_f32(),
            test.char_accuracy.unwrap(),
            test.word_accuracy.unwrap(),
        ))
        .block(
            Block::default()
                .title("Test Results")
                .borders(Borders::ALL)
                .padding(Padding {
                    left: 1,
                    right: 1,
                    top: 1,
                    bottom: 1,
                })
                .title_alignment(Alignment::Center),
        )
        .yellow()
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false }),
        frame.size(),
    );
}
fn game_ui(frame: &mut Frame, test: &Test) {
    let spans = {
        let mut spans = Vec::<Span>::new();
        let mut p = test.prompt.chars();
        let mut i = test.input.chars();

        loop {
            let _chunck = String::new();

            match (p.next(), i.next()) {
                (None, None) => break,
                (None, Some(_)) => break,
                (Some(p), None) => spans.push(Span::styled(
                    p.to_string(),
                    Style::new().fg(Color::DarkGray),
                )),
                (Some(p), Some(i)) if p == i => {
                    spans.push(Span::styled(p.to_string(), Style::new()))
                }
                (Some(p), Some(_)) if p == ' ' => {
                    spans.push(Span::styled(p.to_string(), Style::new().bg(Color::Red)))
                }
                (Some(p), Some(_)) => {
                    spans.push(Span::styled(p.to_string(), Style::new().fg(Color::Red)))
                }
            }
        }

        spans
    };

    let mut definition = None;
    if let Some(word_data) = test.word_data.as_ref() {
        let player_position = test.input.len();
        let mut position = 0;
        for word in test.prompt.split(" ") {
            position += word.len() + 1;
            if position > player_position {
                if let Some(toml::Value::Table(word_data)) = word_data.get(word) {
                    if let Some(toml::Value::Table(translations)) = word_data.get("pu_verbatim") {
                        if let Some(toml::Value::String(def)) = translations.get("en") {
                            definition = Some(Span::from(format!(" {}", def)));
                        }
                    }
                }
                break;
            }
        }
    };

    let line = Line::from(spans);
    let text = match definition {
        Some(definition) => Text::from(vec![line, Line::default(), Line::from(definition)]),
        None => Text::from(line),
    };

    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .title("Typing Test")
                    .borders(Borders::NONE)
                    .padding(Padding {
                        left: 2,
                        right: 2,
                        top: 1,
                        bottom: 1,
                    })
                    .title_alignment(Alignment::Center),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        frame.size(),
    );
}
