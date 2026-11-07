//! Keyboard.
//!
//! - W / Up: thrust, S / Down: reverse, A D / Left Right: turn
//! - Space: guns (hold), F: bomb, X: mine, E: afterburner (hold)
//! - 1-4: change ship, Tab: scores, Esc: menu, H / F1: help, F8: sound
//!
//! Gamepad (gilrs, as Panzer): left stick or d-pad turns and thrusts (up)
//! or brakes (down), right trigger fires, left trigger is the afterburner,
//! A bomb, X mine, Y scores, B or Start menu, Back help. A pad reports
//! every button exactly, in any terminal. Only pads in SDL's controller
//! database count (gilrs also reports tablets and 3D mice);
//! `TSPACE_ANY_PAD=1` accepts everything.
//!
//! With the kitty keyboard protocol presses and releases are exact, and
//! thrusting while turning and firing works as it should. Elsewhere a key
//! counts as held while press/repeat events keep arriving (Panzer's rule),
//! and terminals repeat only the last key pressed.

use crate::ships::{self, Kind};
use crate::world::Controls;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

#[cfg(feature = "gamepad")]
const STICK_TURN: f32 = 0.35;
#[cfg(feature = "gamepad")]
const STICK_THRUST: f32 = 0.5;

#[cfg(feature = "gamepad")]
fn usable_pad(p: &gilrs::Gamepad) -> bool {
    p.mapping_source() == gilrs::MappingSource::SdlMappings || std::env::var("TSPACE_ANY_PAD").is_ok_and(|v| v == "1")
}

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
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    #[cfg(feature = "gamepad")]
    pad: Option<gilrs::GamepadId>,
    pub pad_name: Option<String>,
}

impl Input {
    pub fn new(exact: bool) -> Input {
        Input {
            exact,
            holds: [Hold::default(); KEYS],
            bomb: false,
            mine: false,
            ship: None,
            cmds: Vec::new(),
            text: String::new(),
            #[cfg(feature = "gamepad")]
            gilrs: None,
            #[cfg(feature = "gamepad")]
            pad: None,
            pad_name: None,
        }
    }

    pub fn open_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        match gilrs::Gilrs::new() {
            Ok(g) => {
                let all: Vec<_> = g.gamepads().map(|(id, p)| (id, p.name().to_string(), usable_pad(&p))).collect();
                log::info!("gamepads (name, used): {:?}", all.iter().map(|(_, n, u)| (n, u)).collect::<Vec<_>>());
                if let Some((id, name, _)) = all.into_iter().find(|(_, _, used)| *used) {
                    self.pad = Some(id);
                    self.pad_name = Some(name);
                }
                self.gilrs = Some(g);
            }
            Err(e) => log::warn!("gamepad support unavailable: {e}"),
        }
    }

    /// Drain gamepad events into presses and commands; call once per frame.
    pub fn poll_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        {
            use gilrs::{Button, EventType};
            let Some(g) = &mut self.gilrs else { return };
            let mut events = Vec::new();
            while let Some(ev) = g.next_event() {
                events.push(ev);
            }
            for ev in events {
                let g = self.gilrs.as_ref().unwrap();
                let pad = g.gamepad(ev.id);
                if !usable_pad(&pad) {
                    continue;
                }
                match ev.event {
                    EventType::Connected => {
                        self.pad = Some(ev.id);
                        self.pad_name = Some(pad.name().to_string());
                        log::info!("gamepad connected: {:?}", self.pad_name);
                    }
                    EventType::Disconnected if self.pad == Some(ev.id) => {
                        log::info!("gamepad disconnected");
                        self.pad = None;
                        self.pad_name = None;
                    }
                    EventType::ButtonPressed(b, _) => {
                        self.pad = Some(ev.id);
                        log::debug!("pad button {b:?}");
                        match b {
                            Button::South => {
                                self.bomb = true;
                                self.cmds.push(Cmd::Confirm);
                            }
                            Button::West => self.mine = true,
                            Button::North => self.cmds.push(Cmd::Scores),
                            Button::East | Button::Start => self.cmds.push(Cmd::Back),
                            Button::Select => self.cmds.push(Cmd::Help),
                            Button::DPadUp => self.cmds.push(Cmd::Up),
                            Button::DPadDown => self.cmds.push(Cmd::Down),
                            Button::DPadLeft => self.cmds.push(Cmd::Left),
                            Button::DPadRight => self.cmds.push(Cmd::Right),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// What the pad holds right now: (thrust, reverse, left, right, fire, burn).
    fn pad_held(&self) -> [bool; 6] {
        #[cfg(feature = "gamepad")]
        if let (Some(g), Some(id)) = (&self.gilrs, self.pad) {
            use gilrs::{Axis, Button};
            let p = g.gamepad(id);
            let axis = |a: Axis| p.axis_data(a).map_or(0.0, |d| d.value());
            let button = |b: Button| p.is_pressed(b);
            let trigger = |b: Button, bumper: Button| p.button_data(b).is_some_and(|d| d.value() > 0.3) || button(bumper);
            let (x, y) = (axis(Axis::LeftStickX), axis(Axis::LeftStickY));
            return [
                y > STICK_THRUST || button(Button::DPadUp),
                y < -STICK_THRUST || button(Button::DPadDown),
                x < -STICK_TURN || button(Button::DPadLeft),
                x > STICK_TURN || button(Button::DPadRight),
                trigger(Button::RightTrigger2, Button::RightTrigger),
                trigger(Button::LeftTrigger2, Button::LeftTrigger),
            ];
        }
        [false; 6]
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
        let pad = self.pad_held();
        let held = |k: Key| self.holds[k as usize].held(self.exact, now) || pad[k as usize];
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
