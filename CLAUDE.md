# tspace

An Asciiverse version of **SubSpace** (Burst for Virgin Interactive,
public beta 1996, released 1997), the zero-friction multiplayer space
shooter, drawn in Braille in the terminal. It is built on the MIT code of
**Zubspace**, Peter Kieltyka's Rust remake of SubSpace for the browser.

**Status:** 2026-11-07: v0.1.0 playable, not committed or pushed. Bots,
LAN and ticket play, sound, HUD, demo mode and preview cast are all in
(phases in `ROADMAP.md`). Licence MIT (2026-11-07). 2026-10-27: folder and notes only.

## Name

**tspace**, chosen by David 2026-10-27: terminal space, with a nod to
SubSpace. It cannot be called SubSpace (David). Folder, binary and title.

Collision check, 2026-10-27: no Steam game, no crate, no itch.io game, no
Debian package or `tspace` command. GitHub: `mtklr/tspace` ("terminal
space"), a small C starfield effect from W.P. van Paassen's Demo Effect
Collection (2002, GPL), not a game; MikkTSpace (tangent-space standard for
3D baking, crates `mikktspace`, `bevy_mikktspace`); unrelated Node, Julia
and crypto projects. **Not yet checked:** trademark registers (EUIPO, PRH,
USPTO).

Names checked and dropped on the way: Termspace (`ba2slk/termspace` is a
released Electron terminal app of that name, xterm.js and node-pty, v1.7.0
of 2026-10-05, plus several 2026 terminal workspace tools), Driftwar (Drift
Wars on Steam), Ricochet, Inertia, Rebound, Zeroth (Steam games and
crates), Hyperdrift and Nullspace (itch.io). Clear but not taken: Vauhti
(speed, momentum), Kitkaton (frictionless), Kitka (friction, clash), Liuku
(slide, skid).

## The game

Top-down 2D, a fixed camera on your ship, zero friction: momentum you have
to manage and ships that bounce off walls. Eight ships, guns, bombs, mines,
repels, prizes (burst, rocket, decoy, brick, thor, stealth, cloak, x-radar,
antiwarp, portal), energy that is both health and ammunition, a radar, and
zones like Chaos (free for all) and Extreme Games. A finalist for the
Academy of Interactive Arts & Sciences Online Game of the Year in 1998.
Players kept it alive after Virgin stopped running it, and zones still run
today.

## Background, checked 2026-10-27

### The original source code does not exist in the open

- Virgin never released the SubSpace client (SubSpace) or server (SubGame)
  source. Wikipedia: Continuum was made by reverse engineering "without
  access to the original source code".
- After Virgin Interactive collapsed in 1998, EA bought much of what was
  left, but not SubSpace (Wikipedia). Dan Luu's history
  (danluu.com/subspace-history) says a company called Ozaq2 holds the
  SubSpace copyrights; not confirmed anywhere else.
- `github.com/fatrolls/subgame-continuum-subspace-clone` is **not source
  code**: it is Hex-Rays decompiler output (`subgame.c` says so in its
  header) of SubGame, Continuum 0.40 and the 1.32 client, with VIE's
  original EXEs and zone configs. No licence. Not something to build on;
  at most a reference for how SubGame behaves.
- **Continuum** (PriitK and Mr Ekted, first released 2001) is a clean
  reimplementation of the client. Closed source, no licence found; free on
  Steam since 2015 (app 352700).
- **ASSS** (A Small Subspace Server) is the real open source piece: a full
  server written from scratch in C and Python, **GPL**, with util, pathutil,
  protutil and md5 in the public domain (licence read in `fcxcode/eg-asss`).
  Live forks: `fcxcode/eg-asss` (Extreme Games), `Periapsis/asss`,
  `twdev-repos/asss`, `Ceiu/hyperspace-asss` (the Hyperspace zone). The
  best documentation of the Continuum protocol there is.
- **Discretion** (ss-discretion.sourceforge.net): an old GPL C++ client for
  Windows, Mac and Linux.

### Zubspace

`github.com/pkieltyka/zubspace`, play at zubspace.gg. Created 2026-10-25,
very active. Code **MIT**. About 11 MB of Rust plus the page's JavaScript.

- **Client:** Rust compiled to WASM, drawn with Peter's Macroquad fork. A
  native build of the same binary runs an unnetworked local game with
  client-side bots (`docs/architecture.md`).
- **Server:** native Rust, authoritative, 60 ticks a second; owns movement,
  weapons, damage, prizes, scoring and bots.
- **Shared rules:** deterministic movement and gameplay rules compiled into
  both binaries: `src/gameplay/player_movement.rs`,
  `src/gameplay/shared_rules.rs`, `src/authority*`, `src/protocol/*`. The
  rules files have no Macroquad or WASM imports. The one tie is
  `src/physics.rs`, which takes `Vec2` from `macroquad::prelude` (that is
  glam), so swapping in `glam` directly is straightforward. There is no
  library target: modules are shared with `#[path]` attributes, so reuse
  means vendoring the files or adding a lib target.
- **Bots:** `src/bot.rs` (client side) and the server's own bots.
- **Assets are not licensed.** `ASSETS.md`: everything under `assets/`
  (VIE and Continuum graphics, sounds and fonts, and the community's Chaos
  and Extreme Games maps, zones and art), plus `js/zubspace_ships.png`,
  `js/zubspace_icons.png` and `js/zubspace_share.png`, belongs to its
  creators and no rights are granted. We use none of it.
- Sizes from `src/constants.rs`: tiles 16x16 px (`TILE_SIZE`), ships 36x36
  px (`SHIP_FRAME`), maps 1024x1024 tiles (`MAP_TILES`).

### Can a terminal client play on zubspace.gg?

Technically yes, today. Checked in the code and `deploy/prod/` on
2026-10-27, and `https://zubspace.gg/v1/status` and `/v1/zones` answered
(Chaos Zone open with 15 players, Extreme on standby).

- Signaling is one `POST /v1/rtc/offer` with a JSON SDP offer
  (non-trickle), answered with the SDP answer
  (`src/bin/zubspace_server/connection.rs`, `docs/networking.md`). Three
  DataChannels: `zubspace-game` (unordered, maxPacketLifeTime 80),
  `zubspace-events` (unordered), `zubspace-reliable` (ordered). The server
  already uses webrtc-rs natively.
- Prod has no auth token, no captcha (`deploy/README.md` says never to put
  a challenge in front of the offer endpoint), no user-agent, build-hash or
  client-version check. The security packet checks only the settings and
  map checksums, computed from data the server sends.
- The map, LVZ files and zone settings come from the server at runtime
  (`docs/protocol_compatibility.md`, ZMAP `0x29`, file transfer `0x2a`,
  settings `0x0f`), so a client needs no bundled zone data.
- **The catch, Origin:** prod accepts only `Origin: https://zubspace.gg`
  (`deploy/prod/zubspace_server.toml`). A native client could send that
  header, but that is posing as his web page; not for a catalog title.
- **The catch, stability:** browser and server deploy together from one
  commit, decoders reject unknown versions, and many packets are already at
  v2 or v3. The only version signal is the client protocol revision byte
  after the arena login (currently 3). Any deploy of his can break an
  outside client.
- His docs say nothing about third-party clients, for or against.

So playing on zubspace.gg needs Peter: an allowed origin or token for the
Asciiverse client, and notice before protocol changes. Running his MIT
server ourselves is possible, but it loads maps and settings from his
unlicensed `assets/`, so it needs our own zones first.

### Does it work in a terminal?

Yes, well. The classic 640x480 screen shows 40x30 tiles. At one tile per
2x1 cells (cells are about twice as tall as wide), a 100x30 terminal shows
50x30 tiles: the whole original view plus a little more width, at our
standard recording size. In Braille a tile is then 4x4 dots and a ship
about 9x9 dots (4 or 5 columns by 2 or 3 rows), enough to show a nose,
wings and heading. Ships are drawn as shapes, so any rotation angle (the
original had 40 pre-drawn frames). Walls are solid tiles, bullets single
dots with trails, bombs, mines and prizes small coloured glyphs; radar,
chat and scoreboard are text already. Lost: the painted sprites, the
explosion animations and the zones' LVZ art, which we replace with our own.
A bigger terminal simply shows more of the map, like a bigger monitor in
Continuum.

## Update 2026-11-07: Zubspace again, and what we take

Checked with `gh api` on 2026-11-07.

- **Little change.** Default branch `master`, two commits since 10-27:
  `d64d3df` (chat, controls, connection recovery, authority) and `245b40b`
  (laptop-screen HUD). No files moved. Still one crate with no lib target;
  the server mounts shared files with `#[path]`. Code MIT; `ASSETS.md`
  still grants no rights to `assets/` or `js/zubspace_*.png`.
- **The movement core is self-contained.** `src/gameplay/player_movement.rs`
  (2035 lines) and `src/gameplay/weapon_motion.rs` (103) import nothing:
  own vector types, map lookups passed in as closures, collision X axis
  first then Y, a bounce reflects the axis and scales by `bounce_factor`.
  No glam swap needed. `shared_rules.rs` (1628) is also import-free but
  half of it is SubSpace door, prize and energy rules.
- **The rest is SubSpace-shaped.** `src/authority/*` sits on Continuum
  packet helpers (`continuum_weapon`, `continuum_position`); maps are
  `.lvl` (4-byte records on a fixed 1024x1024 grid); every ship and weapon
  number comes from the Chaos and Extreme `arena.conf` files, parsed into
  a Continuum settings packet. The values copied into `arena_settings/
  chaos.rs` and `constants.rs` are that same community data. The bots
  (about 5000 lines in `server_tick/bots.rs`, `state/bots*`) are tied to
  `ServerState`.
- **Determinism:** the server replays client input and must match five
  `f32`s bit for bit (`docs/authoritative_server.md`). We don't need that:
  Panzer's host-authoritative model sends state, not inputs to replay.

**So:** take only the movement core. In practice `player_movement.rs` does
call two Continuum helpers (the heading table and the hull test), and its
tests check against protocol encodings, so `src/zubspace.rs` is a trimmed,
credited adaptation, not an unchanged copy: thrust, rotation,
afterburner, clamp, recharge, the two-axis bounce, the leading-edge tile
test and `step_terrain` (list in `THIRD_PARTY.md`). Weapons, energy,
damage, arenas, ships and bots are our own. Everything else comes from our own games: Panzer's loop,
input, rodio synth and net code, torched's Braille canvas, tickets and
session log.

## Originality boundary

tspace is an original game in the zero-friction arena shooter genre, not a
SubSpace recreation. Its name, ships, weapons, pickups, arenas, Braille
art, sound and all tuning numbers are our own. No SubSpace, Continuum or
Zubspace names, sprites, maps, zone names or settings, not even the values
in Zubspace's `chaos.rs`. Genre mechanics (momentum, wall bounces, energy
as health and ammo, bombs, mines) are fair game. The README says
"inspired by SubSpace (1997)" and credits the vendored code.

## Plan (revised 2026-11-07)

David's choices: four ships of our own, a core weapon set with our own
names, and v1 = bots plus LAN/internet play together. Phases and their
state are in `ROADMAP.md`.

- **Ships:** Dart (fast, light guns), Lance (all-rounder), Anvil (slow,
  heavy bombs, big energy), Weaver (mines, bouncing shots). Names are
  proposals.
- **Weapons:** guns, bombs with blast damage, mines; energy is health and
  ammo and recharges.
- **Pickups:** Overcharge (more max energy this life), Scatter (spread
  shot), Shroud (off radar for a while), Afterburner (speed boost).
- **Arenas:** our own text grids in `arenas/*.txt`, size per arena; one
  tile is 2x1 cells, 4x4 Braille dots.
- **Bots:** our own, small: target, lead the aim, avoid walls, keep energy.
  Easy/medium/hard by reaction delay and aim error.
- **Network:** Panzer's host-authoritative iroh model, LAN discovery,
  torched-style invite tickets; bots fill empty slots on the host.
- **Later:** a zubspace.gg backend only if Peter agrees; letter once there
  is something to show.

Terminal native: Rust + Ratatui, drawn centred and following resizes,
session log in `$XDG_STATE_HOME/tspace/session.log`.

## Open questions (David's to decide)

- [~] Trademark checks, 2026-11-07, by web search only (TMview, EUIPO
      eSearch and PRH's service refuse automated queries, so this needs
      a person to finish): no TSPACE mark found anywhere. Near marks in the
      US, none for games or software: T-SPACE, Volkswagen (cars, classes
      12/35/37, reg. 7362289); T-SPACE, Time Span Technology Shenzhen
      (kitchenware 21, reg. 6797805; clothing 25, reg. 7468702); TESPACE
      (vehicle parts, pending). Still to do by hand: TMview
      (tmdn.org/tmview, covers EUIPO and PRH) and USPTO search for
      "tspace" and "t space" in classes 9 and 41.
- [x] Arenas: one first, our own text grids (2026-11-07)
- [x] Ships: four of our own, more later (2026-11-07)
- [x] Sound: synthesised at run time, as Panzer (2026-11-07)
- [ ] Writing to Peter: not needed for anything built. The MIT licence
      allows our use as long as the notice is kept (THIRD_PARTY.md). Only
      a zubspace.gg backend would need him (an allowed origin, notice of
      protocol changes). A heads-up is optional courtesy.
- [x] Licence: MIT (David, 2026-11-07), as Panzer; `LICENSE`, the
      Zubspace notice in `THIRD_PARTY.md`

## Running

```bash
cargo run --release                 # title (bots fight behind the menu)
cargo run --release -- --play       # straight into a game, 5 bots by default
cargo run --release -- --host       # host; --join lists LAN games, --join TICKET dials one
cargo run --release -- --demo       # eight bots, camera on the leader
cargo run --release -- --bots 7 --skill hard --ship weaver --mute
```

Controls, ships and pickups are in `README.md`. Session log:
`$XDG_STATE_HOME/tspace/session.log` (`TSPACE_LOG=debug` adds a
snapshot every 2 s). The host's ticket is also written to `ticket.txt`
in the same folder.

## Design decisions (2026-11-07)

- **Units:** pixels, seconds, turns, as in Zubspace. 16 px tiles. Drawn
  at 4 px per Braille dot, so a tile is 4x4 dots (2 cells wide, 1 tall).
  Ships are drawn at 1.5x their hull radius so the shapes read; collision
  uses the hull.
- **Speed limit:** the tile test looks only at the leading column, so no
  ship may move a whole tile in one 60 Hz step (`ships::tests`).
- **Arena:** `arenas/foundry.txt`, 160x160, four-way symmetric, 12
  spawns, 16 pickup spots. Generated once by `tools/gen_foundry.py`; the
  text file is the source of truth now.
- **Match:** free-for-all, first to 15 kills, then 10 s of scores and a
  new match. 3 s respawn, 1.5 s spawn guard (cancelled by firing). A
  ship change warps you out for 1.2 s and counts as no death. No self
  damage.
- **Rendering:** torched's Braille canvas (one colour per cell, by
  priority), two parallax star layers, walls as edge dots with a sparse
  fill, a radar of the whole arena (top right), names under ships.
- **Network:** Panzer's host-authoritative model. Guests send `Controls`
  every tick. The host sends the whole `world::State` plus events at
  30 Hz, about 2-6 kB at eight players (`proto::tests`). Joining is open
  at any time and a guest takes a bot's seat. 8 players at most.
  ALPN `tspace/1`, LAN beacon on UDP 31459, torched-style tickets
  (`tspace` + base32 of the iroh address). The host emits its ticket
  once iroh is online (up to 8 s); LAN play needs no internet. There is
  no guest-side prediction yet, so over the internet a guest feels the
  round trip.
- **Sound:** synthesised (`src/audio.rs`): chirp guns, bombs, blasts,
  hits, death, pickups, spawn, plus an engine voice for your own thrust.
  Other pilots' guns are quieter with distance.
- **Input:** Panzer's rule. Exact with the kitty protocol, otherwise held
  keys are inferred from repeats. The game shows a tip in the first
  seconds when the protocol is missing.

## Code layout

- `src/zubspace.rs`: the adapted movement core (MIT notice at the top).
- `src/arena.rs`: the text-grid format and the built-in arena.
- `src/ships.rs`: the four ships' stats and outlines.
- `src/world.rs`: the simulation (`State` is what goes on the wire).
- `src/bots.rs`: computer pilots.
- `src/game.rs`: roles (solo, host, guest), phases, menus, events into
  effects, sounds and the feed.
- `src/render.rs`: Braille canvas, stars, walls, ships, effects, radar.
- `src/hud.rs`: status bar, cards, title, help.
- `src/input.rs`, `src/audio.rs`.
- `src/proto.rs`, `src/net.rs`, `src/lan.rs`: wire format, iroh,
  LAN beacon (from Panzer).
- `tools/ansi2png.py`: Panzer's capture converter, now drawing Braille
  as dots. `tools/gen_foundry.py`: the arena generator.
- `asciiverse/record-preview.sh`: records `asciiverse/tspace.cast`
  (100x30, about 47 s of `--demo` with the scores card in the middle,
  11.6 MB).

## Verifying

```bash
cargo test --release      # 24 tests: movement, arena, rules, bots fight, frame size, tickets, sound
cargo clippy --release
```

Checked 2026-11-07 in tmux at 100x30 (private socket `tmux -L tspace`):
title, solo play, help, scores. Host plus a LAN guest plus a ticket
guest, three processes on one machine over real iroh: joining, a guest's
thrust reaching the host, the guest list, and a guest leaving. Not tried
yet: two physical machines, a real internet join across NAT, or a
person playing for any length of time (bot strength and ship balance are
untuned).

## Next

- Play it: tune ship numbers and bot skill by feel.
- Guest-side prediction of your own ship for internet play.
- More arenas (smaller ones for 2-4 players), more ships and pickups.
- Gamepad (gilrs, as Panzer).
- Trademark checks, then hub entry and release (David).
