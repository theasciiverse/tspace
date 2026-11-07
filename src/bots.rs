//! Computer pilots. Each bot looks at the world as a player would, then
//! produces the same `Controls` a person's keyboard does.
//!
//! - In a fight: face the target's predicted position (lead for the shot's
//!   speed and both ships' motion), fire when lined up and energy allows,
//!   bomb from closer in, keep a preferred distance.
//! - Otherwise: fly to a pickup, the last place an enemy was seen, or a
//!   random spot, following a breadth-first path field over the tiles.
//! - Nearly out of energy in a fight (medium and hard): turn tail at full
//!   speed, now and then dropping a mine behind.
//! - Long trips with a full tank: afterburner.
//! - A foe's shot about to hit (within half a second): slip forward or
//!   back along the heading, off its line, still aiming (hard always,
//!   medium on half its looks, easy never).
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
    /// Dodging shots this look (hard: always, medium: some looks).
    dodges: bool,
}

impl Brain {
    pub fn new(level: Level, seed: u32) -> Brain {
        Brain { level, rng: seed | 1, look: 0.0, target: None, aim_err: 0.0, goal: None, field: Vec::new(), field_age: 99.0, last_seen: None, was_alive: false, dodges: false }
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
            self.dodges = match self.level {
                Level::Easy => false,
                Level::Medium => self.rand() < 0.5,
                Level::Hard => true,
            };
        }
        if let Some(t) = self.target.and_then(|t| w.player(t)).filter(|t| t.alive) {
            self.last_seen = Some((t.x, t.y));
        } else {
            self.target = None;
        }

        let st = p.kind.stats();
        let mut c = Controls::default();
        let mut aim: Option<f32> = None;
        // nearly out of energy and under fire: break off and run
        let fleeing = self.target.is_some() && p.energy < p.max_energy * 0.22 && self.level != Level::Easy;
        if fleeing {
            let t = w.player(self.target.unwrap()).unwrap();
            let (dx, dy) = (p.x - t.x, p.y - t.y);
            let d = dx.hypot(dy).max(1.0);
            let cruise = st.top_speed;
            let (dvx, dvy) = (dx / d * cruise - p.vx, dy / d * cruise - p.vy);
            let want = turns(dvx, dvy);
            aim = Some(want);
            c.thrust = wrap(want - p.orient).abs() < 0.12;
            // drop a mine behind on the way out, now and then
            c.mine = d < 160.0 && p.energy > st.mine_cost * 1.05 && self.rand() < 0.03;
            self.goal = None;
        } else if let Some(t) = self.target.and_then(|t| w.player(t)) {
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
                    // a long way to go and energy to spare: burn
                    let far = self.goal.is_some_and(|g| {
                        let (gx, gy) = Arena::centre(g);
                        (gx - p.x).hypot(gy - p.y) > 600.0
                    });
                    c.burn = c.thrust && far && p.energy > p.max_energy * 0.9;
                }
            }
        }
        // a shot about to hit us: slip forward or back, off its line
        if self.dodges && let Some(push) = dodge(w, me, p) {
            c.thrust = push > 0.0;
            c.reverse = push < 0.0;
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

/// The most urgent foe's shot that will pass within reach in the next
/// half second, and which way along our heading gets us clear: +1 forward,
/// -1 back. None when nothing is coming, or when the shot comes along our
/// own line (moving along it would not help).
fn dodge(w: &World, me: usize, p: &Player) -> Option<f32> {
    let (hx, hy) = p.heading();
    let mut best: Option<(f32, f32)> = None;
    for s in &w.s.shots {
        if !w.foes(s.owner as usize, me) {
            continue;
        }
        // relative motion of the shot
        let (rx, ry) = (s.x - p.x, s.y - p.y);
        let (vx, vy) = (s.vx - p.vx, s.vy - p.vy);
        let v2 = vx * vx + vy * vy;
        if v2 < 1.0 {
            continue;
        }
        let t = -(rx * vx + ry * vy) / v2;
        if !(0.0..0.5).contains(&t) {
            continue;
        }
        let (cx, cy) = (rx + vx * t, ry + vy * t);
        if cx.hypot(cy) > p.radius() + 10.0 {
            continue;
        }
        // away from the shot's path: the closest point, or sideways if dead on
        let v = v2.sqrt();
        let (mut ax, mut ay) = (-cx, -cy);
        if ax.hypot(ay) < 1.0 {
            (ax, ay) = (-vy / v, vx / v);
        }
        let along = ax * hx + ay * hy;
        let side = (hx * vy - hy * vx).abs() / v;
        if side < 0.3 {
            continue;
        }
        if best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, along.signum()));
        }
    }
    best.map(|b| b.1)
}

fn pick_target(w: &World, me: usize, p: &Player) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for (j, t) in w.s.players.iter().enumerate() {
        let Some(t) = t else { continue };
        if !w.foes(me, j) || !t.alive {
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

    #[test]
    fn dodges_a_shot_from_the_side() {
        use crate::world::{Shot, ShotKind};
        let mut w = World::new("foundry", 1);
        w.add_player("a".into(), true, Kind::Lance);
        w.add_player("b".into(), true, Kind::Lance);
        for _ in 0..60 {
            w.step(1.0 / 60.0, &[]);
        }
        let (x, y) = Arena::centre((40, 36));
        {
            let p = w.s.players[0].as_mut().unwrap();
            (p.x, p.y, p.vx, p.vy, p.orient) = (x, y, 0.0, 0.0, 0.0); // facing up
        }
        // a shot from the left, aimed a little ahead of us (above)
        w.s.shots.push(Shot { owner: 1, kind: ShotKind::Gun, x: x - 200.0, y: y - 4.0, vx: 800.0, vy: 0.0, life: 1.0, bounces: 0, damage: 50.0, radius: 0.0 });
        let p = w.player(0).unwrap();
        // it passes just above our centre, so back away (down)
        assert_eq!(dodge(&w, 0, p), Some(-1.0));
        // nothing to dodge once the shot flies away from us
        w.s.shots[0].vx = -800.0;
        assert_eq!(dodge(&w, 0, w.player(0).unwrap()), None);
    }

    /// A quick look that bots fight: four of them for a simulated two
    /// minutes should shoot each other down a few times and take pickups.
    #[test]
    fn bots_fight() {
        let mut w = World::new("foundry", 3);
        let mut brains: Vec<Brain> = (0..4).map(|k| Brain::new(Level::Hard, 11 + k)).collect();
        for (k, b) in brains.iter_mut().enumerate() {
            let ship = b.pick_ship();
            w.add_player(NAMES[k].into(), true, ship);
        }
        let (mut pickups, mut pulses) = (0, 0);
        for _ in 0..60 * 120 {
            let c: Vec<Controls> = brains.iter_mut().enumerate().map(|(i, b)| b.think(&w, i, 1.0 / 60.0)).collect();
            w.step(1.0 / 60.0, &c);
            for e in w.events.drain(..) {
                match e {
                    crate::world::Event::Pickup { .. } => pickups += 1,
                    crate::world::Event::Pulse { .. } => pulses += 1,
                    _ => {}
                }
            }
        }
        let kills: u32 = w.s.players.iter().flatten().map(|p| p.kills).sum();
        assert!(kills >= 3, "only {kills} kills in two minutes");
        // they go for pickups too, and a Repulsor goes off now and then
        assert!(pickups >= 5, "only {pickups} pickups taken");
        println!("kills {kills} pickups {pickups} repulsor pulses {pulses}");
    }
}

#[cfg(test)]
mod soak {
    use super::*;

    /// Every arena, both modes, eight hard bots for three simulated
    /// minutes each: no ship ever inside a wall or off the map, every
    /// position finite, matches can end and restart.
    #[test]
    fn bots_play_every_arena_and_mode() {
        for (n, (arena, _)) in crate::arena::BUILTIN.iter().enumerate() {
            for teams in [false, true] {
                let mut w = World::new(arena, 40 + n as u64);
                w.set_teams(teams);
                w.s.kill_limit = 6; // a few match ends along the way
                let mut brains: Vec<Brain> = (0..8).map(|k| Brain::new(Level::Hard, 900 + k + n as u32 * 17)).collect();
                for (k, b) in brains.iter_mut().enumerate() {
                    let ship = b.pick_ship();
                    w.add_player(NAMES[k].into(), true, ship);
                }
                let mut matches = 0;
                for step in 0..60 * 180 {
                    let c: Vec<Controls> = brains.iter_mut().enumerate().map(|(i, b)| b.think(&w, i, 1.0 / 60.0)).collect();
                    w.step(1.0 / 60.0, &c);
                    for e in w.events.drain(..) {
                        if matches!(e, crate::world::Event::Won { .. } | crate::world::Event::TeamWon { .. }) {
                            matches += 1;
                        }
                    }
                    for p in w.s.players.iter().flatten().filter(|p| p.alive) {
                        assert!(p.x.is_finite() && p.y.is_finite() && p.vx.is_finite() && p.vy.is_finite(), "{arena} step {step}: {} not finite", p.name);
                        assert!(!w.arena.solid_at(p.x, p.y), "{arena} teams {teams} step {step}: {} inside a wall at {:.0},{:.0}", p.name, p.x, p.y);
                    }
                }
                assert!(matches >= 1, "{arena} teams {teams}: no match ended in three minutes");
            }
        }
    }
}

#[cfg(test)]
mod balance {
    use super::*;

    /// A rough look at the ships, not a measurement: eight hard bots, two
    /// of each ship, fixed ships, several five-minute matches; kills and
    /// deaths per ship. `cargo test --release balance -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn balance() {
        for arena in ["foundry", "lattice", "hollow"] {
            let mut kills = [0u32; 4];
            let mut deaths = [0u32; 4];
            let players = match arena { "hollow" => 4, "lattice" => 6, _ => 8 };
            for seed in 0..4u32 {
                let mut w = World::new(arena, 100 + seed as u64);
                let mut brains: Vec<Brain> = (0..players).map(|k| Brain::new(Level::Hard, 7 + seed * 31 + k as u32)).collect();
                for (k, name) in NAMES.iter().take(players).enumerate() {
                    w.add_player((*name).into(), true, ships::ALL[k % 4]);
                }
                w.s.kill_limit = u32::MAX;
                for _ in 0..60 * 300 {
                    let c: Vec<Controls> = brains.iter_mut().enumerate().map(|(i, b)| Controls { ship: None, ..b.think(&w, i, 1.0 / 60.0) }).collect();
                    w.step(1.0 / 60.0, &c);
                    w.events.clear();
                }
                for (k, p) in w.s.players.iter().flatten().enumerate() {
                    kills[k % 4] += p.kills;
                    deaths[k % 4] += p.deaths;
                }
            }
            println!("{arena}:");
            for k in 0..4 {
                println!("  {:<7} kills {:>4} deaths {:>4}  k/d {:.2}", ships::ALL[k].stats().name, kills[k], deaths[k], kills[k] as f32 / deaths[k].max(1) as f32);
            }
        }
    }
}
