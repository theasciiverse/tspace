//! Arenas: text grids, one character per 16 px tile.
//!
//! ```text
//! # comment
//! name: Foundry
//! ####...
//! #..S.*.
//! ```
//!
//! Comments (`# ` with a space) and `name:` come before the grid.
//! `#` wall, `.` (or space) open, `S` spawn point, `*` pickup spot. Rows may
//! differ in length; short rows are open to the right. Outside the grid is
//! solid. `tools/gen_arenas.py` made the built-in ones; edit the text freely.

pub const TILE: f32 = 16.0;

/// The built-in arenas: id, text. `tools/gen_arenas.py` made them.
pub const BUILTIN: &[(&str, &str)] = &[("foundry", include_str!("../arenas/foundry.txt")), ("hollow", include_str!("../arenas/hollow.txt"))];

#[derive(Clone, Debug)]
pub struct Arena {
    pub name: String,
    pub w: usize,
    pub h: usize,
    solid: Vec<bool>,
    /// Spawn points and pickup spots, as tile coordinates.
    pub spawns: Vec<(usize, usize)>,
    pub pickups: Vec<(usize, usize)>,
}

impl Arena {
    pub fn parse(text: &str) -> Result<Arena, String> {
        let mut name = String::from("Unnamed");
        let mut rows: Vec<&str> = Vec::new();
        for line in text.lines() {
            if rows.is_empty() {
                if line.starts_with("# ") {
                    continue; // a comment; the grid's first row uses no spaces
                }
                if let Some(n) = line.strip_prefix("name:") {
                    name = n.trim().to_string();
                    continue;
                }
                if line.trim().is_empty() {
                    continue;
                }
            }
            rows.push(line.trim_end_matches('\r'));
        }
        while rows.last().is_some_and(|r| r.trim().is_empty()) {
            rows.pop();
        }
        let h = rows.len();
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        if w < 8 || h < 8 {
            return Err(format!("arena {name} is {w}x{h} tiles; it needs at least 8x8"));
        }
        let mut solid = vec![false; w * h];
        let (mut spawns, mut pickups) = (Vec::new(), Vec::new());
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                match c {
                    '#' => solid[y * w + x] = true,
                    'S' => spawns.push((x, y)),
                    '*' => pickups.push((x, y)),
                    '.' | ' ' => {}
                    other => return Err(format!("arena {name}: unknown tile {other:?} at row {}, column {}", y + 1, x + 1)),
                }
            }
        }
        if spawns.is_empty() {
            return Err(format!("arena {name} has no spawn points (S)"));
        }
        Ok(Arena { name, w, h, solid, spawns, pickups })
    }

    pub fn builtin(id: &str) -> Arena {
        let text = BUILTIN.iter().find(|(n, _)| *n == id).map_or(BUILTIN[0].1, |(_, t)| t);
        Arena::parse(text).expect("built-in arenas parse")
    }

    pub fn wall(&self, tx: i32, ty: i32) -> bool {
        if tx < 0 || ty < 0 || tx >= self.w as i32 || ty >= self.h as i32 {
            return true;
        }
        self.solid[ty as usize * self.w + tx as usize]
    }

    /// Is the pixel point inside a wall?
    pub fn solid_at(&self, x: f32, y: f32) -> bool {
        self.wall((x / TILE).floor() as i32, (y / TILE).floor() as i32)
    }

    pub fn px_w(&self) -> f32 {
        self.w as f32 * TILE
    }

    pub fn px_h(&self) -> f32 {
        self.h as f32 * TILE
    }

    /// Clear straight line between two pixel points (sampled every 6 px).
    pub fn clear_line(&self, (x0, y0): (f32, f32), (x1, y1): (f32, f32)) -> bool {
        let d = (x1 - x0).hypot(y1 - y0);
        let n = (d / 6.0).ceil().max(1.0) as i32;
        (1..n).all(|i| {
            let f = i as f32 / n as f32;
            !self.solid_at(x0 + (x1 - x0) * f, y0 + (y1 - y0) * f)
        })
    }

    pub fn centre(t: (usize, usize)) -> (f32, f32) {
        ((t.0 as f32 + 0.5) * TILE, (t.1 as f32 + 0.5) * TILE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_parse() {
        for (id, _) in BUILTIN {
            let a = Arena::builtin(id);
            assert!(a.spawns.len() >= 8, "{id}: {} spawns", a.spawns.len());
            assert!(!a.pickups.is_empty());
            for &s in &a.spawns {
                assert!(!a.wall(s.0 as i32, s.1 as i32));
            }
        }
        let f = Arena::builtin("foundry");
        assert_eq!((f.w, f.h, f.name.as_str()), (160, 160, "Foundry"));
    }

    #[test]
    fn parse_rules() {
        let a = Arena::parse("name: T\n########\n#S.....#\n#..*...\n#......#\n#......#\n#......#\n#......#\n########\n").unwrap();
        assert_eq!((a.w, a.h), (8, 8));
        assert_eq!(a.spawns, vec![(1, 1)]);
        assert_eq!(a.pickups, vec![(3, 2)]);
        assert!(!a.wall(7, 2));
        assert!(a.wall(-1, 3) && a.wall(0, 0));
        assert!(Arena::parse("########\n#......#\n").is_err());
        assert!(Arena::parse("name: X\n########\n#.....?#\n#......#\n#......#\n#......#\n#......#\n#......#\n########").is_err());
    }
}
