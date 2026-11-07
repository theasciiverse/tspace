//! Movement and terrain rules adapted from Zubspace
//! (https://github.com/pkieltyka/zubspace, commit 245b40b), MIT licence,
//! Copyright (c) 2026 Peter Kieltyka and zubspace contributors. The full
//! notice and the list of changes are in THIRD_PARTY.md.
//!
//! Trimmed to what tspace uses: thrust, reverse, rotation, afterburner,
//! top-speed clamp and recharge; the two-axis wall bounce; the leading-edge
//! tile test; swept projectile motion. Units are pixels and seconds,
//! orientation is in turns (0..1, 0 = up, clockwise).

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MovementVector {
    pub x: f32,
    pub y: f32,
}

impl MovementVector {
    #[allow(dead_code)]
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }

    fn subtract(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }

    fn scale(self, f: f32) -> Self {
        Self::new(self.x * f, self.y * f)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerMovementInput {
    pub thrust: bool,
    pub reverse: bool,
    pub rotate_left: bool,
    pub rotate_right: bool,
    pub afterburner_held: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerMovementState {
    /// Orientation in turns: `0.0..1.0`, not radians.
    pub orientation: f32,
    pub velocity: MovementVector,
    pub energy: f32,
    pub max_energy: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerMovementTuning {
    pub rotation_per_second: f32,
    pub base_thrust_acceleration: f32,
    pub afterburner_thrust_acceleration: f32,
    pub base_top_speed: f32,
    pub afterburner_top_speed: f32,
    pub afterburner_energy_per_second: f32,
    pub recharge_per_second: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppliedThrust {
    None,
    Forward,
    Reverse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerMovementOutcome {
    pub state: PlayerMovementState,
    pub afterburning: bool,
    pub applied_thrust: AppliedThrust,
    /// The cap used before collision; re-apply it after a bounce.
    pub top_speed: f32,
}

/// One movement step: thrust along the pre-turn heading, rotation, top-speed
/// clamp, afterburner drain, then recharge.
pub fn step_player_movement(state: PlayerMovementState, input: PlayerMovementInput, tuning: PlayerMovementTuning, dt: f32) -> PlayerMovementOutcome {
    let dt = dt.max(0.0);
    let mut velocity = state.velocity;
    let thrust_heading = orientation_to_heading(state.orientation);

    let wants_afterburner = input.afterburner_held && (input.thrust || input.reverse);
    let afterburner_cost = tuning.afterburner_energy_per_second * dt;
    let afterburning = wants_afterburner && state.energy > afterburner_cost;

    let thrust_acceleration = if afterburning { tuning.afterburner_thrust_acceleration } else { tuning.base_thrust_acceleration };

    let applied_thrust = if input.reverse {
        velocity = velocity.subtract(thrust_heading.scale(thrust_acceleration * dt));
        AppliedThrust::Reverse
    } else if input.thrust {
        velocity = velocity.add(thrust_heading.scale(thrust_acceleration * dt));
        AppliedThrust::Forward
    } else {
        AppliedThrust::None
    };

    let orientation = rotate_orientation_step(state.orientation, input.rotate_left, input.rotate_right, tuning.rotation_per_second * dt);

    let top_speed = if afterburning { tuning.afterburner_top_speed } else { tuning.base_top_speed };
    velocity = truncate_movement_vector(velocity, top_speed);

    let mut energy = state.energy;
    if afterburning {
        energy -= afterburner_cost;
    }
    energy = (energy + tuning.recharge_per_second * dt).min(state.max_energy);

    PlayerMovementOutcome {
        state: PlayerMovementState { orientation, velocity, energy, max_energy: state.max_energy },
        afterburning,
        applied_thrust,
        top_speed,
    }
}

pub fn truncate_movement_vector(vector: MovementVector, maximum: f32) -> MovementVector {
    let length = vector.length();
    if length > maximum && length > 0.0 { vector.scale(maximum.max(0.0) / length) } else { vector }
}

/// Left subtracts and wraps up, right adds and wraps down, in that order.
pub fn rotate_orientation_step(orientation: f32, rotate_left: bool, rotate_right: bool, rotation_step: f32) -> f32 {
    let mut next = orientation;
    if rotate_left {
        next -= rotation_step;
        if next < 0.0 {
            next += 1.0;
        }
    }
    if rotate_right {
        next += rotation_step;
        if next >= 1.0 {
            next -= 1.0;
        }
    }
    next
}

/// Unit heading for an orientation in turns: 0 points up (negative y),
/// a quarter turn points right.
pub fn orientation_to_heading(orientation: f32) -> MovementVector {
    let a = orientation * std::f32::consts::TAU;
    MovementVector::new(a.sin(), -a.cos())
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CollisionIntegration {
    pub position: MovementVector,
    pub velocity: MovementVector,
    pub bounced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionAxis {
    X,
    Y,
}

/// One collision step: the X axis, then the Y axis with the velocity the X
/// bounce left. A blocked axis keeps its coordinate, reflects its velocity
/// and scales both components by `bounce_factor`.
pub fn integrate_player_collision<F>(position: MovementVector, mut velocity: MovementVector, dt: f32, bounce_factor: f32, mut blocked: F) -> CollisionIntegration
where
    F: FnMut(MovementVector, CollisionAxis, f32) -> bool,
{
    let dt = dt.max(0.0);
    let start = position;
    let mut position = position;
    let mut bounced = false;

    let delta_x = velocity.x * dt;
    let candidate_x = MovementVector::new(start.x + delta_x, start.y);
    if blocked(candidate_x, CollisionAxis::X, delta_x) {
        velocity.x = -velocity.x * bounce_factor;
        velocity.y *= bounce_factor;
        bounced = true;
    } else {
        position.x = candidate_x.x;
    }

    let delta_y = velocity.y * dt;
    let candidate_y = MovementVector::new(position.x, start.y + delta_y);
    if blocked(candidate_y, CollisionAxis::Y, delta_y) {
        velocity.y = -velocity.y * bounce_factor;
        velocity.x *= bounce_factor;
        bounced = true;
    } else {
        position.y = candidate_y.y;
    }

    CollisionIntegration { position, velocity, bounced }
}

/// Tests only the movement-leading column (X) or row (Y) of tiles against a
/// square hull of `radius` around the pixel-truncated position. Outside the
/// map counts as blocked.
#[allow(clippy::too_many_arguments)]
pub fn movement_axis_overlaps_blocked_tile(
    position: MovementVector,
    radius: f32,
    map_w: usize,
    map_h: usize,
    tile_size: f32,
    axis: CollisionAxis,
    signed_delta: f32,
    mut blocked_tile: impl FnMut(i32, i32) -> bool,
) -> bool {
    if !position.x.is_finite() || !position.y.is_finite() || !radius.is_finite() || radius < 0.0 || map_w == 0 || map_h == 0 || tile_size <= 0.0 || !signed_delta.is_finite() {
        return true;
    }
    let center = MovementVector::new(position.x.trunc(), position.y.trunc());
    let hull_min = MovementVector::new(center.x - radius, center.y - radius);
    let hull_max = MovementVector::new(center.x + radius, center.y + radius);

    let (axis_position, perpendicular_position, axis_len) = match axis {
        CollisionAxis::X => (position.x, position.y, map_w),
        CollisionAxis::Y => (position.y, position.x, map_h),
    };
    let leading = if signed_delta < 0.0 { ((axis_position - radius) / tile_size).floor() as i32 } else { ((axis_position + radius) / tile_size).trunc() as i32 };
    if leading < 0 || leading >= axis_len as i32 {
        return true;
    }
    let perpendicular_start = ((perpendicular_position - radius - tile_size) / tile_size).trunc() as i32;
    let perpendicular_end = ((perpendicular_position + radius + tile_size) / tile_size).trunc() as i32;

    for perpendicular in perpendicular_start..perpendicular_end {
        let (tile_x, tile_y) = match axis {
            CollisionAxis::X => (leading, perpendicular),
            CollisionAxis::Y => (perpendicular, leading),
        };
        let out_of_bounds = tile_x < 0 || tile_y < 0 || tile_x >= map_w as i32 || tile_y >= map_h as i32;
        if (out_of_bounds || blocked_tile(tile_x, tile_y)) && movement_aabb_overlaps_tile(hull_min, hull_max, tile_x, tile_y, tile_size) {
            return true;
        }
    }
    false
}

/// Touching a tile's min edge counts as overlap; touching its max edge does not.
pub fn movement_aabb_overlaps_tile(hull_min: MovementVector, hull_max: MovementVector, tile_x: i32, tile_y: i32, tile_size: f32) -> bool {
    let tile_min = MovementVector::new(tile_x as f32 * tile_size, tile_y as f32 * tile_size);
    let tile_max = MovementVector::new(tile_min.x + tile_size, tile_min.y + tile_size);
    hull_max.x >= tile_min.x && hull_min.x < tile_max.x && hull_max.y >= tile_min.y && hull_min.y < tile_max.y
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerrainContact {
    pub x: bool,
    pub y: bool,
}

impl TerrainContact {
    pub fn any(self) -> bool {
        self.x || self.y
    }
}

/// Swept point motion for projectiles: samples every 8 px so nothing skips
/// a 16 px wall. A blocked axis keeps its coordinate (and reflects if
/// `bounce`); Y uses the resolved X.
pub fn step_terrain(x: &mut f32, y: &mut f32, vx: &mut f32, vy: &mut f32, dt: f32, bounce: bool, is_solid: impl Fn(f32, f32) -> bool) -> TerrainContact {
    fn swept(x: f32, y: f32, dx: f32, dy: f32, solid: &impl Fn(f32, f32) -> bool) -> bool {
        let samples = (dx.abs().max(dy.abs()) / 8.0).ceil().max(1.0) as u32;
        (1..=samples).any(|i| {
            let f = i as f32 / samples as f32;
            solid(x + dx * f, y + dy * f)
        })
    }
    let dx = *vx * dt;
    let dy = *vy * dt;
    let cx = swept(*x, *y, dx, 0.0, &is_solid);
    if cx {
        if bounce {
            *vx = -*vx;
        }
    } else {
        *x += dx;
    }
    let cy = swept(*x, *y, 0.0, dy, &is_solid);
    if cy {
        if bounce {
            *vy = -*vy;
        }
    } else {
        *y += dy;
    }
    TerrainContact { x: cx, y: cy }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> PlayerMovementTuning {
        PlayerMovementTuning {
            rotation_per_second: 0.5,
            base_thrust_acceleration: 300.0,
            afterburner_thrust_acceleration: 500.0,
            base_top_speed: 200.0,
            afterburner_top_speed: 300.0,
            afterburner_energy_per_second: 100.0,
            recharge_per_second: 50.0,
        }
    }

    fn still() -> PlayerMovementState {
        PlayerMovementState { orientation: 0.0, velocity: MovementVector::ZERO, energy: 500.0, max_energy: 1000.0 }
    }

    #[test]
    fn thrust_up_and_clamp() {
        let mut s = still();
        let input = PlayerMovementInput { thrust: true, ..Default::default() };
        for _ in 0..600 {
            s = step_player_movement(s, input, tuning(), 1.0 / 60.0).state;
        }
        assert!(s.velocity.y < -199.0 && s.velocity.y >= -200.001, "{:?}", s.velocity);
        assert!(s.velocity.x.abs() < 1e-3);
        assert!((s.energy - 1000.0).abs() < 0.1);
    }

    #[test]
    fn no_friction() {
        let mut s = still();
        s.velocity = MovementVector::new(50.0, -20.0);
        let s2 = step_player_movement(s, PlayerMovementInput::default(), tuning(), 1.0).state;
        assert_eq!(s2.velocity, s.velocity);
    }

    #[test]
    fn rotation_wraps() {
        assert!((rotate_orientation_step(0.1, true, false, 0.25) - 0.85).abs() < 1e-6);
        assert!((rotate_orientation_step(0.9, false, true, 0.25) - 0.15).abs() < 1e-6);
        let h = orientation_to_heading(0.25);
        assert!((h.x - 1.0).abs() < 1e-6 && h.y.abs() < 1e-6);
    }

    #[test]
    fn afterburner_needs_energy() {
        let mut s = still();
        s.energy = 0.5;
        let input = PlayerMovementInput { thrust: true, afterburner_held: true, ..Default::default() };
        let o = step_player_movement(s, input, tuning(), 1.0 / 60.0);
        assert!(!o.afterburning);
        s.energy = 500.0;
        let o = step_player_movement(s, input, tuning(), 1.0 / 60.0);
        assert!(o.afterburning && o.state.energy < 500.0);
    }

    #[test]
    fn bounce_off_wall() {
        // solid from tile column 10 (x = 160) rightwards
        let wall = |tx: i32, _ty: i32| tx >= 10;
        let pos = MovementVector::new(140.0, 100.0);
        let vel = MovementVector::new(300.0, 40.0);
        let r = integrate_player_collision(pos, vel, 0.1, 0.5, |p, axis, d| movement_axis_overlaps_blocked_tile(p, 12.0, 64, 64, 16.0, axis, d, wall));
        assert!(r.bounced);
        assert_eq!(r.position.x, 140.0);
        assert_eq!(r.velocity.x, -150.0);
        assert_eq!(r.velocity.y, 20.0);
    }

    #[test]
    fn projectile_reflects() {
        let solid = |x: f32, _y: f32| x >= 160.0;
        let (mut x, mut y, mut vx, mut vy) = (150.0, 50.0, 600.0, 0.0);
        let c = step_terrain(&mut x, &mut y, &mut vx, &mut vy, 0.05, true, solid);
        assert!(c.x && vx == -600.0 && x == 150.0);
    }
}
