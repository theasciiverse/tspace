//! Keyboard.
//!
//! - W / Up: thrust, S / Down: reverse, A D / Left Right: turn
//! - Space: guns (hold), F: bomb, X: mine, E: afterburner (hold)
//! - 1-4: change ship, Tab: scores, Esc: menu, H / F1: help, F8: sound
//!
//! With the kitty keyboard protocol presses and releases are exact, and
//! thrusting while turning and firing works as it should. Elsewhere a key
//! counts as held while press/repeat events keep arriving (Panzer's rule),
//! and terminals repeat only the last key pressed.

use crate::ships::{self, Kind};
use crate::world::Controls;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

const PRE_REPEAT: Duration = Duration::from_millis(220);
const REPEAT_GAP: Duration = Duration::from_millis(110);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Key {
    Thrust,
    Reverse,
    Left,
    Right,
    Fire,
    Burn,
}
const KEYS: usize = 6;

#[derive(Clone, Copy, Default)]
struct Hold {
    down: bool,
    last: Option<Instant>,
    repeats: u32,
}

impl Hold {
    fn held(&self, exact: bool, now: Instant) -> bool {
        if exact {
            return self.down;
        }
        let Some(last) = self.last else { return false };
        let window = if self.repeats == 0 { PRE_REPEAT } else { REPEAT_GAP };
        now.duration_since(last) <= window
    }
}

/// Menu and system commands, as edges.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    Confirm,
    Back,
    Up,
    Down,
    Left,
    Right,
    Help,
    Mute,
    Scores,
    Backspace,
    Exit,
}

pub struct Input {
    pub exact: bool,
    holds: [Hold; KEYS],
    bomb: bool,
    mine: bool,
    ship: Option<Kind>,
    pub cmds: Vec<Cmd>,
    /// Characters typed (for the ticket box), and pasted text.
    pub text: String,
}

impl Input {
    pub fn new(exact: bool) -> Input {
        Input { exact, holds: [Hold::default(); KEYS], bomb: false, mine: false, ship: None, cmds: Vec::new(), text: String::new() }
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) {
        let press = k.kind != KeyEventKind::Release;
        let fresh = k.kind == KeyEventKind::Press;
        if fresh
            && let KeyCode::Char(ch) = k.code
                && !k.modifiers.contains(KeyModifiers::CONTROL) {
                    self.text.push(ch);
                }
        let held = match k.code {
            KeyCode::Char('w' | 'W') | KeyCode::Up => Some(Key::Thrust),
            KeyCode::Char('s' | 'S') | KeyCode::Down => Some(Key::Reverse),
            KeyCode::Char('a' | 'A') | KeyCode::Left => Some(Key::Left),
            KeyCode::Char('d' | 'D') | KeyCode::Right => Some(Key::Right),
            KeyCode::Char(' ') => Some(Key::Fire),
            KeyCode::Char('e' | 'E') => Some(Key::Burn),
            _ => None,
        };
        if let Some(key) = held {
            let h = &mut self.holds[key as usize];
            if self.exact {
                h.down = press;
            } else {
                // without release events every repeat arrives as a press
                if h.held(false, now) {
                    h.repeats += 1;
                } else {
                    h.repeats = 0;
                }
                h.last = Some(now);
            }
            if fresh {
                // the same keys move through menus
                match key {
                    Key::Thrust => self.cmds.push(Cmd::Up),
                    Key::Reverse => self.cmds.push(Cmd::Down),
                    Key::Left => self.cmds.push(Cmd::Left),
                    Key::Right => self.cmds.push(Cmd::Right),
                    Key::Fire => self.cmds.push(Cmd::Confirm),
                    Key::Burn => {}
                }
            }
            return;
        }
        if !fresh {
            return;
        }
        match k.code {
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => self.cmds.push(Cmd::Exit),
            KeyCode::Char('f' | 'F') => self.bomb = true,
            KeyCode::Char('x' | 'X') => self.mine = true,
            KeyCode::Char(c @ '1'..='4') => self.ship = Some(ships::ALL[c as usize - '1' as usize]),
            KeyCode::Enter => self.cmds.push(Cmd::Confirm),
            KeyCode::Esc => self.cmds.push(Cmd::Back),
            KeyCode::Tab => self.cmds.push(Cmd::Scores),
            KeyCode::Backspace => self.cmds.push(Cmd::Backspace),
            KeyCode::F(1) | KeyCode::Char('h' | 'H' | '?') => self.cmds.push(Cmd::Help),
            KeyCode::F(8) => self.cmds.push(Cmd::Mute),
            _ => {}
        }
    }

    pub fn on_paste(&mut self, s: &str) {
        self.text.push_str(s);
    }

    /// Drop presses, e.g. between screens.
    pub fn clear(&mut self) {
        self.bomb = false;
        self.mine = false;
        self.ship = None;
        self.cmds.clear();
        self.text.clear();
    }

    /// Controls for one step.
    pub fn controls(&mut self, now: Instant) -> Controls {
        let held = |k: Key| self.holds[k as usize].held(self.exact, now);
        Controls {
            thrust: held(Key::Thrust),
            reverse: held(Key::Reverse),
            left: held(Key::Left),
            right: held(Key::Right),
            fire: held(Key::Fire),
            burn: held(Key::Burn),
            bomb: std::mem::take(&mut self.bomb),
            mine: std::mem::take(&mut self.mine),
            ship: self.ship.take(),
        }
    }
}
