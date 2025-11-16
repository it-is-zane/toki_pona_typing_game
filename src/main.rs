#![allow(unused)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use rand::{rngs::ThreadRng, seq::SliceRandom};
use ratatui::{
    crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers},
    layout::{
        Constraint,
        Direction::{Horizontal, Vertical},
        Layout,
    },
    style::{Color, Modifier, Style, Styled, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType::Rounded, Paragraph, Wrap},
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    ops::SubAssign,
    str::Chars,
    sync::LazyLock,
    time::{Instant, SystemTime},
};

const APPLICATION: &str = "tt";

#[cfg(not(feature = "compressed"))]
static WORDS: LazyLock<HashMap<String, toml::Table>> = LazyLock::new(|| {
    toml::from_str(include_str!("res/words.toml")).expect("failed to parse words.toml")
});

#[cfg(feature = "compressed")]
static WORDS: LazyLock<HashMap<String, toml::Table>> = LazyLock::new(|| {
    let bz2 = include_bytes!("res/words.toml.bz2").as_slice();
    let mut toml = String::new();
    let mut decompressor = bzip2::read::BzDecoder::new(bz2);

    decompressor
        .read_to_string(&mut toml)
        .expect("failed to decompress words");

    toml::from_str(&toml).expect("failed to parse words.toml")
});

#[derive(serde::Deserialize, serde::Serialize)]
struct WordResults {}

enum GameSpan<T> {
    Correct(T),
    Wrong(T),
    Overflow(T),
    Skipped(T),
    Hidden(T),
}

impl<T> GameSpan<T> {
    fn map<T2, F: Fn(&T) -> T2>(&self, f: F) -> GameSpan<T2> {
        match self {
            Self::Correct(v) => GameSpan::Correct(f(v)),
            Self::Wrong(v) => GameSpan::Wrong(f(v)),
            Self::Overflow(v) => GameSpan::Overflow(f(v)),
            Self::Skipped(v) => GameSpan::Skipped(f(v)),
            Self::Hidden(v) => GameSpan::Hidden(f(v)),
        }
    }
}

struct GameSettings<T> {
    core: T,
    common: T,
    uncommon: T,
    obscure: T,
    sandbox: T,
    deprecated: T,
    nondeprecated: T,
    words: HashMap<String, T>,
    len: usize,
}

impl GameSettings<usize> {
    const DEFAULT: usize = 1000;

    fn get_word(&self, word: &str) -> usize {
        *self.words.get(word).unwrap_or(&Self::DEFAULT)
    }
}

impl Default for GameSettings<usize> {
    fn default() -> Self {
        Self {
            core: Self::DEFAULT,
            common: Self::DEFAULT * 200,
            uncommon: Self::DEFAULT * 400,
            obscure: Self::DEFAULT * 600,
            sandbox: Self::DEFAULT * 800,
            deprecated: Self::DEFAULT * 800,
            nondeprecated: Self::DEFAULT,
            words: HashMap::new(),
            len: 60,
        }
    }
}

struct Game<K> {
    words: Vec<&'static toml::map::Map<String, toml::Value>>,
    key_log: Vec<(K, Instant)>,
    target: String,
    input: String,
    spans: Vec<GameSpan<String>>,
}

impl Game<KeyCode> {
    fn new(settings: &GameSettings<usize>) -> Self {
        let mut words: Vec<_> = WORDS.values().collect();

        words.sort_by_cached_key(|toml| {
            let category_weight = toml
                .get("usage_category")
                .and_then(toml::Value::as_str)
                .map(|cat| match cat {
                    "core" => settings.core,
                    "common" => settings.common,
                    "uncommon" => settings.uncommon,
                    "obscure" => settings.obscure,
                    "sandbox" => settings.sandbox,
                    _ => todo!(),
                })
                .expect("failed to get category");

            let deprecated_weight = toml
                .get("deprecated")
                .and_then(toml::Value::as_bool)
                .map(|b| {
                    if b {
                        settings.deprecated
                    } else {
                        settings.nondeprecated
                    }
                })
                .expect("failed to get deprecation");

            let word_weight = settings.get_word(
                toml.get("word")
                    .and_then(toml::Value::as_str)
                    .expect("failed to get word field"),
            );

            category_weight * deprecated_weight * word_weight * rand::random_range(900..1100)
        });

        words.truncate(settings.len);

        let mut target = String::new();
        let mut iter = words
            .iter()
            .filter_map(|word| word.get("word"))
            .filter_map(toml::Value::as_str);

        target.push_str(iter.next().expect("words list was empty"));
        for word in iter {
            target.push(' ');
            target.push_str(word);
        }

        Self {
            words,
            key_log: Vec::new(),
            target: target.clone(),
            input: String::new(),
            spans: Vec::new(),
        }
    }

    fn calculate_spans(&mut self) {
        let mut spans = Vec::new();

        let mut targ = self.target.chars().peekable();
        let mut inpt = self.input.chars().peekable();

        loop {
            match (targ.peek(), inpt.peek()) {
                (Some(t), Some(i)) if t == i => {
                    spans.push(GameSpan::Correct(*t));
                    targ.next();
                    inpt.next();
                }
                (Some(t), Some(' ')) => {
                    spans.push(GameSpan::Skipped(*t));
                    targ.next();
                }
                (Some(' ') | None, Some(i)) => {
                    spans.push(GameSpan::Overflow(*i));
                    inpt.next();
                }
                (Some(t), Some(i)) => {
                    spans.push(GameSpan::Wrong(*t));
                    targ.next();
                    inpt.next();
                }
                (Some(t), None) => {
                    spans.push(GameSpan::Hidden(if *t == ' ' { ' ' } else { '_' }));
                    targ.next();
                }
                _ => break,
            }
        }

        let mut spans = spans.iter().peekable();
        self.spans.clear();

        loop {
            match (self.spans.last_mut(), spans.peek()) {
                (Some(GameSpan::Correct(s_span)), Some(GameSpan::Correct(c_span)))
                | (Some(GameSpan::Wrong(s_span)), Some(GameSpan::Wrong(c_span)))
                | (Some(GameSpan::Overflow(s_span)), Some(GameSpan::Overflow(c_span)))
                | (Some(GameSpan::Skipped(s_span)), Some(GameSpan::Skipped(c_span)))
                | (Some(GameSpan::Hidden(s_span)), Some(GameSpan::Hidden(c_span))) => {
                    s_span.push(*c_span);
                    spans.next();
                }
                (_, Some(c_span)) => {
                    self.spans
                        .push(c_span.map(std::string::ToString::to_string));
                    spans.next();
                }
                _ => break,
            }
        }
    }

    fn crossterm_event(&mut self, event: &Event) {
        if let Event::Key(key_event) = event {
            self.key_log.push((key_event.code, Instant::now()));

            match key_event.code {
                KeyCode::Char(c) => self.input.push(c),
                KeyCode::Backspace => _ = self.input.pop(),
                _ => (),
            }
        }

        self.calculate_spans();
    }

    fn draw_game_ratatui<B: ratatui::backend::Backend>(&self, terminal: &mut ratatui::Terminal<B>) {
        const CORRECT: Style = Style::new().fg(Color::Green);

        const WRONG: Style = Style::new()
            .fg(Color::Red)
            .add_modifier(Modifier::UNDERLINED)
            .add_modifier(Modifier::BOLD);

        const OVERFLOW: Style = Style::new().fg(Color::Yellow);

        const SKIPPED: Style = Style::new().fg(Color::LightRed);

        const HIDDEN: Style = Style::new();

        let current_index = self.input.chars().filter(|c| *c == ' ').count();
        let mut words = self.target.split_whitespace();

        let word_1 = if self.input.ends_with(' ') {
            words.nth(current_index)
        } else {
            words.nth(current_index.checked_sub(1).unwrap_or_default())
        };

        let word_2 = words.next();

        terminal
            .draw(|frame| {
                let [top, main] = Layout::new(Vertical, [Constraint::Fill(1), Constraint::Fill(3)])
                    .areas(frame.area());
                let [top_l, top_r] =
                    Layout::new(Horizontal, [Constraint::Fill(1), Constraint::Fill(1)]).areas(top);

                let ratatui_spans = self.spans.iter().map(|span| match span {
                    GameSpan::Correct(line) => Span::styled(line, CORRECT),
                    GameSpan::Wrong(line) => Span::styled(line, WRONG),
                    GameSpan::Overflow(line) => Span::styled(line, OVERFLOW),
                    GameSpan::Skipped(line) => Span::styled(line, SKIPPED),
                    GameSpan::Hidden(line) => Span::styled(line, HIDDEN),
                });

                for (word, area) in [(word_1, top_l), (word_2, top_r)] {
                    if let Some(toml) = word.and_then(|w| WORDS.get(w)) {
                        frame.render_widget(
                            Paragraph::new(
                                [
                                    toml.get("definition")
                                        .map(toml::Value::to_string)
                                        .map(|s| "DEFINITION ".to_string() + s.trim_matches('\"')),
                                    Some(String::new()),
                                    toml.get("pu_verbatim")
                                        .and_then(|value| value.get("en"))
                                        .map(toml::Value::to_string)
                                        .map(|s| s.trim_matches('\"').to_string()),
                                    Some(String::new()),
                                    toml.get("ku_data").and_then(|value| value.as_table()).map(
                                        |table| {
                                            table.keys().fold("KU DATA".to_string(), |mut s, k| {
                                                s.push(' ');
                                                s.push_str(k);
                                                s
                                            })
                                        },
                                    ),
                                ]
                                .iter()
                                .flatten()
                                .map(Line::raw)
                                .collect::<Text>(),
                            )
                            .wrap(Wrap { trim: false })
                            .block(Block::bordered()),
                            area,
                        );
                    }
                }

                frame.render_widget(
                    Paragraph::new(ratatui_spans.collect::<Line>()).wrap(Wrap::default()),
                    main,
                );
            })
            .expect("failed to draw frame");
    }
}

fn main() {
    let mut terminal = ratatui::init();

    ratatui::crossterm::execute!(
        terminal.backend_mut(),
        ratatui::crossterm::event::EnableMouseCapture
    );

    // get user history
    // let history_path = directories::ProjectDirs::from("", "", APPLICATION)
    //     .map(|base_dirs| {
    //         if !base_dirs.config_dir().exists() {
    //             std::fs::create_dir_all(base_dirs.config_dir());
    //         }

    //         base_dirs.config_dir().to_path_buf()
    //     })
    //     .unwrap()
    //     .join("config.toml");

    // parse user profile
    // let history: std::collections::HashMap<String, Vec<WordResults>> =
    //     std::fs::read_to_string(&history_path)
    //         .map(|data| toml::from_str(&data).ok())
    //         .ok()
    //         .flatten()
    //         .unwrap();

    // initialization
    let mut game: Game<KeyCode> = Game::new(&GameSettings::default());

    // game
    loop {
        let event = ratatui::crossterm::event::read().expect("failed to read event");

        if let Event::Key(
            KeyEvent {
                code: KeyCode::Esc, ..
            }
            | KeyEvent {
                code: KeyCode::Char('c' | 'd'),
                modifiers: KeyModifiers::CONTROL,
                ..
            },
        ) = event
        {
            break;
        }

        game.crossterm_event(&event);
        game.draw_game_ratatui(&mut terminal);
    }

    // results

    // write user data to file
    // std::fs::File::create(&history_path)
    //     .unwrap()
    //     .write(toml::to_string(&history).unwrap().as_bytes());

    ratatui::crossterm::execute!(
        terminal.backend_mut(),
        ratatui::crossterm::event::DisableMouseCapture
    );

    ratatui::restore();
}
