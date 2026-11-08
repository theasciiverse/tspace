# tspace roadmap

Plan of 2026-11-07 (see `CLAUDE.md`, "Plan (revised 2026-11-07)").
All eight phases built on 2026-11-07 as v0.1.0; play-testing comes next.

- [x] **0. Background:** CLAUDE.md update, this roadmap, `THIRD_PARTY.md`
- [x] **1. Skeleton:** Rust 2024 + Ratatui 0.30, Panzer's 60 Hz loop and
      held-key input, torched's Braille canvas and session log, centred
      view that follows resizes
- [x] **2. Flight and arena:** vendored Zubspace movement core, text-grid
      arenas, one first arena, camera on the ship, wall bounces
- [x] **3. Combat and ships:** energy, guns, bombs, mines, damage, death,
      respawn, score; Dart, Lance, Anvil, Weaver; four pickups
- [x] **4. Bots:** target, lead, wall avoidance, energy reserve, pickups;
      easy/medium/hard
- [x] **5. HUD and menus:** radar, energy bar, kill feed, scoreboard, ship
      select, title menu, help
- [x] **6. Multiplayer:** host-authoritative iroh (Panzer), LAN beacon,
      invite tickets (torched), bots fill empty slots
- [x] **7. Sound:** synthesised effects and thrust hum, `--mute`
- [x] **8. Packaging:** README, release workflow, hub drafts, `.cast`
      (no push, tag or release without David; drafts and cast done
      2026-11-07; licence MIT)

## Later

- [x] Guest-side prediction of the guest's own ship (2026-11-07)
- [x] Gamepad, gilrs as Panzer (2026-11-07)
- [x] Second arena, Hollow, for two to four (2026-11-07)
- More ships, arenas and pickups
- [x] Team mode, Red against Blue (2026-11-07)
- zubspace.gg backend, only with Peter's agreement

## 0.1.0: built

- Four ships, guns, bombs, mines and four pickups in the Foundry
- Bots at three skill levels
- LAN and internet play: one player hosts, bots fill the empty seats
- Radar, scores, help, sound and a demo mode

## 0.2.0: built

- Gamepad
- Guests fly their own ship ahead of the host's updates, so steering
  feels immediate over the internet
- A second arena, the Hollow, for two to four

## 0.3.0: built

- Team mode: Red against Blue, no friendly fire, first team to 25
- A third arena, the Lattice: a field of pillars for four to six
- A fifth pickup, the Repulsor: a shockwave that shoves enemies away and
  wipes out their shots and mines
- Bots flee when low, use the afterburner and dodge shots
- Hit flash, low-energy warning, kill streaks, and the camera follows
  your killer while you wait to fly again
- The title remembers your settings and pilot name
