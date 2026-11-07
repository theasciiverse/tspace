//! Wire messages between a tspace host and its guests.
//!
//! The host runs the only simulation. Guests send their controls every
//! tick; the host sends everyone a `Frame` (the whole world state plus the
//! events since the last frame) 30 times a second. Guests fly their own
//! ship ahead of the frames and correct when one arrives (`Frame::acks`).
//! Encoding is postcard inside u32-length-prefixed frames, as in Panzer
//! and Tetrix.

use crate::world::{Controls, Event, State};
use serde::{Deserialize, Serialize};

/// Bumped on any incompatible change; the host turns away other versions.
pub const VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Msg {
    /// A guest's first message.
    Hello { name: String, version: u32 },
    Reject { reason: String },
    /// Which player slot is the guest's, and the arena.
    Welcome { slot: u8, arena: String },
    /// `acks`: for each guest's slot, the last input the host applied, so
    /// the guest can replay the ones after it (its prediction).
    Frame { state: State, events: Vec<Event>, acks: Vec<(u8, u32)> },
    /// A guest's controls for one tick, numbered.
    Input { seq: u32, controls: Controls },
    /// Keeps a quiet connection from timing out.
    Ping,
}

pub fn encode(m: &Msg) -> Vec<u8> {
    postcard::to_allocvec(m).expect("postcard encoding of an owned message cannot fail")
}

pub fn decode(b: &[u8]) -> Option<Msg> {
    postcard::from_bytes(b).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::{Brain, Level, NAMES};
    use crate::world::World;

    /// A busy eight-player frame stays small enough to send 30 times a
    /// second to seven guests.
    #[test]
    fn round_trip_and_size() {
        let mut w = World::new("foundry", 5);
        let mut brains: Vec<Brain> = (0..8).map(|k| Brain::new(Level::Hard, 3 + k)).collect();
        for (k, b) in brains.iter_mut().enumerate() {
            let ship = b.pick_ship();
            w.add_player(NAMES[k].into(), true, ship);
        }
        let mut biggest = 0;
        for _ in 0..60 * 40 {
            let c: Vec<Controls> = brains.iter_mut().enumerate().map(|(i, b)| b.think(&w, i, 1.0 / 60.0)).collect();
            w.step(1.0 / 60.0, &c);
            let events: Vec<Event> = w.events.drain(..).collect();
            let bytes = encode(&Msg::Frame { state: w.s.clone(), events: events.clone(), acks: vec![(3, 12345), (5, 99)] });
            biggest = biggest.max(bytes.len());
            match decode(&bytes) {
                Some(Msg::Frame { state, events: e, .. }) => {
                    assert_eq!(state.shots.len(), w.s.shots.len());
                    assert_eq!(e, events);
                }
                other => panic!("decoded {other:?}"),
            }
        }
        assert!(biggest < 6000, "biggest frame {biggest} bytes");
        assert!(decode(&[0xff, 0xff]).is_none());
    }
}
