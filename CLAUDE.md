# tspace

An Asciiverse version of **SubSpace** (Burst for Virgin Interactive,
public beta 1996, released 1997), the zero-friction multiplayer space
shooter, drawn in Braille in the terminal. It is built on the MIT code of
**Zubspace**, Peter Kieltyka's Rust remake of SubSpace for the browser.

**Status:** folder and notes only, 2026-10-27. No code. The direction is
settled (local play first, see below). The maps, the art and a letter to
Peter are still open (see the end).

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

## Plan (decided 2026-10-27)

**Local play first**, zubspace.gg later if Peter agrees:

1. **Engine:** Zubspace's shared movement and gameplay rules, vendored
   (MIT, credited), with `glam` in place of Macroquad's `Vec2`.
2. **Single player against bots**, from Zubspace's bots.
3. **LAN and internet play over iroh**, wired in directly as in Panzer and
   Tetrix: one player hosts, friends join, no central server.
4. **Our own content:** Braille ships, explosions and effects; our own
   arenas (1024x1024 tile grids, designed to read well at Braille scale);
   our own sound; our own zone settings (ship speeds, weapon damage,
   energy, prize rates) tuned to SubSpace's feel. Nothing from `assets/`,
   and not the Chaos or Extreme Games settings, which are community work
   under the same "no rights granted" notice.
5. **Later, a second network backend** for zubspace.gg, under the same
   game, once Peter agrees. The letter to him can wait until there is
   something to show.

Terminal native from the first line: Rust + Ratatui, drawn centred and
following resizes, session logging per the Asciiverse convention.

## Open questions (David's to decide)

- [ ] Trademark checks for tspace (EUIPO, PRH, USPTO) before announcing
- [ ] Arenas: how many for a first version, and what they look like
- [ ] The eight ships: kept as eight roles with our own look and names?
- [ ] Sound: style and source
- [ ] When to write to Peter, and what to ask for
- [ ] Licence (case by case, David's choice; the vendored Zubspace code
      stays MIT with its notice)
