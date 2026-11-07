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

- Guest-side prediction of the guest's own ship, for internet latency
- More ships, arenas and pickups
- Gamepad (gilrs, as Panzer)
- Team modes
- zubspace.gg backend, only with Peter's agreement
