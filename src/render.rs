//! Drawing the arena in Braille: 2x4 dots per cell, one colour per cell
//! (the colour of the most important thing in it). One 16 px tile is 4x4
//! dots, so 4 px per dot and a tile is two cells wide and one tall.
//!
//! Also the client-side effects (sparks, blast rings, exhaust), made from
//! the world's events, so host and guests show the same thing.

use crate::arena::TILE;
use crate::ships;
use crate::world::{Event, PickupKind, Player, ShotKind, World};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub const PX_PER_DOT: f32 = 4.0;

pub const SLOT_COLOURS: [Color; 8] = [
    Color::Rgb(90, 220, 255),
    Color::Rgb(255, 170, 60),
    Color::Rgb(130, 235, 110),
    Color::Rgb(240, 110, 220),
    Color::Rgb(245, 230, 90),
    Color::Rgb(255, 95, 95),
    Color::Rgb(135, 150, 255),
    Color::Rgb(235, 235, 235),
];

/// Team games: red and blue, a few shades each so teammates differ.
const TEAM_COLOURS: [[Color; 4]; 2] = [
    [Color::Rgb(255, 95, 95), Color::Rgb(255, 150, 110), Color::Rgb(230, 80, 140), Color::Rgb(255, 185, 160)],
    [Color::Rgb(90, 170, 255), Color::Rgb(120, 215, 255), Color::Rgb(120, 130, 255), Color::Rgb(170, 200, 255)],
];

pub fn team_colour(team: u8) -> Color {
    TEAM_COLOURS[team as usize % 2][0]
}

/// A player's colour: by slot, or by team in a team game.
pub fn player_colour(w: &World, i: usize) -> Color {
    if !w.s.teams {
        return SLOT_COLOURS[i % 8];
    }
    let team = w.player(i).map_or(0, |p| p.team);
    let shade = (0..i).filter(|&j| w.player(j).is_some_and(|q| q.team == team)).count();
    TEAM_COLOURS[team as usize % 2][shade % 4]
}

const EDGE: Color = Color::Rgb(110, 145, 200);
const FILL: Color = Color::Rgb(52, 66, 98);
const STAR: Color = Color::Rgb(70, 76, 96);
const STAR_BRIGHT: Color = Color::Rgb(140, 146, 170);

pub fn pickup_colour(k: PickupKind) -> Color {
    match k {
        PickupKind::Overcharge => Color::Rgb(110, 255, 140),
        PickupKind::Scatter => Color::Rgb(255, 150, 60),
        PickupKind::Shroud => Color::Rgb(185, 120, 255),
        PickupKind::Overdrive => Color::Rgb(80, 235, 235),
    }
}

/// Draw priorities: higher wins the cell's colour.
mod prio {
    pub const STAR: u8 = 0;
    pub const WALL: u8 = 1;
    pub const FX: u8 = 3;
    pub const PICKUP: u8 = 4;
    pub const MINE: u8 = 5;
    pub const SHOT: u8 = 6;
    pub const BLAST: u8 = 7;
    pub const SHIP: u8 = 8;
    pub const ME: u8 = 9;
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    cols: u16,
    bits: Vec<u8>,
    colour: Vec<Color>,
    prio: Vec<u8>,
}

impl Canvas {
    pub fn new(cols: u16, rows: u16) -> Canvas {
        let n = cols as usize * rows as usize;
        Canvas { w: cols as i32 * 2, h: rows as i32 * 4, cols, bits: vec![0; n], colour: vec![Color::Reset; n], prio: vec![0; n] }
    }

    pub fn dot(&mut self, x: i32, y: i32, colour: Color, prio: u8) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        const BIT: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
        let cell = (y / 4) as usize * self.cols as usize + (x / 2) as usize;
        self.bits[cell] |= BIT[(x % 2) as usize][(y % 4) as usize];
        if prio >= self.prio[cell] {
            self.prio[cell] = prio;
            self.colour[cell] = colour;
        }
    }

    pub fn line(&mut self, (x0, y0): (f32, f32), (x1, y1): (f32, f32), colour: Color, prio: u8) {
        let n = (x1 - x0).abs().max((y1 - y0).abs()).ceil().max(1.0) as i32;
        for i in 0..=n {
            let f = i as f32 / n as f32;
            self.dot((x0 + (x1 - x0) * f).round() as i32, (y0 + (y1 - y0) * f).round() as i32, colour, prio);
        }
    }

    pub fn blit(&self, buf: &mut Buffer, area: Rect) {
        for row in 0..area.height.min((self.h / 4) as u16) {
            for col in 0..area.width.min(self.cols) {
                let i = row as usize * self.cols as usize + col as usize;
                if self.bits[i] == 0 {
                    continue;
                }
                let ch = char::from_u32(0x2800 + self.bits[i] as u32).unwrap_or(' ');
                if let Some(cell) = buf.cell_mut((area.x + col, area.y + row)) {
                    cell.set_char(ch).set_fg(self.colour[i]);
                }
            }
        }
    }
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    colour: Color,
}

struct Ring {
    x: f32,
    y: f32,
    r: f32,
    t: f32,
}

/// Visual effects, made from events and the ships' state.
#[derive(Default)]
pub struct Fx {
    parts: Vec<Particle>,
    rings: Vec<Ring>,
    seed: u32,
    /// Seconds of screen shake left.
    pub shake: f32,
}

impl Fx {
    fn rand(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn burst(&mut self, x: f32, y: f32, n: usize, speed: f32, life: f32, colour: Color) {
        for _ in 0..n {
            let a = self.rand() * std::f32::consts::TAU;
            let s = speed * (0.3 + 0.7 * self.rand());
            let l = life * (0.5 + 0.5 * self.rand());
            self.parts.push(Particle { x, y, vx: a.cos() * s, vy: a.sin() * s, life: l, colour });
        }
    }

    pub fn event(&mut self, e: &Event, w: &World, me: usize) {
        match *e {
            Event::Hit { x, y, .. } => self.burst(x, y, 5, 120.0, 0.25, Color::Rgb(255, 240, 200)),
            Event::Blast { x, y, r } => {
                self.rings.push(Ring { x, y, r, t: 0.0 });
                self.burst(x, y, 18, r * 4.0, 0.45, Color::Rgb(255, 190, 90));
                if let Some(p) = w.player(me)
                    && (p.x - x).hypot(p.y - y) < 400.0 {
                        self.shake = self.shake.max(0.25);
                    }
            }
            Event::Died { who, x, y, .. } => {
                let c = player_colour(w, who as usize);
                self.burst(x, y, 40, 260.0, 0.9, c);
                self.burst(x, y, 20, 120.0, 0.6, Color::Rgb(255, 255, 255));
                self.rings.push(Ring { x, y, r: 40.0, t: 0.0 });
                if who as usize == me {
                    self.shake = 0.5;
                }
            }
            Event::Pickup { who, kind } => {
                if let Some(p) = w.player(who as usize) {
                    self.burst(p.x, p.y, 14, 140.0, 0.4, pickup_colour(kind));
                }
            }
            Event::Spawn { who } => {
                if let Some(p) = w.player(who as usize) {
                    self.rings.push(Ring { x: p.x, y: p.y, r: 30.0, t: 0.0 });
                }
            }
            _ => {}
        }
    }

    /// Advance effects, and add exhaust for thrusting ships.
    pub fn step(&mut self, dt: f32, w: &World) {
        for p in self.parts.iter_mut() {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vx *= 0.96;
            p.vy *= 0.96;
            p.life -= dt;
        }
        self.parts.retain(|p| p.life > 0.0);
        for r in self.rings.iter_mut() {
            r.t += dt;
        }
        self.rings.retain(|r| r.t < 0.35);
        self.shake = (self.shake - dt).max(0.0);
        let thrusting: Vec<(f32, f32, f32, f32, f32, bool)> = w
            .s
            .players
            .iter()
            .flatten()
            .filter(|p| p.alive && p.thrusting && p.shroud <= 0.0)
            .map(|p| {
                let (hx, hy) = p.heading();
                (p.x, p.y, hx, hy, p.radius(), p.burning)
            })
            .collect();
        for (x, y, hx, hy, r, burn) in thrusting {
            let n = if burn { 3 } else { 1 };
            for _ in 0..n {
                let spread = (self.rand() - 0.5) * 0.8;
                let s = 140.0 + 120.0 * self.rand();
                let (dx, dy) = (-hx + spread * -hy, -hy + spread * hx);
                let colour = if burn { Color::Rgb(255, 120, 60) } else { Color::Rgb(255, 200, 110) };
                let life = 0.12 + 0.1 * self.rand();
                self.parts.push(Particle { x: x - hx * r, y: y - hy * r, vx: dx * s, vy: dy * s, life, colour });
            }
        }
    }
}

/// Where the camera looks: the top-left dot of the view, in world dots.
pub fn camera(centre: (f32, f32), c: &Canvas) -> (i32, i32) {
    ((centre.0 / PX_PER_DOT).round() as i32 - c.w / 2, (centre.1 / PX_PER_DOT).round() as i32 - c.h / 2)
}

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

/// Two layers of stars behind everything, drifting slower than the arena.
fn stars(c: &mut Canvas, cam: (i32, i32)) {
    for (depth, density, colour) in [(3, 31u32, STAR), (2, 47u32, STAR_BRIGHT)] {
        let (ox, oy) = (cam.0 / depth, cam.1 / depth);
        let (cx0, cy0) = (ox.div_euclid(4), oy.div_euclid(4));
        for cy in cy0..=cy0 + c.h / 4 + 1 {
            for cx in cx0..=cx0 + c.w / 4 + 1 {
                let h = hash(cx * depth, cy * 7 + depth);
                if !h.is_multiple_of(density) {
                    continue;
                }
                let (dx, dy) = (cx * 4 + (h >> 8) as i32 % 4, cy * 4 + (h >> 12) as i32 % 4);
                c.dot(dx - ox, dy - oy, colour, prio::STAR);
            }
        }
    }
}

fn walls(c: &mut Canvas, w: &World, cam: (i32, i32)) {
    let a = &w.arena;
    let (tx0, ty0) = (cam.0.div_euclid(4) - 1, cam.1.div_euclid(4) - 1);
    let (tx1, ty1) = (tx0 + c.w / 4 + 2, ty0 + c.h / 4 + 2);
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            if tx < 0 || ty < 0 || tx >= a.w as i32 || ty >= a.h as i32 || !a.wall(tx, ty) {
                continue;
            }
            let open = |dx: i32, dy: i32| {
                let (nx, ny) = (tx + dx, ty + dy);
                nx >= 0 && ny >= 0 && nx < a.w as i32 && ny < a.h as i32 && !a.wall(nx, ny)
            };
            let (l, r, u, d) = (open(-1, 0), open(1, 0), open(0, -1), open(0, 1));
            for dy in 0..4 {
                for dx in 0..4 {
                    let edge = (dx == 0 && l) || (dx == 3 && r) || (dy == 0 && u) || (dy == 3 && d);
                    let (x, y) = (tx * 4 + dx - cam.0, ty * 4 + dy - cam.1);
                    if edge {
                        c.dot(x, y, EDGE, prio::WALL);
                    } else if (tx * 4 + dx + ty * 4 + dy) % 2 == 0 && dy % 2 == 0 {
                        c.dot(x, y, FILL, prio::WALL);
                    }
                }
            }
        }
    }
}

/// A ship outline at its position and heading. `sparse` draws only a few
/// dots (a shrouded ship seen by others).
pub fn ship_shape(c: &mut Canvas, p: &Player, cam: (i32, i32), colour: Color, prio: u8, sparse: bool, scale: f32) {
    let (hx, hy) = p.heading();
    let (rx, ry) = (-hy, hx);
    // drawn a little larger than the collision hull, so the shape reads
    let r = p.radius() * 1.5 / PX_PER_DOT * scale;
    let (px, py) = (p.x / PX_PER_DOT - cam.0 as f32, p.y / PX_PER_DOT - cam.1 as f32);
    let pts: Vec<(f32, f32)> = ships::outline(p.kind).iter().map(|&(lx, ly)| (px + (rx * lx - hx * ly) * r, py + (ry * lx - hy * ly) * r)).collect();
    if sparse {
        for &(x, y) in &pts {
            c.dot(x.round() as i32, y.round() as i32, colour, prio);
        }
        return;
    }
    for k in 0..pts.len() {
        c.line(pts[k], pts[(k + 1) % pts.len()], colour, prio);
    }
}

pub fn dim(c: Color, f: f32) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb((r as f32 * f) as u8, (g as f32 * f) as u8, (b as f32 * f) as u8),
        other => other,
    }
}

/// The arena around `centre` (world px), seen by player `me`.
pub fn draw_world(buf: &mut Buffer, area: Rect, w: &World, me: usize, centre: (f32, f32), fx: &Fx) -> (i32, i32) {
    let mut c = Canvas::new(area.width, area.height);
    let mut cam = camera(centre, &c);
    if fx.shake > 0.0 {
        let t = w.s.time * 40.0;
        cam.0 += (t.sin() * 2.0 * fx.shake / 0.5).round() as i32;
        cam.1 += ((t * 1.3).cos() * 2.0 * fx.shake / 0.5).round() as i32;
    }
    let time = w.s.time;
    stars(&mut c, cam);
    walls(&mut c, w, cam);
    let to = |x: f32, y: f32| (x / PX_PER_DOT - cam.0 as f32, y / PX_PER_DOT - cam.1 as f32);

    // pickups: a small pulsing ring
    for s in &w.s.spots {
        let Some(kind) = s.ready else { continue };
        let (x, y) = crate::arena::Arena::centre((s.tile.0 as usize, s.tile.1 as usize));
        let (sx, sy) = to(x, y);
        let r = 1.6 + 0.5 * (time * 5.0).sin();
        for k in 0..8 {
            let a = k as f32 / 8.0 * std::f32::consts::TAU + time * 2.0;
            c.dot((sx + a.cos() * r).round() as i32, (sy + a.sin() * r).round() as i32, pickup_colour(kind), prio::PICKUP);
        }
        c.dot(sx.round() as i32, sy.round() as i32, pickup_colour(kind), prio::PICKUP);
    }
    // mines: a small cross, blinking once armed
    for m in &w.s.mines {
        let (sx, sy) = to(m.x, m.y);
        let (sx, sy) = (sx.round() as i32, sy.round() as i32);
        let col = player_colour(w, m.owner as usize);
        let on = m.age < 0.7 || (time * 3.0).fract() < 0.7;
        let col = if on { col } else { dim(col, 0.45) };
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            c.dot(sx + dx, sy + dy, col, prio::MINE);
        }
    }
    // shots with a short trail
    for s in &w.s.shots {
        let col = player_colour(w, s.owner as usize);
        let (sx, sy) = to(s.x, s.y);
        let (tx, ty) = to(s.x - s.vx * 0.012, s.y - s.vy * 0.012);
        match s.kind {
            ShotKind::Gun => {
                c.line((tx, ty), (sx, sy), dim(col, 0.6), prio::SHOT);
                c.dot(sx.round() as i32, sy.round() as i32, Color::Rgb(255, 255, 230), prio::SHOT);
            }
            ShotKind::Bomb => {
                c.line((tx, ty), (sx, sy), dim(col, 0.6), prio::SHOT);
                let (bx, by) = (sx.round() as i32, sy.round() as i32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0), (0, -1)] {
                    c.dot(bx + dx, by + dy, col, prio::SHOT + 1);
                }
            }
        }
    }
    // effects
    for p in &fx.parts {
        let (sx, sy) = to(p.x, p.y);
        c.dot(sx.round() as i32, sy.round() as i32, p.colour, prio::FX);
    }
    for r in &fx.rings {
        let (sx, sy) = to(r.x, r.y);
        let rad = (r.r / PX_PER_DOT) * (0.3 + 0.7 * (r.t / 0.35));
        let n = (rad * 6.0).max(8.0) as i32;
        let col = if r.t < 0.12 { Color::Rgb(255, 255, 220) } else { Color::Rgb(255, 160, 70) };
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            c.dot((sx + a.cos() * rad).round() as i32, (sy + a.sin() * rad * 1.0).round() as i32, col, prio::BLAST);
        }
    }
    // ships: others first, ours on top
    for (i, p) in w.s.players.iter().enumerate() {
        let Some(p) = p else { continue };
        if !p.alive {
            continue;
        }
        let mine = i == me;
        // spawn guard: blink
        if p.guard > 0.0 && (time * 8.0).fract() < 0.35 {
            continue;
        }
        let col = player_colour(w, i);
        if p.shroud > 0.0 && !mine {
            if let Some(m) = w.player(me).filter(|m| m.alive)
                && (m.x - p.x).hypot(m.y - p.y) > 220.0 {
                    continue;
                }
            ship_shape(&mut c, p, cam, dim(col, 0.5), prio::SHIP, true, 1.0);
            continue;
        }
        let col = if mine && p.shroud > 0.0 { dim(col, 0.55) } else { col };
        // a hit flashes the ship white for a moment
        let col = if p.flash > 0.0 { Color::Rgb(255, 255, 255) } else { col };
        ship_shape(&mut c, p, cam, col, if mine { prio::ME } else { prio::SHIP }, false, 1.0);
    }
    c.blit(buf, area);
    cam
}

/// Names under the other ships, in plain text cells.
pub fn draw_names(buf: &mut Buffer, area: Rect, w: &World, me: usize, cam: (i32, i32)) {
    for (i, p) in w.s.players.iter().enumerate() {
        let Some(p) = p else { continue };
        if i == me || !p.alive || p.shroud > 0.0 {
            continue;
        }
        let col = ((p.x / PX_PER_DOT) as i32 - cam.0) / 2;
        let row = ((p.y / PX_PER_DOT) as i32 - cam.1) / 4 + 2;
        let name: String = p.name.chars().take(10).collect();
        let x0 = col - name.chars().count() as i32 / 2;
        if row < 0 || row >= area.height as i32 {
            continue;
        }
        for (k, ch) in name.chars().enumerate() {
            let x = x0 + k as i32;
            if x < 0 || x >= area.width as i32 {
                continue;
            }
            // the label wins over stars and walls behind it
            if let Some(cell) = buf.cell_mut((area.x + x as u16, area.y + row as u16)) {
                cell.set_char(ch).set_fg(dim(player_colour(w, i), 0.7));
            }
        }
    }
}

/// The whole arena, shrunk into `area`, with ships as dots.
pub fn draw_radar(buf: &mut Buffer, area: Rect, w: &World, me: usize, blink: bool) {
    let mut c = Canvas::new(area.width, area.height);
    let a = &w.arena;
    let sx = a.w as f32 / c.w as f32;
    let sy = a.h as f32 / c.h as f32;
    for y in 0..c.h {
        for x in 0..c.w {
            // a radar dot is wall when a fair share of its tiles are
            let (tx0, ty0) = ((x as f32 * sx) as i32, (y as f32 * sy) as i32);
            let (tx1, ty1) = (((x + 1) as f32 * sx).ceil() as i32, ((y + 1) as f32 * sy).ceil() as i32);
            let mut walls = 0;
            let mut all = 0;
            for ty in ty0..ty1.max(ty0 + 1) {
                for tx in tx0..tx1.max(tx0 + 1) {
                    all += 1;
                    walls += a.wall(tx, ty) as i32;
                }
            }
            if walls * 4 > all {
                c.dot(x, y, FILL, 1);
            }
        }
    }
    let px = |x: f32, y: f32| ((x / TILE / sx) as i32, (y / TILE / sy) as i32);
    for s in &w.s.spots {
        if let Some(k) = s.ready {
            let (x, y) = crate::arena::Arena::centre((s.tile.0 as usize, s.tile.1 as usize));
            let (dx, dy) = px(x, y);
            c.dot(dx, dy, dim(pickup_colour(k), 0.8), 3);
        }
    }
    for (i, p) in w.s.players.iter().enumerate() {
        let Some(p) = p else { continue };
        if !p.alive || (p.shroud > 0.0 && i != me) {
            continue;
        }
        let (dx, dy) = px(p.x, p.y);
        if i == me {
            if blink {
                for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    c.dot(dx + ox, dy + oy, Color::Rgb(255, 255, 255), 9);
                }
            }
        } else {
            c.dot(dx, dy, player_colour(w, i), 8);
            c.dot(dx + 1, dy, player_colour(w, i), 8);
        }
    }
    c.blit(buf, area);
}
