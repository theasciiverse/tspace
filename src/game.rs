//! Everything around the simulation: the title (with bots dogfighting
//! behind it), solo play against bots, hosting and joining, the in-game
//! menu, and turning world events into effects, sounds and the kill feed.
//!
//! Three roles share one state machine. Solo and Host run the simulation
//! and the bots; a Host also takes its guests' controls and sends everyone
//! the world 30 times a second. A Guest runs no simulation: it shows the
//! host's world and sends its controls.

use crate::audio::{Audio, Sfx};
use crate::bots::{Brain, Level, NAMES};
use crate::input::{Cmd, Input};
use crate::lan;
use crate::net::{self, ClientId, Evt, Net};
use crate::proto::{Msg, VERSION};
use crate::render::Fx;
use crate::ships::{self, Kind};
use crate::world::{Controls, Event, MAX_PLAYERS, World};
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

pub const DT: f32 = 1.0 / 60.0;
/// The title backdrop's arena.
pub const DEMO_ARENA: &str = "foundry";
const FEED_FOR: f32 = 6.0;
pub const TOAST_FOR: f32 = 4.0;
/// Frames to guests every this many ticks (60 Hz / 2 = 30 Hz).
const FRAME_EVERY: u64 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Title,
    /// Looking for games on the LAN.
    Browse,
    /// Typing or pasting an invite ticket.
    Ticket,
    /// Dialled a host, waiting for its welcome.
    Connecting,
    Play,
}

pub const MENU: [&str; 10] = ["Play against bots", "Host a game", "Join a LAN game", "Join with a ticket", "Arena", "Mode", "Bots", "Bot skill", "Ship", "Quit"];
pub const PAUSE_MENU: [&str; 2] = ["Back to the game", "Leave"];

/// How the program was started.
pub enum Start {
    Title,
    Solo,
    /// Bots only, the camera on the leader (catalog preview, attract mode).
    Demo,
    Host,
    Browse,
    Join(iroh::EndpointAddr),
}

pub struct Settings {
    pub arena: &'static str,
    pub teams: bool,
    pub bots: usize,
    pub level: Level,
    pub ship: Kind,
}

struct Remote {
    slot: usize,
    /// The last input applied from this guest.
    seq: u32,
    /// Inputs received, applied one per tick (so the guest's replay of
    /// unacknowledged inputs matches what the host did).
    queue: VecDeque<(u32, Controls)>,
    name: String,
    held: Controls,
    bomb: bool,
    mine: bool,
    ship: Option<Kind>,
}

pub struct Host {
    net: Net,
    pub ticket: Option<String>,
    guests: HashMap<ClientId, Remote>,
    out: Vec<Event>,
}

pub struct Guest {
    net: Net,
    pub host_name: String,
    ping_t: f32,
    seq: u32,
    /// Inputs sent and not yet acknowledged by a frame, for the prediction.
    unacked: VecDeque<(u32, Controls)>,
}

pub enum Role {
    Solo,
    Host(Box<Host>),
    Guest(Box<Guest>),
}

pub struct Game {
    pub phase: Phase,
    pub phase_t: f32,
    pub time: f32,
    pub menu: usize,
    pub settings: Settings,
    pub world: World,
    brains: Vec<Option<Brain>>,
    pub me: usize,
    pub role: Role,
    pub name: String,
    pub browser: Option<lan::Listener>,
    pub browse_pick: usize,
    pub ticket_text: String,
    pub fx: Fx,
    pub help: bool,
    pub scores: bool,
    pub paused_menu: Option<usize>,
    pub feed: VecDeque<(String, f32)>,
    pub toast: Option<(String, f32)>,
    pub exit: bool,
    pub exact_keys: bool,
    /// Watching a bots-only match.
    pub spectate: bool,
    /// A gamepad is connected (no keyboard tip needed).
    pub pad: bool,
    /// The low-energy beep has sounded for this dip.
    low_warned: bool,
    /// Guest: the host's events not yet turned into effects and sounds.
    pending: Vec<Event>,
    ticks: u64,
    seed: u64,
}

fn player_name() -> String {
    let n = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "Pilot".into());
    let n: String = n.chars().take(12).collect();
    if n.is_empty() { "Pilot".into() } else { n }
}

impl Game {
    pub fn new(start: Start, settings: Settings, exact_keys: bool) -> Game {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        let mut g = Game {
            phase: Phase::Title,
            phase_t: 0.0,
            time: 0.0,
            menu: 0,
            settings,
            world: World::new(DEMO_ARENA, seed),
            brains: (0..MAX_PLAYERS).map(|_| None).collect(),
            me: 0,
            role: Role::Solo,
            name: player_name(),
            browser: None,
            browse_pick: 0,
            ticket_text: String::new(),
            fx: Fx::default(),
            help: false,
            scores: false,
            paused_menu: None,
            feed: VecDeque::new(),
            toast: None,
            exit: false,
            exact_keys,
            spectate: false,
            pad: false,
            low_warned: false,
            pending: Vec::new(),
            ticks: 0,
            seed,
        };
        g.demo();
        match start {
            Start::Title => {}
            Start::Solo => g.start_solo(),
            Start::Demo => g.start_demo(),
            Start::Host => g.host(),
            Start::Browse => g.browse(),
            Start::Join(addr) => g.join(addr, "a ticket".into()),
        }
        g
    }

    pub fn is_guest(&self) -> bool {
        matches!(self.role, Role::Guest(_))
    }

    pub fn host_ticket(&self) -> Option<&str> {
        match &self.role {
            Role::Host(h) => h.ticket.as_deref(),
            _ => None,
        }
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        log::info!("toast: {msg}");
        self.toast = Some((msg, self.time));
    }

    fn set_phase(&mut self, p: Phase) {
        log::info!("phase {:?} -> {:?}", self.phase, p);
        self.phase = p;
        self.phase_t = 0.0;
    }

    fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.seed
    }

    fn fresh_world(&mut self, arena: &str) {
        let seed = self.next_seed();
        self.world = World::new(arena, seed);
        self.brains = (0..MAX_PLAYERS).map(|_| None).collect();
        self.fx = Fx::default();
        self.feed.clear();
        self.paused_menu = None;
        self.scores = false;
    }

    fn add_bot(&mut self) -> bool {
        let used: Vec<String> = self.world.s.players.iter().flatten().map(|p| p.name.clone()).collect();
        let start = (self.next_seed() % NAMES.len() as u64) as usize;
        let name = (0..NAMES.len()).map(|k| NAMES[(start + k) % NAMES.len()]).find(|n| !used.iter().any(|u| u == n)).unwrap_or("Bot");
        let mut brain = Brain::new(self.settings.level, self.next_seed() as u32);
        let ship = brain.pick_ship();
        match self.world.add_player(name.into(), true, ship) {
            Some(slot) => {
                self.brains[slot] = Some(brain);
                true
            }
            None => false,
        }
    }

    /// The title's backdrop: bots fighting, the camera on one of them.
    fn demo(&mut self) {
        self.fresh_world(DEMO_ARENA);
        for _ in 0..6 {
            self.add_bot();
        }
        self.me = 0;
    }

    fn start_play(&mut self) {
        self.spectate = false;
        self.fresh_world(self.settings.arena);
        self.world.set_teams(self.settings.teams);
        let name = self.name.clone();
        self.me = self.world.add_player(name, false, self.settings.ship).unwrap_or(0);
        for _ in 0..self.settings.bots.min(MAX_PLAYERS - 1) {
            self.add_bot();
        }
        self.set_phase(Phase::Play);
    }

    fn start_demo(&mut self) {
        self.role = Role::Solo;
        self.fresh_world(self.settings.arena);
        self.world.set_teams(self.settings.teams);
        for _ in 0..MAX_PLAYERS {
            self.add_bot();
        }
        self.me = 0;
        self.spectate = true;
        self.set_phase(Phase::Play);
    }

    /// Keep the camera on a living pilot, the best placed first.
    fn follow_leader(&mut self) {
        if self.world.player(self.me).is_none_or(|p| !p.alive)
            && let Some(&i) = self.world.ranking().iter().find(|&&i| self.world.player(i).is_some_and(|p| p.alive))
        {
            self.me = i;
        }
    }

    fn start_solo(&mut self) {
        self.role = Role::Solo;
        self.start_play();
        let goal = if self.world.s.teams { format!("Red against Blue, first team to {} kills", self.world.s.kill_limit) } else { format!("first to {} kills wins", self.world.s.kill_limit) };
        self.say(format!("{} bots, {} — {goal}", self.settings.bots, self.settings.level.name()));
    }

    fn host(&mut self) {
        self.role = Role::Host(Box::new(Host { net: Net::host(self.name.clone()), ticket: None, guests: HashMap::new(), out: Vec::new() }));
        self.start_play();
        self.say("Hosting: others can join from their LAN list, or with the ticket (Tab)");
    }

    fn browse(&mut self) {
        self.role = Role::Solo;
        match lan::Listener::start(lan::PORT) {
            Ok(l) => self.browser = Some(l),
            Err(e) => self.say(format!("Cannot listen for LAN games: {e}")),
        }
        self.browse_pick = 0;
        self.set_phase(Phase::Browse);
    }

    fn join(&mut self, addr: iroh::EndpointAddr, host_name: String) {
        log::info!("joining {host_name}");
        self.role = Role::Guest(Box::new(Guest { net: Net::join(addr, self.name.clone()), host_name, ping_t: 0.0, seq: 0, unacked: VecDeque::new() }));
        self.set_phase(Phase::Connecting);
    }

    /// Back to the title, leaving any network game.
    fn leave(&mut self, why: Option<String>) {
        self.role = Role::Solo;
        self.spectate = false;
        self.browser = None;
        if let Some(w) = why {
            self.say(w);
        }
        self.demo();
        self.set_phase(Phase::Title);
    }

    /// One tick: commands, network, simulation, and what to hear.
    pub fn tick(&mut self, input: &mut Input, audio: &mut Audio) {
        self.time += DT;
        self.phase_t += DT;
        self.ticks += 1;
        self.pad = input.pad_name.is_some();
        let mut cmds: Vec<Cmd> = input.cmds.drain(..).collect();
        if self.phase == Phase::Ticket {
            // letters are ticket text here, not shortcuts
            cmds.retain(|c| matches!(c, Cmd::Exit | Cmd::Back | Cmd::Confirm | Cmd::Backspace));
        }
        for c in &cmds {
            match c {
                Cmd::Exit => self.exit = true,
                Cmd::Mute => audio.toggle_mute(),
                Cmd::Help => self.help = !self.help,
                Cmd::Scores if self.phase == Phase::Play => self.scores = !self.scores,
                _ => {}
            }
        }
        if self.help && cmds.iter().any(|c| matches!(c, Cmd::Back | Cmd::Confirm)) {
            self.help = false;
            cmds.clear();
            input.clear();
        }
        self.poll_net();
        match self.phase {
            Phase::Title => self.title_tick(&cmds, input, audio),
            Phase::Browse => self.browse_tick(&cmds, input),
            Phase::Ticket => self.ticket_tick(&cmds, input),
            Phase::Connecting => {
                input.text.clear();
                if cmds.contains(&Cmd::Back) {
                    self.leave(Some("Cancelled".into()));
                }
            }
            Phase::Play => self.play_tick(&cmds, input, audio),
        }
        if self.phase != Phase::Play {
            audio.set_engine(0);
        }
    }

    fn title_tick(&mut self, cmds: &[Cmd], input: &mut Input, audio: &mut Audio) {
        input.text.clear();
        // the backdrop plays on, silently
        self.simulate(None);
        let events: Vec<Event> = self.world.events.drain(..).collect();
        for e in &events {
            self.fx.event(e, &self.world, self.me);
        }
        self.fx.step(DT, &self.world);
        self.follow_leader();
        if self.help {
            return;
        }
        let mut moved = false;
        for c in cmds {
            match c {
                Cmd::Up => self.menu = (self.menu + MENU.len() - 1) % MENU.len(),
                Cmd::Down => self.menu = (self.menu + 1) % MENU.len(),
                Cmd::Left | Cmd::Right => {
                    let up = *c == Cmd::Right;
                    match self.menu {
                        4 => {
                            let ids: Vec<&'static str> = crate::arena::BUILTIN.iter().map(|(id, _)| *id).collect();
                            let k = ids.iter().position(|&a| a == self.settings.arena).unwrap_or(0);
                            self.settings.arena = ids[if up { (k + 1) % ids.len() } else { (k + ids.len() - 1) % ids.len() }];
                        }
                        5 => self.settings.teams = !self.settings.teams,
                        6 => self.settings.bots = if up { (self.settings.bots + 1).min(MAX_PLAYERS - 1) } else { self.settings.bots.saturating_sub(1) },
                        7 => {
                            let k = Level::ALL.iter().position(|&l| l == self.settings.level).unwrap_or(1);
                            self.settings.level = Level::ALL[if up { (k + 1) % 3 } else { (k + 2) % 3 }];
                        }
                        8 => {
                            let k = self.settings.ship.index();
                            self.settings.ship = ships::ALL[if up { (k + 1) % 4 } else { (k + 3) % 4 }];
                        }
                        _ => continue,
                    }
                }
                _ => continue,
            }
            moved = true;
        }
        if moved {
            audio.play(Sfx::Select, 0.5);
        }
        if cmds.contains(&Cmd::Back) {
            self.exit = true;
        } else if cmds.contains(&Cmd::Confirm) {
            input.clear();
            match self.menu {
                0 => self.start_solo(),
                1 => self.host(),
                2 => self.browse(),
                3 => {
                    self.ticket_text.clear();
                    self.set_phase(Phase::Ticket);
                }
                4..=8 => {}
                _ => self.exit = true,
            }
        }
    }

    fn browse_tick(&mut self, cmds: &[Cmd], input: &mut Input) {
        input.text.clear();
        let hosts = self.browser.as_ref().map(|b| b.hosts()).unwrap_or_default();
        let n = hosts.len();
        for c in cmds {
            match c {
                Cmd::Up if n > 0 => self.browse_pick = (self.browse_pick + n - 1) % n,
                Cmd::Down if n > 0 => self.browse_pick = (self.browse_pick + 1) % n,
                _ => {}
            }
        }
        if cmds.contains(&Cmd::Back) {
            self.leave(None);
        } else if cmds.contains(&Cmd::Confirm) && n > 0 {
            input.clear();
            let pick = &hosts[self.browse_pick.min(n - 1)];
            if pick.beacon.players as usize >= MAX_PLAYERS {
                self.say(format!("{}'s game is full", pick.beacon.name));
            } else {
                let (addr, name) = (pick.addr(), pick.beacon.name.clone());
                self.browser = None;
                self.join(addr, name);
            }
        }
    }

    fn ticket_tick(&mut self, cmds: &[Cmd], input: &mut Input) {
        let typed: String = std::mem::take(&mut input.text);
        self.ticket_text.extend(typed.chars().filter(|c| c.is_ascii_alphanumeric()));
        if cmds.contains(&Cmd::Backspace) {
            self.ticket_text.pop();
        }
        if cmds.contains(&Cmd::Back) {
            self.leave(None);
        } else if cmds.contains(&Cmd::Confirm) && !typed.contains(' ') {
            match net::decode_ticket(&self.ticket_text) {
                Ok(addr) => {
                    input.clear();
                    self.join(addr, "the ticket's host".into());
                }
                Err(e) => self.say(e),
            }
        }
    }

    fn play_tick(&mut self, cmds: &[Cmd], input: &mut Input, audio: &mut Audio) {
        input.text.clear();
        if !self.help {
            if let Some(k) = self.paused_menu {
                let n = PAUSE_MENU.len();
                for c in cmds {
                    match c {
                        Cmd::Up => self.paused_menu = Some((k + n - 1) % n),
                        Cmd::Down => self.paused_menu = Some((k + 1) % n),
                        _ => {}
                    }
                }
                if cmds.contains(&Cmd::Back) {
                    self.paused_menu = None;
                    input.clear();
                } else if cmds.contains(&Cmd::Confirm) {
                    input.clear();
                    match self.paused_menu {
                        Some(0) => self.paused_menu = None,
                        _ => return self.leave(None),
                    }
                }
            } else if cmds.contains(&Cmd::Back) {
                self.paused_menu = Some(0);
            }
        }
        // a solo game stands still while a menu is up
        let frozen = matches!(self.role, Role::Solo) && (self.paused_menu.is_some() || self.help);
        let mine = if self.paused_menu.is_some() || self.help { Controls::default() } else { input.controls(Instant::now()) };
        let mine = if self.paused_menu.is_some() || self.help { Controls { ship: mine.ship, ..Default::default() } } else { mine };
        if let Role::Guest(g) = &mut self.role {
            g.seq = g.seq.wrapping_add(1);
            g.net.send(net::Cmd::Broadcast(Msg::Input { seq: g.seq, controls: mine }));
            g.unacked.push_back((g.seq, mine));
            while g.unacked.len() > 240 {
                g.unacked.pop_front();
            }
            // fly our own ship now; let the rest coast until the next frame
            self.world.fly(self.me, mine, DT);
            self.world.coast(self.me, DT);
            g.ping_t += DT;
            if g.ping_t > 2.0 {
                g.ping_t = 0.0;
                g.net.send(net::Cmd::Broadcast(Msg::Ping));
            }
            let events = std::mem::take(&mut self.pending);
            self.react(&events, audio);
        } else if !frozen {
            if self.spectate {
                self.simulate(None);
                self.follow_leader();
            } else {
                self.simulate(Some(mine));
            }
            let events: Vec<Event> = self.world.events.drain(..).collect();
            self.react(&events, audio);
            if let Role::Host(h) = &mut self.role {
                h.out.extend(events);
            }
            self.send_frame();
        }
        if !frozen {
            self.fx.step(DT, &self.world);
        }
        // beep once each time our energy dips under a quarter
        if let Some(p) = self.world.player(self.me).filter(|p| p.alive && !self.spectate) {
            let frac = p.energy / p.max_energy.max(1.0);
            if frac < 0.25 && !self.low_warned {
                self.low_warned = true;
                audio.play(Sfx::Low, 0.7);
            } else if frac > 0.4 {
                self.low_warned = false;
            }
        }
        let engine = self.world.player(self.me).filter(|p| p.alive && p.thrusting).map_or(0, |p| if p.burning { 2 } else { 1 });
        audio.set_engine(if frozen { 0 } else { engine });
        while self.feed.front().is_some_and(|(_, t)| self.time - t > FEED_FOR) {
            self.feed.pop_front();
        }
    }

    /// One step of the world: our controls (if playing), guests', bots'.
    fn simulate(&mut self, mine: Option<Controls>) {
        let mut cs = vec![Controls::default(); MAX_PLAYERS];
        for (i, (c, brain)) in cs.iter_mut().zip(self.brains.iter_mut()).enumerate() {
            if let Some(b) = brain {
                *c = b.think(&self.world, i, DT);
            }
        }
        if let Some(c) = mine {
            cs[self.me] = c;
        }
        if let Role::Host(h) = &mut self.role {
            for r in h.guests.values_mut() {
                // one input per tick; a backlog (a burst after a stall) is caught up
                while r.queue.len() > 3 {
                    let (seq, c) = r.queue.pop_front().unwrap();
                    (r.seq, r.held) = (seq, c);
                    r.bomb |= c.bomb;
                    r.mine |= c.mine;
                    r.ship = c.ship.or(r.ship);
                }
                if let Some((seq, c)) = r.queue.pop_front() {
                    (r.seq, r.held) = (seq, c);
                    r.bomb |= c.bomb;
                    r.mine |= c.mine;
                    r.ship = c.ship.or(r.ship);
                }
                let mut c = r.held;
                c.bomb = std::mem::take(&mut r.bomb);
                c.mine = std::mem::take(&mut r.mine);
                c.ship = r.ship.take();
                cs[r.slot] = c;
            }
        }
        self.world.step(DT, &cs);
    }

    fn name_of(&self, who: u8) -> String {
        self.world.player(who as usize).map_or_else(|| "someone".into(), |p| p.name.clone())
    }

    /// Effects, sounds and the kill feed for what just happened.
    fn react(&mut self, events: &[Event], audio: &mut Audio) {
        let me = self.me;
        let my_pos = self.world.player(me).filter(|p| p.alive).map(|p| (p.x, p.y));
        let near = |x: f32, y: f32| my_pos.map_or(0.6, |(mx, my)| (1.0 - (mx - x).hypot(my - y) / 900.0).clamp(0.0, 1.0));
        for e in events {
            self.fx.event(e, &self.world, me);
            match *e {
                Event::Fire { who, x, y } => {
                    if who as usize == me {
                        audio.play(Sfx::Gun, 0.8);
                    } else if near(x, y) > 0.3 {
                        audio.play(Sfx::GunFar, near(x, y));
                    }
                }
                Event::Bomb { x, y, .. } => {
                    if near(x, y) > 0.2 {
                        audio.play(Sfx::Bomb, near(x, y));
                    }
                }
                Event::Pulse { x, y, .. } => {
                    if near(x, y) > 0.05 {
                        audio.play(Sfx::Bomb, near(x, y));
                    }
                }
                Event::Blast { x, y, .. } => {
                    if near(x, y) > 0.05 {
                        audio.play(Sfx::Blast, near(x, y));
                    }
                }
                Event::Hit { who, .. } => {
                    if who as usize == me {
                        audio.play(Sfx::Hit, 0.9);
                    }
                }
                Event::Bounce { who } => {
                    if who as usize == me {
                        audio.play(Sfx::Bounce, 0.6);
                    }
                }
                Event::Died { who, by, x, y } => {
                    audio.play(Sfx::Death, if who as usize == me { 1.0 } else { near(x, y).max(0.15) });
                    let victim = self.name_of(who);
                    let line = match by {
                        Some(k) => format!("{} shot down {victim}", self.name_of(k)),
                        None => format!("{victim} blew up"),
                    };
                    self.feed_line(line);
                }
                Event::MineSet { who } => {
                    if who as usize == me {
                        audio.play(Sfx::MineSet, 0.7);
                    }
                }
                Event::Pickup { who, kind } => {
                    if who as usize == me {
                        audio.play(Sfx::Pickup, 0.8);
                        self.say(kind.name().to_string());
                    }
                }
                Event::Spawn { who } => {
                    if who as usize == me {
                        audio.play(Sfx::Spawn, 0.6);
                    }
                }
                Event::Streak { who, n } => {
                    let line = format!("{} is on a streak of {n}!", self.name_of(who));
                    self.feed_line(line);
                }
                Event::StreakEnded { who, by, n } => {
                    let line = match by {
                        Some(k) => format!("{} ended {}'s streak of {n}", self.name_of(k), self.name_of(who)),
                        None => format!("{}'s streak of {n} is over", self.name_of(who)),
                    };
                    self.feed_line(line);
                }
                Event::TeamWon { team } => {
                    audio.play(Sfx::Win, 0.9);
                    let line = format!("{} team wins the match!", crate::world::TEAM_NAMES[team as usize % 2]);
                    self.feed.push_back((line, self.time));
                }
                Event::Won { who } => {
                    audio.play(Sfx::Win, 0.9);
                    let line = format!("{} wins the match!", self.name_of(who));
                    self.feed.push_back((line, self.time));
                }
            }
        }
    }

    fn feed_line(&mut self, line: String) {
        log::info!("feed: {line}");
        self.feed.push_back((line, self.time));
        while self.feed.len() > 5 {
            self.feed.pop_front();
        }
    }

    fn poll_net(&mut self) {
        let mut evts = Vec::new();
        match &self.role {
            Role::Host(h) => evts.extend(std::iter::from_fn(|| h.net.poll())),
            Role::Guest(g) => evts.extend(std::iter::from_fn(|| g.net.poll())),
            Role::Solo => return,
        }
        for e in evts {
            if !matches!(e, Evt::ClientMsg { msg: Msg::Input { .. } | Msg::Ping, .. } | Evt::HostMsg(Msg::Frame { .. })) {
                log::debug!("net {e:?}");
            }
            match e {
                Evt::Hosting { id } => log::info!("hosting as {id}"),
                Evt::Ticket(t) => {
                    write_ticket(&t);
                    if let Role::Host(h) = &mut self.role {
                        h.ticket = Some(t);
                    }
                }
                Evt::ClientJoined { client, name } => self.guest_joined(client, name),
                Evt::ClientMsg { client, msg } => {
                    let Role::Host(h) = &mut self.role else { continue };
                    let Some(r) = h.guests.get_mut(&client) else { continue };
                    if let Msg::Input { seq, controls: c } = msg {
                        r.queue.push_back((seq, c));
                    }
                }
                Evt::ClientLeft { client } => {
                    let Role::Host(h) = &mut self.role else { continue };
                    if let Some(r) = h.guests.remove(&client) {
                        self.world.remove_player(r.slot);
                        self.say(format!("{} left", r.name));
                        if self.world.count(true) < self.settings.bots {
                            self.add_bot();
                        }
                    }
                }
                Evt::Connected => log::info!("connected, waiting for the welcome"),
                Evt::HostMsg(Msg::Welcome { slot, arena }) => {
                    let name = if let Role::Guest(g) = &self.role { g.host_name.clone() } else { String::new() };
                    if !crate::arena::BUILTIN.iter().any(|(id, _)| *id == arena) {
                        self.leave(Some(format!("The host plays an arena this version lacks ({arena})")));
                        continue;
                    }
                    self.fresh_world(&arena);
                    self.me = slot as usize;
                    self.set_phase(Phase::Play);
                    self.say(format!("Joined {name}'s game"));
                }
                Evt::HostMsg(Msg::Frame { state, events, acks }) => {
                    if self.phase == Phase::Play {
                        let before = self.world.player(self.me).filter(|p| p.alive).map(|p| (p.x, p.y));
                        self.world.s = state;
                        self.guest_events(events);
                        // replay what the host has not applied yet
                        let me = self.me;
                        if let Role::Guest(g) = &mut self.role {
                            if let Some(&(_, ack)) = acks.iter().find(|(slot, _)| *slot as usize == me) {
                                while g.unacked.front().is_some_and(|(seq, _)| seq.wrapping_sub(ack) as i32 <= 0) {
                                    g.unacked.pop_front();
                                }
                            }
                            for &(_, c) in &g.unacked {
                                self.world.fly(me, c, DT);
                            }
                            // how far off the prediction was (latency, lost inputs, hits)
                            if let (Some((bx, by)), Some(p)) = (before, self.world.player(me).filter(|p| p.alive)) {
                                let off = (p.x - bx).hypot(p.y - by);
                                if off > 12.0 {
                                    log::debug!("prediction corrected by {off:.0} px ({} inputs pending)", g.unacked.len());
                                }
                            }
                        }
                    }
                }
                Evt::HostMsg(Msg::Reject { reason }) => self.leave(Some(format!("Turned away: {reason}"))),
                Evt::HostMsg(_) => {}
                Evt::Failed(why) => self.leave(Some(why)),
                Evt::Disconnected => self.leave(Some("The host left the game".into())),
            }
        }
    }

    /// Guest: keep the host's events for the next tick, where audio is at hand.
    fn guest_events(&mut self, events: Vec<Event>) {
        self.pending.extend(events);
    }

    fn guest_joined(&mut self, client: ClientId, name: String) {
        let Role::Host(_) = &self.role else { return };
        if self.world.count(false) >= MAX_PLAYERS {
            self.reject(client, "the game is full");
            return;
        }
        // make room: a bot gives up its seat
        if self.world.s.players.iter().all(|p| p.is_some())
            && let Some(bot) = (0..MAX_PLAYERS).rev().find(|&i| self.world.player(i).is_some_and(|p| p.bot)) {
                self.world.remove_player(bot);
                self.brains[bot] = None;
            }
        // two pilots with one name get numbers
        let taken = |n: &str| self.world.s.players.iter().flatten().any(|p| p.name == n);
        let name = if taken(&name) { (2..).map(|k| format!("{name} {k}")).find(|n| !taken(n)).unwrap() } else { name };
        let Some(slot) = self.world.add_player(name.clone(), false, Kind::Lance) else {
            self.reject(client, "the game is full");
            return;
        };
        let Role::Host(h) = &mut self.role else { return };
        h.net.send(net::Cmd::SendTo(client, Msg::Welcome { slot: slot as u8, arena: self.world.arena_id.clone() }));
        h.guests.insert(client, Remote { slot, seq: 0, queue: VecDeque::new(), name: name.clone(), held: Controls::default(), bomb: false, mine: false, ship: None });
        self.say(format!("{name} joined"));
    }

    fn reject(&mut self, client: ClientId, reason: &str) {
        if let Role::Host(h) = &mut self.role {
            h.net.send(net::Cmd::SendTo(client, Msg::Reject { reason: reason.into() }));
            h.net.send(net::Cmd::Kick(client));
        }
    }

    /// Host: the world to every guest, every `FRAME_EVERY` ticks.
    fn send_frame(&mut self) {
        let players = self.world.s.players.iter().flatten().count() as u8;
        let humans = self.world.count(false) as u8;
        let ticks = self.ticks;
        let Role::Host(h) = &mut self.role else { return };
        if ticks.is_multiple_of(30) {
            h.net.send(net::Cmd::Beacon { players: humans.max(1), in_game: players as usize >= MAX_PLAYERS && humans as usize >= MAX_PLAYERS });
        }
        if h.guests.is_empty() {
            h.out.clear();
            return;
        }
        if !ticks.is_multiple_of(FRAME_EVERY) {
            return;
        }
        let events = std::mem::take(&mut h.out);
        let acks = h.guests.values().map(|r| (r.slot as u8, r.seq)).collect();
        h.net.send(net::Cmd::Broadcast(Msg::Frame { state: self.world.s.clone(), events, acks }));
    }
}

/// The host's ticket, also in a file so it can be copied from outside.
fn write_ticket(t: &str) {
    if let Some(dir) = crate::state_dir() {
        let path = dir.join("ticket.txt");
        match std::fs::write(&path, format!("{t}\n")) {
            Ok(()) => log::info!("ticket written to {}", path.display()),
            Err(e) => log::warn!("could not write {}: {e}", path.display()),
        }
    }
}

pub fn version_string() -> String {
    format!("tspace {} (protocol {VERSION})", env!("CARGO_PKG_VERSION"))
}
