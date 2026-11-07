//! Computer pilots. Each bot looks at the world as a player would, then
//! produces the same `Controls` a person's keyboard does.
//!
//! - In a fight: face the target's predicted position (lead for the shot's
//!   speed and both ships' motion), fire when lined up and energy allows,
//!   bomb from closer in, keep a preferred distance.
//! - Otherwise: fly to a pickup, the last place an enemy was seen, or a
//!   random spot, following a breadth-first path field over the tiles.
//! - Always: brake before flying into a wall.
//!
//! Difficulty changes only reaction time, aim error and how much energy a
//! bot dares to spend, never ship stats.

use crate::arena::{Arena, TILE};
use crate::ships::{self, Kind};
use crate::world::{Controls, Player, World};
use std::collections::VecDeque;
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Level {
    Easy,
    #[default]
    Medium,
    Hard,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Easy, Level::Medium, Level::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Level::Easy => "easy",
            Level::Medium => "medium",
            Level::Hard => "hard",
        }
    }

    /// (seconds between looks, aim error in turns, energy kept in reserve)
    fn skill(self) -> (f32, f32, f32) {
        match self {
            Level::Easy => (0.55, 0.035, 0.55),
            Level::Medium => (0.25, 0.014, 0.4),
            Level::Hard => (0.08, 0.004, 0.3),
        }
    }
}

pub const NAMES: [&str; 12] = ["Vega", "Orrin", "Tamsin", "Kestrel", "Juno", "Brask", "Ilse", "Moth", "Quill", "Rook", "Sable", "Wren"];

pub struct Brain {
    pub level: Level,
    rng: u32,
    look: f32,
    target: Option<usize>,
    aim_err: f32,
    goal: Option<(usize, usize)>,
    field: Vec<u16>,
    field_age: f32,
    last_seen: Option<(f32, f32)>,
    was_alive: bool,
}

impl Brain {
    pub fn new(level: Level, seed: u32) -> Brain {
        Brain { level, rng: seed | 1, look: 0.0, target: None, aim_err: 0.0, goal: None, field: Vec::new(), field_age: 99.0, last_seen: None, was_alive: false }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32
    }

    /// A ship for a new bot or a respawn.
    pub fn pick_ship(&mut self) -> Kind {
        ships::ALL[((self.rand() * 4.0) as usize).min(3)]
    }

    pub fn think(&mut self, w: &World, me: usize, dt: f32) -> Controls {
        let Some(p) = w.player(me) else { return Controls::default() };
        if !p.alive {
            let mut c = Controls::default();
            if self.was_alive && self.rand() < 0.25 {
                c.ship = Some(self.pick_ship());
            }
            self.was_alive = false;
            self.target = None;
            self.goal = None;
            return c;
        }
        self.was_alive = true;
        let (react, err, reserve) = self.level.skill();
        self.look -= dt;
        self.field_age += dt;
        if self.look <= 0.0 {
            self.look = react * (0.7 + 0.6 * self.rand());
            self.target = pick_target(w, me, p);
            self.aim_err = (self.rand() * 2.0 - 1.0) * err;
        }
        if let Some(t) = self.target.and_then(|t| w.player(t)).filter(|t| t.alive) {
            self.last_seen = Some((t.x, t.y));
        } else {
            self.target = None;
        }

        let st = p.kind.stats();
        let mut c = Controls::default();
        let mut aim: Option<f32> = None;
        if let Some(t) = self.target.and_then(|t| w.player(t)) {
            let (dx, dy) = (t.x - p.x, t.y - p.y);
            let dist = dx.hypot(dy);
            let (lx, ly) = lead((dx, dy), (t.vx - p.vx, t.vy - p.vy), st.gun.speed);
            let want = turns(lx, ly) + self.aim_err;
            let diff = wrap(want - p.orient);
            aim = Some(want);
            let spare = p.energy > p.max_energy * reserve;
            let range = st.gun.speed * st.gun.life * 0.85;
            c.fire = diff.abs() < 0.02 && dist < range && spare;
            c.bomb = diff.abs() < 0.03 && dist < 360.0 && p.energy > p.max_energy * 0.75 && p.bomb_cd <= 0.0;
            c.mine = dist < 110.0 && p.energy > p.max_energy * 0.6 && self.rand() < if p.kind == Kind::Weaver { 0.05 } else { 0.01 };
            // keep a preferred distance, moving along the line we face
            let keep = match p.kind {
                Kind::Dart => 170.0,
                Kind::Lance => 250.0,
                Kind::Anvil => 290.0,
                Kind::Weaver => 230.0,
            };
            let closing = (p.vx * dx + p.vy * dy) / dist.max(1.0);
            if diff.abs() < 0.15 {
                c.thrust = dist > keep + 50.0 && closing < 220.0;
                c.reverse = dist < keep - 50.0 && closing > -160.0;
            }
            self.goal = None;
        } else {
            // travel
            if self.goal.is_none() || self.field_age > 3.0 || self.at_goal(p) {
                self.choose_goal(w, p);
            }
            if let Some(dir) = self.waypoint(w, p) {
                let cruise = st.top_speed * 0.7;
                let (dvx, dvy) = (dir.0 * cruise - p.vx, dir.1 * cruise - p.vy);
                if dvx.hypot(dvy) > 25.0 {
                    let want = turns(dvx, dvy);
                    aim = Some(want);
                    c.thrust = wrap(want - p.orient).abs() < 0.07;
                }
            }
        }
        // about to hit a wall: brake, unless the bounce is gentle
        let (px, py) = (p.x + p.vx * 0.4, p.y + p.vy * 0.4);
        if p.speed() > 140.0 && !w.arena.clear_line((p.x, p.y), (px, py)) {
            let want = turns(-p.vx, -p.vy);
            let lined = c.fire;
            aim = Some(want);
            c.thrust = wrap(want - p.orient).abs() < 0.1;
            c.reverse = false;
            c.fire = lined;
        }
        if let Some(want) = aim {
            let diff = wrap(want - p.orient);
            let step = st.rotation * dt;
            c.left = diff < -step * 0.5;
            c.right = diff > step * 0.5;
        }
        c
    }

    fn at_goal(&self, p: &Player) -> bool {
        self.goal.is_some_and(|g| {
            let (gx, gy) = Arena::centre(g);
            (gx - p.x).hypot(gy - p.y) < 40.0
        })
    }

    fn choose_goal(&mut self, w: &World, p: &Player) {
        let a = &w.arena;
        let mut goal = None;
        if let Some((x, y)) = self.last_seen.take() {
            goal = Some(((x / TILE) as usize, (y / TILE) as usize));
        }
        if goal.is_none() && self.rand() < 0.6 {
            goal = w
                .s
                .spots
                .iter()
                .filter(|s| s.ready.is_some())
                .map(|s| (s.tile.0 as usize, s.tile.1 as usize))
                .min_by_key(|&t| {
                    let (x, y) = Arena::centre(t);
                    (x - p.x).hypot(y - p.y) as i32
                });
        }
        while goal.is_none() {
            let t = ((self.rand() * a.w as f32) as usize, (self.rand() * a.h as f32) as usize);
            if !a.wall(t.0 as i32, t.1 as i32) && open_around(a, t) {
                goal = Some(t);
            }
        }
        self.goal = goal;
        self.field = path_field(a, goal.unwrap());
        self.field_age = 0.0;
    }

    /// Direction to the farthest point down the path field still in view.
    fn waypoint(&self, w: &World, p: &Player) -> Option<(f32, f32)> {
        let a = &w.arena;
        if self.field.len() != a.w * a.h {
            return None;
        }
        let mut t = ((p.x / TILE) as usize, (p.y / TILE) as usize);
        let mut best = None;
        for _ in 0..10 {
            let here = self.field[t.1 * a.w + t.0];
            if here == 0 || here == u16::MAX {
                break;
            }
            let next = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)]
                .iter()
                .map(|&(dx, dy)| (t.0 as i32 + dx, t.1 as i32 + dy))
                .filter(|&(x, y)| !a.wall(x, y))
                .min_by_key(|&(x, y)| self.field[y as usize * a.w + x as usize])?;
            t = (next.0 as usize, next.1 as usize);
            let c = Arena::centre(t);
            if wide_clear(a, (p.x, p.y), c, p.radius()) {
                best = Some(c);
            } else {
                break;
            }
        }
        let (bx, by) = best.unwrap_or_else(|| Arena::centre(t));
        let (dx, dy) = (bx - p.x, by - p.y);
        let d = dx.hypot(dy);
        (d > 1.0).then(|| (dx / d, dy / d))
    }
}

fn pick_target(w: &World, me: usize, p: &Player) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for (j, t) in w.s.players.iter().enumerate() {
        let Some(t) = t else { continue };
        if j == me || !t.alive {
            continue;
        }
        let d = (t.x - p.x).hypot(t.y - p.y);
        let seen = if t.shroud > 0.0 { 150.0 } else { 720.0 };
        if d < seen && d < best.map_or(f32::MAX, |b| b.0) && w.arena.clear_line((p.x, p.y), (t.x, t.y)) {
            best = Some((d, j));
        }
    }
    best.map(|b| b.1)
}

/// Where to aim so a shot at `speed` meets a target at `d` moving at `v`
/// (relative velocity). Falls back to straight at it.
pub fn lead(d: (f32, f32), v: (f32, f32), speed: f32) -> (f32, f32) {
    let a = v.0 * v.0 + v.1 * v.1 - speed * speed;
    let b = 2.0 * (d.0 * v.0 + d.1 * v.1);
    let c = d.0 * d.0 + d.1 * d.1;
    let disc = b * b - 4.0 * a * c;
    if a.abs() < 1e-3 || disc < 0.0 {
        return d;
    }
    let s = disc.sqrt();
    let t = [(-b - s) / (2.0 * a), (-b + s) / (2.0 * a)].into_iter().filter(|t| *t > 0.0).fold(f32::MAX, f32::min);
    if t == f32::MAX || t > 3.0 {
        return d;
    }
    (d.0 + v.0 * t, d.1 + v.1 * t)
}

/// Orientation in turns pointing along (x, y): 0 up, clockwise.
pub fn turns(x: f32, y: f32) -> f32 {
    (x.atan2(-y) / TAU).rem_euclid(1.0)
}

/// Signed shortest turn, in (-0.5, 0.5].
pub fn wrap(d: f32) -> f32 {
    let d = d.rem_euclid(1.0);
    if d > 0.5 { d - 1.0 } else { d }
}

fn open_around(a: &Arena, t: (usize, usize)) -> bool {
    [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(dx, dy)| !a.wall(t.0 as i32 + dx, t.1 as i32 + dy))
}

/// A straight line wide enough for the ship: centre and both sides clear.
fn wide_clear(a: &Arena, from: (f32, f32), to: (f32, f32), r: f32) -> bool {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let d = dx.hypot(dy).max(1.0);
    let (nx, ny) = (-dy / d * r, dx / d * r);
    a.clear_line(from, to) && a.clear_line((from.0 + nx, from.1 + ny), (to.0 + nx, to.1 + ny)) && a.clear_line((from.0 - nx, from.1 - ny), (to.0 - nx, to.1 - ny))
}

/// Steps from every open tile to `goal`, 8-way without cutting corners;
/// tiles next to walls cost extra so paths keep off them.
pub fn path_field(a: &Arena, goal: (usize, usize)) -> Vec<u16> {
    let mut f = vec![u16::MAX; a.w * a.h];
    let mut q = VecDeque::new();
    f[goal.1 * a.w + goal.0] = 0;
    q.push_back(goal);
    while let Some((x, y)) = q.pop_front() {
        let here = f[y * a.w + x];
        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if a.wall(nx, ny) {
                continue;
            }
            let hug = !open_around(a, (nx as usize, ny as usize));
            let cost = if hug { 3 } else { 1 };
            let i = ny as usize * a.w + nx as usize;
            if here.saturating_add(cost) < f[i] {
                f[i] = here + cost;
                q.push_back((nx as usize, ny as usize));
            }
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles() {
        assert!((turns(0.0, -1.0) - 0.0).abs() < 1e-6);
        assert!((turns(1.0, 0.0) - 0.25).abs() < 1e-6);
        assert!((turns(0.0, 1.0) - 0.5).abs() < 1e-6);
        assert!((wrap(0.9) + 0.1).abs() < 1e-6);
        assert!((wrap(-0.7) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn lead_hits_moving_target() {
        let d = (300.0, 0.0);
        let v = (0.0, 100.0);
        let speed = 800.0;
        let (ax, ay) = lead(d, v, speed);
        // fly the shot and the target for the meeting time
        let n = (ax * ax + ay * ay).sqrt();
        let t = n / speed;
        let shot = (ax / n * speed * t, ay / n * speed * t);
        let target = (d.0 + v.0 * t, d.1 + v.1 * t);
        assert!((shot.0 - target.0).abs() < 0.5 && (shot.1 - target.1).abs() < 0.5);
    }

    /// A quick look that bots fight: four of them for a simulated two
    /// minutes should shoot each other down a few times.
    #[test]
    fn bots_fight() {
        let mut w = World::new("foundry", 3);
        let mut brains: Vec<Brain> = (0..4).map(|k| Brain::new(Level::Hard, 11 + k)).collect();
        for (k, b) in brains.iter_mut().enumerate() {
            let ship = b.pick_ship();
            w.add_player(NAMES[k].into(), true, ship);
        }
        for _ in 0..60 * 120 {
            let c: Vec<Controls> = brains.iter_mut().enumerate().map(|(i, b)| b.think(&w, i, 1.0 / 60.0)).collect();
            w.step(1.0 / 60.0, &c);
            w.events.clear();
        }
        let kills: u32 = w.s.players.iter().flatten().map(|p| p.kills).sum();
        assert!(kills >= 3, "only {kills} kills in two minutes");
    }
}
