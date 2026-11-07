//! The simulation: ships, shots, bombs, mines, pickups and the match.
//!
//! `State` is everything that changes and is what the host sends to its
//! guests, so a guest draws exactly what the host simulates. The arena is
//! fixed and known to both from its id.

use crate::arena::{Arena, TILE};
use crate::ships::Kind;
use crate::zubspace::{self as zs, MovementVector, PlayerMovementInput, PlayerMovementState};
use serde::{Deserialize, Serialize};

pub const MAX_PLAYERS: usize = 8;
pub const KILL_LIMIT: u32 = 15;
/// Team games play to this many kills for the team.
pub const TEAM_KILL_LIMIT: u32 = 25;
pub const TEAM_NAMES: [&str; 2] = ["Red", "Blue"];
const RESPAWN: f32 = 3.0;
const WARP_OUT: f32 = 1.2;
const SPAWN_GUARD: f32 = 1.5;
const MINE_ARM: f32 = 0.7;
const MINE_LIFE: f32 = 90.0;
const MINE_REACH: f32 = 26.0;
const MINE_BLAST: f32 = 56.0;
const BOMB_LIFE: f32 = 2.6;
const PICKUP_EVERY: f32 = 15.0;
const PICKUP_REACH: f32 = 14.0;
const EFFECT_FOR: f32 = 12.0;
const NEXT_MATCH_AFTER: f32 = 10.0;

/// One player's controls for one step. `fire` and `burn` are held; `bomb`,
/// `mine` and `ship` are presses, kept by the caller until a step uses them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Controls {
    pub thrust: bool,
    pub reverse: bool,
    pub left: bool,
    pub right: bool,
    pub burn: bool,
    pub fire: bool,
    pub bomb: bool,
    pub mine: bool,
    pub ship: Option<Kind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickupKind {
    /// More maximum energy for this life, and a full tank.
    Overcharge,
    /// Guns fire a three-way spread.
    Scatter,
    /// Off everyone's radar and hard to see.
    Shroud,
    /// More thrust and top speed.
    Overdrive,
}

pub const PICKUPS: [PickupKind; 4] = [PickupKind::Overcharge, PickupKind::Scatter, PickupKind::Shroud, PickupKind::Overdrive];

impl PickupKind {
    pub fn name(self) -> &'static str {
        match self {
            PickupKind::Overcharge => "Overcharge",
            PickupKind::Scatter => "Scatter",
            PickupKind::Shroud => "Shroud",
            PickupKind::Overdrive => "Overdrive",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub bot: bool,
    pub kind: Kind,
    pub alive: bool,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// Turns, 0 = up, clockwise.
    pub orient: f32,
    pub energy: f32,
    pub max_energy: f32,
    /// Seconds until respawn while dead.
    pub respawn: f32,
    pub gun_cd: f32,
    pub bomb_cd: f32,
    pub kills: u32,
    pub deaths: u32,
    pub guard: f32,
    pub scatter: f32,
    pub shroud: f32,
    pub overdrive: f32,
    pub thrusting: bool,
    pub burning: bool,
    pub last_hit: Option<u8>,
    /// 0 or 1 in a team game (unused in free-for-all).
    pub team: u8,
    /// Kills since this player last died.
    pub streak: u32,
    /// Seconds of the white flash after a hit.
    pub flash: f32,
}

impl Player {
    fn new(name: String, bot: bool, kind: Kind) -> Player {
        Player {
            name,
            bot,
            kind,
            alive: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            orient: 0.0,
            energy: 0.0,
            max_energy: kind.stats().energy,
            respawn: 0.5,
            gun_cd: 0.0,
            bomb_cd: 0.0,
            kills: 0,
            deaths: 0,
            guard: 0.0,
            scatter: 0.0,
            shroud: 0.0,
            overdrive: 0.0,
            thrusting: false,
            burning: false,
            last_hit: None,
            flash: 0.0,
            team: 0,
            streak: 0,
        }
    }

    pub fn heading(&self) -> (f32, f32) {
        let h = zs::orientation_to_heading(self.orient);
        (h.x, h.y)
    }

    pub fn radius(&self) -> f32 {
        self.kind.stats().radius
    }

    pub fn speed(&self) -> f32 {
        self.vx.hypot(self.vy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShotKind {
    Gun,
    Bomb,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Shot {
    pub owner: u8,
    pub kind: ShotKind,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub bounces: u8,
    pub damage: f32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Mine {
    pub owner: u8,
    pub x: f32,
    pub y: f32,
    pub age: f32,
    pub damage: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Spot {
    pub tile: (u16, u16),
    pub ready: Option<PickupKind>,
    pub timer: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Fire { who: u8, x: f32, y: f32 },
    Bomb { who: u8, x: f32, y: f32 },
    Bounce { who: u8 },
    Hit { who: u8, x: f32, y: f32 },
    Blast { x: f32, y: f32, r: f32 },
    Died { who: u8, by: Option<u8>, x: f32, y: f32 },
    MineSet { who: u8 },
    Pickup { who: u8, kind: PickupKind },
    Spawn { who: u8 },
    Won { who: u8 },
    TeamWon { team: u8 },
    /// A kill streak reached 3, 5, 8, then every 5 more.
    Streak { who: u8, n: u32 },
    /// A streak of 3 or more ended by `by` (None: blew up).
    StreakEnded { who: u8, by: Option<u8>, n: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub time: f32,
    pub players: Vec<Option<Player>>,
    pub shots: Vec<Shot>,
    pub mines: Vec<Mine>,
    pub spots: Vec<Spot>,
    /// Winner (a player, or a team in a team game) and seconds left until
    /// the next match.
    pub over: Option<(u8, f32)>,
    /// Two teams instead of everyone for themselves.
    pub teams: bool,
    pub kill_limit: u32,
    pub rng: u64,
}

pub struct World {
    pub arena: Arena,
    pub arena_id: String,
    pub s: State,
    /// What happened in the last steps; the caller drains it.
    pub events: Vec<Event>,
}

impl World {
    pub fn new(arena_id: &str, seed: u64) -> World {
        let arena = Arena::builtin(arena_id);
        let spots = arena.pickups.iter().map(|&(x, y)| Spot { tile: (x as u16, y as u16), ready: None, timer: 2.0 }).collect();
        World {
            arena,
            arena_id: arena_id.to_string(),
            s: State {
                time: 0.0,
                players: vec![None; MAX_PLAYERS],
                shots: Vec::new(),
                mines: Vec::new(),
                spots,
                over: None,
                kill_limit: KILL_LIMIT,
                teams: false,
                rng: seed | 1,
            },
            events: Vec::new(),
        }
    }

    fn rand(&mut self) -> f32 {
        let s = &mut self.s.rng;
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        (*s >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn add_player(&mut self, name: String, bot: bool, kind: Kind) -> Option<usize> {
        let slot = self.s.players.iter().position(|p| p.is_none())?;
        log::info!("player {slot} joins: {name} (bot {bot}, {:?})", kind);
        let mut p = Player::new(name, bot, kind);
        // join the smaller team
        let reds = self.s.players.iter().flatten().filter(|q| q.team == 0).count();
        let blues = self.s.players.iter().flatten().filter(|q| q.team == 1).count();
        p.team = (blues < reds) as u8;
        self.s.players[slot] = Some(p);
        Some(slot)
    }

    /// Team game on or off; call before adding players.
    pub fn set_teams(&mut self, on: bool) {
        self.s.teams = on;
        self.s.kill_limit = if on { TEAM_KILL_LIMIT } else { KILL_LIMIT };
    }

    /// Can `a` hurt `b`? Never itself, and never a teammate.
    pub fn foes(&self, a: usize, b: usize) -> bool {
        if a == b {
            return false;
        }
        if !self.s.teams {
            return true;
        }
        match (self.player(a), self.player(b)) {
            (Some(p), Some(q)) => p.team != q.team,
            _ => true,
        }
    }

    pub fn team_kills(&self, team: u8) -> u32 {
        self.s.players.iter().flatten().filter(|p| p.team == team).map(|p| p.kills).sum()
    }

    pub fn remove_player(&mut self, slot: usize) {
        if let Some(p) = self.s.players.get_mut(slot).and_then(Option::take) {
            log::info!("player {slot} leaves: {}", p.name);
        }
        self.s.mines.retain(|m| m.owner as usize != slot);
        self.s.shots.retain(|m| m.owner as usize != slot);
    }

    pub fn player(&self, slot: usize) -> Option<&Player> {
        self.s.players.get(slot).and_then(Option::as_ref)
    }

    pub fn count(&self, bots: bool) -> usize {
        self.s.players.iter().flatten().filter(|p| p.bot == bots).count()
    }

    /// Advance `dt` seconds; `controls[i]` steers player slot `i`.
    pub fn step(&mut self, dt: f32, controls: &[Controls]) {
        self.s.time += dt;
        if let Some((winner, left)) = self.s.over {
            let left = left - dt;
            if left <= 0.0 {
                self.next_match();
            } else {
                self.s.over = Some((winner, left));
            }
        }
        for i in 0..MAX_PLAYERS {
            let c = controls.get(i).copied().unwrap_or_default();
            self.step_player(i, c, dt);
        }
        self.step_shots(dt);
        self.step_mines(dt);
        self.step_spots(dt);
        if self.s.over.is_none() {
            if self.s.teams {
                if let Some(team) = (0..2u8).find(|&t| self.team_kills(t) >= self.s.kill_limit) {
                    log::info!("match won by team {team}");
                    self.s.over = Some((team, NEXT_MATCH_AFTER));
                    self.events.push(Event::TeamWon { team });
                }
            } else if let Some((i, _)) = self.s.players.iter().enumerate().filter_map(|(i, p)| p.as_ref().map(|p| (i, p))).find(|(_, p)| p.kills >= self.s.kill_limit) {
                log::info!("match won by player {i}");
                self.s.over = Some((i as u8, NEXT_MATCH_AFTER));
                self.events.push(Event::Won { who: i as u8 });
            }
        }
    }

    fn next_match(&mut self) {
        self.s.over = None;
        self.s.shots.clear();
        self.s.mines.clear();
        for p in self.s.players.iter_mut().flatten() {
            p.kills = 0;
            p.deaths = 0;
            p.streak = 0;
            p.alive = false;
            p.respawn = 0.5;
        }
        log::info!("next match");
    }

    fn spawn_point(&mut self, who: usize) -> (f32, f32) {
        let enemies: Vec<(f32, f32)> = self.s.players.iter().enumerate().filter(|(i, _)| *i != who).filter_map(|(_, p)| p.as_ref()).filter(|p| p.alive).map(|p| (p.x, p.y)).collect();
        let n = self.arena.spawns.len();
        let mut best = (f32::MIN, self.arena.spawns[0]);
        for _ in 0..4 {
            let k = ((self.rand() * n as f32) as usize).min(n - 1);
            let t = self.arena.spawns[k];
            let (x, y) = Arena::centre(t);
            let d = enemies.iter().map(|e| (e.0 - x).hypot(e.1 - y)).fold(f32::MAX, f32::min);
            if d > best.0 {
                best = (d, t);
            }
        }
        Arena::centre(best.1)
    }

    fn step_player(&mut self, i: usize, c: Controls, dt: f32) {
        let Some(p) = self.s.players[i].as_ref() else { return };
        // a ship change: warp out now, back in the new ship shortly
        if let Some(k) = c.ship {
            let alive = p.alive;
            let p = self.s.players[i].as_mut().unwrap();
            if k != p.kind || !alive {
                p.kind = k;
                if alive {
                    p.alive = false;
                    p.respawn = WARP_OUT;
                }
            }
        }
        let p = self.s.players[i].as_ref().unwrap();
        if !p.alive {
            let respawn = p.respawn - dt;
            if respawn <= 0.0 && self.s.over.is_none() {
                let (x, y) = self.spawn_point(i);
                let orient = self.rand();
                let p = self.s.players[i].as_mut().unwrap();
                let st = p.kind.stats();
                p.alive = true;
                (p.x, p.y, p.vx, p.vy, p.orient) = (x, y, 0.0, 0.0, orient);
                p.max_energy = st.energy;
                p.energy = st.energy;
                p.guard = SPAWN_GUARD;
                (p.scatter, p.shroud, p.overdrive) = (0.0, 0.0, 0.0);
                p.last_hit = None;
                self.events.push(Event::Spawn { who: i as u8 });
            } else {
                self.s.players[i].as_mut().unwrap().respawn = respawn;
            }
            return;
        }

        let p = self.s.players[i].as_mut().unwrap();
        let st = p.kind.stats();
        let r = st.radius;
        if move_ship(&self.arena, p, c, dt) {
            self.events.push(Event::Bounce { who: i as u8 });
        }

        // weapons
        let p = self.s.players[i].as_mut().unwrap();
        let (hx, hy) = p.heading();
        let mut new_shots = Vec::new();
        let mut events = Vec::new();
        if c.fire && p.gun_cd <= 0.0 && p.energy > st.gun.cost {
            p.gun_cd = st.gun.delay;
            p.energy -= st.gun.cost;
            p.guard = 0.0;
            let mut dirs = vec![p.orient];
            if p.scatter > 0.0 {
                dirs.extend([p.orient - 0.04, p.orient + 0.04]);
            }
            for o in dirs {
                let h = zs::orientation_to_heading(o);
                let offsets: &[f32] = if st.gun.barrels == 2 { &[-0.55, 0.55] } else { &[0.0] };
                for &off in offsets {
                    let (px, py) = (-hy * off * r, hx * off * r);
                    new_shots.push(Shot {
                        owner: i as u8,
                        kind: ShotKind::Gun,
                        x: p.x + hx * r + px,
                        y: p.y + hy * r + py,
                        vx: p.vx + h.x * st.gun.speed,
                        vy: p.vy + h.y * st.gun.speed,
                        life: st.gun.life,
                        bounces: st.gun.bounces,
                        damage: st.gun.damage,
                        radius: 0.0,
                    });
                }
            }
            events.push(Event::Fire { who: i as u8, x: p.x, y: p.y });
        }
        if c.bomb && p.bomb_cd <= 0.0 && p.energy > st.bomb.cost {
            p.bomb_cd = st.bomb.delay;
            p.energy -= st.bomb.cost;
            p.guard = 0.0;
            new_shots.push(Shot {
                owner: i as u8,
                kind: ShotKind::Bomb,
                x: p.x + hx * r,
                y: p.y + hy * r,
                vx: p.vx + hx * st.bomb.speed,
                vy: p.vy + hy * st.bomb.speed,
                life: BOMB_LIFE,
                bounces: st.bomb.bounces,
                damage: st.bomb.damage,
                radius: st.bomb.radius,
            });
            events.push(Event::Bomb { who: i as u8, x: p.x, y: p.y });
        }
        let own_mines = self.s.mines.iter().filter(|m| m.owner as usize == i).count();
        let p = self.s.players[i].as_mut().unwrap();
        if c.mine && own_mines < st.mines as usize && p.energy > st.mine_cost {
            p.energy -= st.mine_cost;
            p.guard = 0.0;
            self.s.mines.push(Mine { owner: i as u8, x: p.x, y: p.y, age: 0.0, damage: st.mine_damage });
            events.push(Event::MineSet { who: i as u8 });
        }
        self.s.shots.extend(new_shots);
        self.events.extend(events);
    }

    fn step_shots(&mut self, dt: f32) {
        let mut shots = std::mem::take(&mut self.s.shots);
        let mut blasts = Vec::new();
        let mut hits = Vec::new();
        shots.retain_mut(|s| {
            s.life -= dt;
            let arena = &self.arena;
            let contact = zs::step_terrain(&mut s.x, &mut s.y, &mut s.vx, &mut s.vy, dt, s.bounces > 0, |x, y| arena.solid_at(x, y));
            if contact.any() {
                if s.bounces == 0 {
                    if s.kind == ShotKind::Bomb {
                        blasts.push((s.x, s.y, s.radius, s.damage, s.owner));
                    }
                    return false;
                }
                s.bounces -= 1;
            }
            // ships in the way
            for (j, p) in self.s.players.iter().enumerate() {
                let Some(p) = p else { continue };
                if !p.alive || !self.foes(s.owner as usize, j) {
                    continue;
                }
                if (p.x - s.x).hypot(p.y - s.y) <= p.radius() + 2.0 {
                    match s.kind {
                        ShotKind::Gun => hits.push((j, s.damage, s.owner, s.x, s.y)),
                        ShotKind::Bomb => blasts.push((s.x, s.y, s.radius, s.damage, s.owner)),
                    }
                    return false;
                }
            }
            if s.life <= 0.0 {
                if s.kind == ShotKind::Bomb {
                    blasts.push((s.x, s.y, s.radius, s.damage, s.owner));
                }
                return false;
            }
            true
        });
        self.s.shots = shots;
        for (j, dmg, by, x, y) in hits {
            self.events.push(Event::Hit { who: j as u8, x, y });
            self.damage(j, dmg, by);
        }
        for (x, y, r, dmg, by) in blasts {
            self.blast(x, y, r, dmg, by);
        }
    }

    fn step_mines(&mut self, dt: f32) {
        let mut mines = std::mem::take(&mut self.s.mines);
        let mut blasts = Vec::new();
        mines.retain_mut(|m| {
            m.age += dt;
            if m.age > MINE_LIFE {
                return false;
            }
            if m.age < MINE_ARM {
                return true;
            }
            let near = self.s.players.iter().enumerate().any(|(j, p)| p.as_ref().is_some_and(|p| p.alive && self.foes(m.owner as usize, j) && (p.x - m.x).hypot(p.y - m.y) <= MINE_REACH + p.radius()));
            if near {
                blasts.push((m.x, m.y, MINE_BLAST, m.damage, m.owner));
                return false;
            }
            true
        });
        self.s.mines = mines;
        for (x, y, r, dmg, by) in blasts {
            self.blast(x, y, r, dmg, by);
        }
    }

    fn step_spots(&mut self, dt: f32) {
        for k in 0..self.s.spots.len() {
            let spot = self.s.spots[k];
            let (x, y) = Arena::centre((spot.tile.0 as usize, spot.tile.1 as usize));
            match spot.ready {
                None => {
                    let t = spot.timer - dt;
                    if t <= 0.0 {
                        let kind = PICKUPS[((self.rand() * 4.0) as usize).min(3)];
                        self.s.spots[k].ready = Some(kind);
                    } else {
                        self.s.spots[k].timer = t;
                    }
                }
                Some(kind) => {
                    let taker = self.s.players.iter().position(|p| p.as_ref().is_some_and(|p| p.alive && (p.x - x).hypot(p.y - y) <= p.radius() + PICKUP_REACH));
                    if let Some(j) = taker {
                        let p = self.s.players[j].as_mut().unwrap();
                        match kind {
                            PickupKind::Overcharge => {
                                p.max_energy = p.kind.stats().energy * 1.35;
                                p.energy = p.max_energy;
                            }
                            PickupKind::Scatter => p.scatter = EFFECT_FOR,
                            PickupKind::Shroud => p.shroud = EFFECT_FOR,
                            PickupKind::Overdrive => p.overdrive = EFFECT_FOR,
                        }
                        self.s.spots[k] = Spot { tile: spot.tile, ready: None, timer: PICKUP_EVERY };
                        self.events.push(Event::Pickup { who: j as u8, kind });
                    }
                }
            }
        }
    }

    fn blast(&mut self, x: f32, y: f32, r: f32, dmg: f32, by: u8) {
        self.events.push(Event::Blast { x, y, r });
        let mut hurt = Vec::new();
        for (j, p) in self.s.players.iter().enumerate() {
            let Some(p) = p else { continue };
            if !p.alive || !self.foes(by as usize, j) {
                continue;
            }
            let reach = r + p.radius();
            let d = (p.x - x).hypot(p.y - y);
            if d <= reach {
                hurt.push((j, dmg * (1.0 - 0.7 * d / reach)));
            }
        }
        for (j, d) in hurt {
            self.damage(j, d, by);
        }
    }

    fn damage(&mut self, j: usize, amount: f32, by: u8) {
        let over = self.s.over.is_some();
        let Some(p) = self.s.players[j].as_mut() else { return };
        if !p.alive || p.guard > 0.0 {
            return;
        }
        p.energy -= amount;
        p.last_hit = Some(by);
        p.flash = 0.12;
        if p.energy > 0.0 {
            return;
        }
        p.alive = false;
        p.energy = 0.0;
        p.respawn = RESPAWN;
        if !over {
            p.deaths += 1;
        }
        let ended = std::mem::take(&mut p.streak);
        let (x, y) = (p.x, p.y);
        let killer = (by as usize != j).then_some(by);
        let mut streak = None;
        if let Some(k) = killer.and_then(|k| self.s.players.get_mut(k as usize)).and_then(Option::as_mut)
            && !over
        {
            k.kills += 1;
            k.streak += 1;
            if matches!(k.streak, 3 | 5 | 8) || (k.streak > 8 && k.streak % 5 == 0) {
                streak = Some(k.streak);
            }
        }
        self.events.push(Event::Died { who: j as u8, by: killer, x, y });
        if ended >= 3 && !over {
            self.events.push(Event::StreakEnded { who: j as u8, by: killer, n: ended });
        }
        if let (Some(n), Some(k)) = (streak, killer) {
            self.events.push(Event::Streak { who: k, n });
        }
    }

    /// A guest's prediction: fly its own ship one step, movement only
    /// (weapons, damage and pickups stay the host's business).
    pub fn fly(&mut self, slot: usize, c: Controls, dt: f32) {
        if let Some(p) = self.s.players.get_mut(slot).and_then(Option::as_mut).filter(|p| p.alive) {
            move_ship(&self.arena, p, c, dt);
        }
    }

    /// A guest between frames: let everyone else and the shots coast.
    pub fn coast(&mut self, except: usize, dt: f32) {
        for (i, p) in self.s.players.iter_mut().enumerate() {
            if let Some(p) = p.as_mut().filter(|p| p.alive && i != except) {
                let (x, y) = (p.x + p.vx * dt, p.y + p.vy * dt);
                if !self.arena.solid_at(x, y) {
                    (p.x, p.y) = (x, y);
                }
            }
        }
        for s in self.s.shots.iter_mut() {
            s.x += s.vx * dt;
            s.y += s.vy * dt;
        }
        self.s.time += dt;
    }

    /// Players ranked for the scoreboard: kills, then fewer deaths.
    pub fn ranking(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..MAX_PLAYERS).filter(|&i| self.s.players[i].is_some()).collect();
        v.sort_by_key(|&i| {
            let p = self.s.players[i].as_ref().unwrap();
            (std::cmp::Reverse(p.kills), p.deaths, i)
        });
        v
    }
}

/// One step of a ship's flight: thrust, turn, energy, walls. True when it
/// bounced hard enough to hear.
fn move_ship(arena: &Arena, p: &mut Player, c: Controls, dt: f32) -> bool {
    let st = p.kind.stats();
    let input = PlayerMovementInput { thrust: c.thrust, reverse: c.reverse, rotate_left: c.left, rotate_right: c.right, afterburner_held: c.burn };
    let state = PlayerMovementState { orientation: p.orient, velocity: MovementVector::new(p.vx, p.vy), energy: p.energy, max_energy: p.max_energy };
    let out = zs::step_player_movement(state, input, p.kind.tuning(p.overdrive > 0.0), dt);
    let r = st.radius;
    let col = zs::integrate_player_collision(MovementVector::new(p.x, p.y), out.state.velocity, dt, st.bounce, |pos, axis, d| {
        zs::movement_axis_overlaps_blocked_tile(pos, r, arena.w, arena.h, TILE, axis, d, |tx, ty| arena.wall(tx, ty))
    });
    let vel = zs::truncate_movement_vector(col.velocity, out.top_speed);
    (p.x, p.y, p.vx, p.vy) = (col.position.x, col.position.y, vel.x, vel.y);
    p.orient = out.state.orientation;
    p.energy = out.state.energy;
    p.thrusting = out.applied_thrust != zs::AppliedThrust::None;
    p.burning = out.afterburning;
    for t in [&mut p.gun_cd, &mut p.bomb_cd, &mut p.guard, &mut p.scatter, &mut p.shroud, &mut p.overdrive, &mut p.flash] {
        *t = (*t - dt).max(0.0);
    }
    col.bounced && vel.length() > 60.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ships;

    fn world_with(n: usize) -> World {
        let mut w = World::new("foundry", 7);
        for k in 0..n {
            w.add_player(format!("p{k}"), true, ships::ALL[k % 4]).unwrap();
        }
        w
    }

    fn run(w: &mut World, secs: f32, c: &[Controls]) {
        for _ in 0..(secs * 60.0) as usize {
            w.step(1.0 / 60.0, c);
        }
    }

    #[test]
    fn players_spawn_and_fly() {
        let mut w = world_with(4);
        run(&mut w, 1.0, &[]);
        assert!(w.s.players.iter().flatten().all(|p| p.alive));
        let thrust = Controls { thrust: true, left: true, ..Default::default() };
        let before = w.player(0).map(|p| (p.x, p.y)).unwrap();
        run(&mut w, 5.0, &[thrust]);
        let p = w.player(0).unwrap();
        assert!((p.x, p.y) != before);
        // still inside open space
        assert!(!w.arena.solid_at(p.x, p.y));
    }

    #[test]
    fn ships_never_end_up_in_walls() {
        let mut w = world_with(8);
        let c: Vec<Controls> = (0..8).map(|k| Controls { thrust: true, burn: k % 2 == 0, left: k % 3 == 0, fire: true, ..Default::default() }).collect();
        for _ in 0..60 * 30 {
            w.step(1.0 / 60.0, &c);
            for p in w.s.players.iter().flatten().filter(|p| p.alive) {
                assert!(!w.arena.solid_at(p.x, p.y), "{} inside a wall at {},{}", p.name, p.x, p.y);
            }
        }
    }

    #[test]
    fn gun_hits_and_kills() {
        let mut w = world_with(2);
        run(&mut w, 1.0, &[]);
        // put the two face to face in open space
        let (x, y) = Arena::centre((40, 36));
        {
            let a = w.s.players[0].as_mut().unwrap();
            (a.x, a.y, a.vx, a.vy, a.orient, a.guard) = (x, y, 0.0, 0.0, 0.5, 0.0);
        }
        {
            let b = w.s.players[1].as_mut().unwrap();
            (b.x, b.y, b.vx, b.vy, b.guard) = (x, y + 120.0, 0.0, 0.0, 0.0);
        }
        assert!(w.arena.clear_line((x, y), (x, y + 120.0)));
        let fire = Controls { fire: true, ..Default::default() };
        let mut died = false;
        for _ in 0..60 * 8 {
            w.step(1.0 / 60.0, &[fire]);
            // keep them parked
            for p in w.s.players.iter_mut().flatten() {
                (p.vx, p.vy) = (0.0, 0.0);
            }
            if w.events.drain(..).any(|e| matches!(e, Event::Died { who: 1, by: Some(0), .. })) {
                died = true;
                break;
            }
        }
        assert!(died);
        assert_eq!(w.player(0).unwrap().kills, 1);
        assert_eq!(w.player(1).unwrap().deaths, 1);
    }

    #[test]
    fn mine_and_bomb_blasts() {
        let mut w = world_with(2);
        run(&mut w, 1.0, &[]);
        let (x, y) = Arena::centre((40, 36));
        w.s.mines.push(Mine { owner: 0, x, y, age: 1.0, damage: 5000.0 });
        {
            let b = w.s.players[1].as_mut().unwrap();
            (b.x, b.y, b.vx, b.vy, b.guard) = (x + 10.0, y, 0.0, 0.0, 0.0);
        }
        w.step(1.0 / 60.0, &[]);
        assert!(!w.player(1).unwrap().alive);
        assert!(w.s.mines.is_empty());
        assert!(w.events.iter().any(|e| matches!(e, Event::Blast { .. })));
    }

    #[test]
    fn pickups_appear_and_are_taken() {
        let mut w = world_with(1);
        run(&mut w, 3.0, &[]);
        assert!(w.s.spots.iter().all(|s| s.ready.is_some()));
        let (x, y) = Arena::centre((w.s.spots[0].tile.0 as usize, w.s.spots[0].tile.1 as usize));
        {
            let p = w.s.players[0].as_mut().unwrap();
            (p.x, p.y, p.vx, p.vy) = (x, y, 0.0, 0.0);
        }
        w.step(1.0 / 60.0, &[]);
        assert!(w.s.spots[0].ready.is_none());
        assert!(w.events.iter().any(|e| matches!(e, Event::Pickup { who: 0, .. })));
    }

    #[test]
    fn ship_change_warps() {
        let mut w = world_with(1);
        run(&mut w, 1.0, &[]);
        w.step(1.0 / 60.0, &[Controls { ship: Some(Kind::Anvil), ..Default::default() }]);
        let p = w.player(0).unwrap();
        assert!(!p.alive && p.kind == Kind::Anvil && p.deaths == 0);
        run(&mut w, 2.0, &[]);
        assert!(w.player(0).unwrap().alive);
    }

    /// The guest's prediction: from the host's state at an acknowledged
    /// input, replaying the later inputs lands exactly where the host is.
    #[test]
    fn replayed_inputs_match_the_host() {
        let mut host = world_with(1);
        run(&mut host, 1.0, &[]);
        let pattern = |k: usize| Controls { thrust: k % 90 < 50, left: k % 140 < 30, right: (k + 70) % 140 < 25, burn: k % 200 < 20, ..Default::default() };
        let mut snapshot = None;
        for k in 0..600 {
            if k == 400 {
                snapshot = Some(host.s.clone());
            }
            host.step(1.0 / 60.0, &[pattern(k)]);
        }
        let mut guest = world_with(1);
        guest.s = snapshot.unwrap();
        for k in 400..600 {
            guest.fly(0, pattern(k), 1.0 / 60.0);
        }
        let (h, g) = (host.player(0).unwrap(), guest.player(0).unwrap());
        assert_eq!((h.x, h.y, h.vx, h.vy, h.orient), (g.x, g.y, g.vx, g.vy, g.orient));
    }

    #[test]
    fn teams_split_and_spare_friends() {
        let mut w = World::new("foundry", 9);
        w.set_teams(true);
        for k in 0..4 {
            w.add_player(format!("p{k}"), true, ships::ALL[k]);
        }
        let teams: Vec<u8> = w.s.players.iter().flatten().map(|p| p.team).collect();
        assert_eq!(teams, vec![0, 1, 0, 1]);
        assert!(!w.foes(0, 2) && w.foes(0, 1) && !w.foes(1, 1));
        run(&mut w, 1.0, &[]);
        // a mine of player 0 ignores teammate 2, takes out foe 1
        let (x, y) = Arena::centre((40, 36));
        w.s.mines.push(Mine { owner: 0, x, y, age: 1.0, damage: 5000.0 });
        for (slot, dx) in [(2usize, 0.0f32), (1, 400.0)] {
            let p = w.s.players[slot].as_mut().unwrap();
            (p.x, p.y, p.vx, p.vy, p.guard) = (x + dx, y, 0.0, 0.0, 0.0);
        }
        w.step(1.0 / 60.0, &[]);
        assert!(w.player(2).unwrap().alive && w.s.mines.len() == 1);
        let p = w.s.players[1].as_mut().unwrap();
        (p.x, p.y) = (x + 10.0, y);
        w.step(1.0 / 60.0, &[]);
        // the blast spares the teammate standing on it
        assert!(!w.player(1).unwrap().alive && w.player(2).unwrap().alive);
        assert_eq!(w.team_kills(0), 1);
    }

    #[test]
    fn streaks_are_announced_and_ended() {
        let mut w = world_with(3);
        run(&mut w, 1.0, &[]);
        let mut events = Vec::new();
        for _ in 0..3 {
            w.s.players[1].as_mut().unwrap().guard = 0.0;
            w.damage(1, 99999.0, 0);
            events.extend(w.events.drain(..));
            w.s.players[1].as_mut().unwrap().alive = true;
        }
        assert!(events.contains(&Event::Streak { who: 0, n: 3 }));
        w.s.players[0].as_mut().unwrap().guard = 0.0;
        w.damage(0, 99999.0, 2);
        assert!(w.events.contains(&Event::StreakEnded { who: 0, by: Some(2), n: 3 }));
        assert_eq!(w.player(0).unwrap().streak, 0);
    }

    #[test]
    fn match_ends_and_restarts() {
        let mut w = world_with(2);
        run(&mut w, 1.0, &[]);
        w.s.players[0].as_mut().unwrap().kills = KILL_LIMIT;
        w.step(1.0 / 60.0, &[]);
        assert!(matches!(w.s.over, Some((0, _))));
        run(&mut w, NEXT_MATCH_AFTER + 1.0, &[]);
        assert!(w.s.over.is_none());
        assert_eq!(w.player(0).unwrap().kills, 0);
    }
}
