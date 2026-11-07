//! The four ships. All numbers are tspace's own, tuned by feel: pixels,
//! seconds, turns per second, energy points.

use crate::zubspace::PlayerMovementTuning;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Kind {
    Dart,
    #[default]
    Lance,
    Anvil,
    Weaver,
}

pub const ALL: [Kind; 4] = [Kind::Dart, Kind::Lance, Kind::Anvil, Kind::Weaver];

#[derive(Clone, Copy, Debug)]
pub struct Gun {
    /// Shots side by side per trigger (1 or 2).
    pub barrels: u8,
    pub speed: f32,
    pub damage: f32,
    pub cost: f32,
    pub delay: f32,
    pub life: f32,
    /// Wall bounces before a shot dies.
    pub bounces: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Bomb {
    pub speed: f32,
    pub damage: f32,
    pub radius: f32,
    pub cost: f32,
    pub delay: f32,
    pub bounces: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub name: &'static str,
    pub blurb: &'static str,
    pub radius: f32,
    pub rotation: f32,
    pub thrust: f32,
    pub top_speed: f32,
    pub burn_thrust: f32,
    pub burn_speed: f32,
    pub burn_cost: f32,
    pub energy: f32,
    pub recharge: f32,
    pub bounce: f32,
    pub gun: Gun,
    pub bomb: Bomb,
    pub mines: u8,
    pub mine_cost: f32,
    pub mine_damage: f32,
}

impl Kind {
    pub fn stats(self) -> &'static Stats {
        &STATS[self as usize]
    }

    pub fn tuning(self, overdrive: bool) -> PlayerMovementTuning {
        let s = self.stats();
        let k = if overdrive { 1.4 } else { 1.0 };
        PlayerMovementTuning {
            rotation_per_second: s.rotation,
            base_thrust_acceleration: s.thrust * k,
            afterburner_thrust_acceleration: s.burn_thrust * k,
            base_top_speed: s.top_speed * k,
            afterburner_top_speed: s.burn_speed * k,
            afterburner_energy_per_second: s.burn_cost,
            recharge_per_second: s.recharge,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

pub const STATS: [Stats; 4] = [
    Stats {
        name: "Dart",
        blurb: "fast and fragile, quick light guns",
        radius: 9.0,
        rotation: 0.80,
        thrust: 420.0,
        top_speed: 420.0,
        burn_thrust: 700.0,
        burn_speed: 600.0,
        burn_cost: 420.0,
        energy: 900.0,
        recharge: 270.0,
        bounce: 0.75,
        gun: Gun { barrels: 1, speed: 950.0, damage: 85.0, cost: 22.0, delay: 0.11, life: 1.0, bounces: 0 },
        bomb: Bomb { speed: 600.0, damage: 420.0, radius: 36.0, cost: 260.0, delay: 1.0, bounces: 0 },
        mines: 1,
        mine_cost: 300.0,
        mine_damage: 450.0,
    },
    Stats {
        name: "Lance",
        blurb: "the all-rounder, twin guns",
        radius: 11.0,
        rotation: 0.62,
        thrust: 330.0,
        top_speed: 340.0,
        burn_thrust: 560.0,
        burn_speed: 500.0,
        burn_cost: 400.0,
        energy: 1200.0,
        recharge: 300.0,
        bounce: 0.65,
        gun: Gun { barrels: 2, speed: 850.0, damage: 70.0, cost: 36.0, delay: 0.17, life: 1.2, bounces: 0 },
        bomb: Bomb { speed: 520.0, damage: 620.0, radius: 48.0, cost: 300.0, delay: 0.9, bounces: 0 },
        mines: 2,
        mine_cost: 300.0,
        mine_damage: 620.0,
    },
    Stats {
        name: "Anvil",
        blurb: "slow and tough, heavy bouncing bombs",
        radius: 13.0,
        rotation: 0.46,
        thrust: 240.0,
        top_speed: 270.0,
        burn_thrust: 420.0,
        burn_speed: 400.0,
        burn_cost: 420.0,
        energy: 1800.0,
        recharge: 330.0,
        bounce: 0.5,
        gun: Gun { barrels: 1, speed: 700.0, damage: 140.0, cost: 40.0, delay: 0.3, life: 1.4, bounces: 0 },
        bomb: Bomb { speed: 480.0, damage: 900.0, radius: 64.0, cost: 380.0, delay: 0.75, bounces: 1 },
        mines: 2,
        mine_cost: 350.0,
        mine_damage: 900.0,
    },
    Stats {
        name: "Weaver",
        blurb: "mine-layer, shots that bounce off walls",
        radius: 11.0,
        rotation: 0.72,
        thrust: 310.0,
        top_speed: 330.0,
        burn_thrust: 540.0,
        burn_speed: 480.0,
        burn_cost: 400.0,
        energy: 1250.0,
        recharge: 310.0,
        bounce: 0.7,
        gun: Gun { barrels: 1, speed: 850.0, damage: 105.0, cost: 30.0, delay: 0.17, life: 1.8, bounces: 3 },
        bomb: Bomb { speed: 520.0, damage: 520.0, radius: 44.0, cost: 300.0, delay: 1.0, bounces: 0 },
        mines: 4,
        mine_cost: 200.0,
        mine_damage: 560.0,
    },
];

/// Outline of each ship in units of its radius, nose pointing up (-y),
/// as one closed polygon. Drawn rotated, so any heading works.
pub fn outline(kind: Kind) -> &'static [(f32, f32)] {
    match kind {
        // a slim arrow with a notched tail
        Kind::Dart => &[(0.0, -1.3), (0.75, 0.9), (0.0, 0.45), (-0.75, 0.9)],
        // a long triangle with tail fins
        Kind::Lance => &[(0.0, -1.3), (0.45, 0.2), (0.95, 0.95), (0.3, 0.7), (-0.3, 0.7), (-0.95, 0.95), (-0.45, 0.2)],
        // blunt and wide
        Kind::Anvil => &[(-0.45, -1.0), (0.45, -1.0), (1.0, -0.1), (1.0, 0.85), (-1.0, 0.85), (-1.0, -0.1)],
        // a diamond with forked prongs at the front
        Kind::Weaver => &[(-0.35, -1.25), (-0.15, -0.5), (0.15, -0.5), (0.35, -1.25), (0.9, 0.1), (0.0, 1.0), (-0.9, 0.1)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tile test looks only at the leading column, so no ship may move
    /// a whole tile in one 60 Hz step, even with Overdrive and afterburner.
    #[test]
    fn no_ship_outruns_a_tile_per_step() {
        for k in ALL {
            let t = k.tuning(true);
            assert!(t.afterburner_top_speed.max(t.base_top_speed) / 60.0 < crate::arena::TILE, "{:?}", k);
        }
    }
}
