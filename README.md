# tspace

Zero-friction arena dogfights in the terminal, drawn in Braille. Up to
eight pilots, people or bots, in one arena. Your ship keeps every bit of
momentum you give it and bounces off the walls. Your energy is your shield
and your ammunition at once. First to 15 kills wins the match.

Inspired by SubSpace (Burst / Virgin Interactive, 1997). tspace is its own
game, with its own ships, arenas, art, sound and numbers. Its movement code
is adapted from [Zubspace](https://github.com/pkieltyka/zubspace) (MIT; see
`THIRD_PARTY.md`).

## Running

```bash
cargo run --release                    # title screen
cargo run --release -- --play          # straight into a game against bots
cargo run --release -- --host          # host: LAN players find you; Tab shows an invite ticket
cargo run --release -- --join          # find games on this network
cargo run --release -- --join TICKET   # join across the internet
cargo run --release -- --demo          # watch eight bots
cargo run --release -- --bots 7 --skill hard --ship weaver
cargo run --release -- --arena hollow --bots 2   # the small arena
cargo run --release -- --teams --play            # Red against Blue
```

You need a truecolour terminal with UTF-8, at least 60x20; 100x30 or bigger
is better, and a bigger terminal shows more of the arena. Play is best in a
terminal with the kitty keyboard protocol (kitty, WezTerm, foot, Ghostty and
others). Elsewhere a terminal repeats only the last key held, so you cannot
thrust, turn and fire at the same time. A gamepad avoids that in any
terminal.

## Controls

| Key | |
|---|---|
| W or Up | thrust (with no friction, you keep drifting) |
| S or Down | reverse thrust, to brake |
| A D or Left Right | turn |
| Space | guns (hold) |
| F | bomb (blast damage) |
| X | mine (goes off when an enemy comes near) |
| E | afterburner (hold, with thrust; it burns energy) |
| 1 2 3 4 | Dart, Lance, Anvil, Weaver |
| Tab | scores, plus the host's invite ticket |
| Esc | menu |
| H or F1 | help |
| F8 | sound on or off |

Gamepad: left stick or d-pad flies (up thrusts, down brakes, sideways
turns), right trigger fires, left trigger is the afterburner, A bomb, X
mine, Y scores, B or Start menu, Back help. Pads from SDL's controller
list are used; `TSPACE_ANY_PAD=1` accepts any.

## Modes

- **Free-for-all:** everyone for themselves, first to 15 kills.
- **Teams:** Red against Blue, first team to 25 kills. Teammates can't
  hurt each other, and each team flies in its own colours. Newcomers join
  the smaller team. Pick it on the title (*Mode*) or with `--teams`.

## Arenas

- **Foundry:** 160x160 tiles, for up to eight. Corner bases, a ring in
  the middle with pickups inside it.
- **Lattice:** 128x96, for four to six. A field of pillars to bank shots
  off and hide behind, around a block in the middle.
- **Hollow:** 96x72, for two to four. A corridor round the edge and a
  hall in the middle.

## Ships

| | |
|---|---|
| **Dart** | fast and fragile, quick light guns, one mine |
| **Lance** | the all-rounder, twin guns |
| **Anvil** | slow and tough, heavy bouncing bombs |
| **Weaver** | lays four mines, and its shots bounce off walls |

## Pickups

Pickups appear at fixed spots and come back after a while:

| | |
|---|---|
| **Overcharge** | more maximum energy for this life |
| **Scatter** | your guns fire a three-way spread |
| **Shroud** | off radar and hard to see |
| **Overdrive** | more thrust and top speed |
| **Repulsor** | a shockwave on the spot: nearby enemies are shoved away, their shots and mines nearby are wiped out |

## Playing together

One player hosts and the others join. There is no central server. The
host runs the game and picks the arena, and bots keep the empty seats
filled. Each guest flies its own ship ahead of the host's updates, so
steering feels immediate over the internet too.

- **Same network:** the host picks *Host a game*. The others pick *Join a
  LAN game* and the host shows up in their list (UDP port 31459).
- **Over the internet:** the host's scores card (Tab) shows an invite
  ticket, which is also saved in `~/.local/state/tspace/ticket.txt`. The
  others paste it into *Join with a ticket*, or run `tspace --join TICKET`.
  The connection uses [iroh](https://iroh.computer), which goes through a
  relay when a direct connection can't be made.

## Settings

The title screen remembers your arena, mode, bots, bot skill and ship in
`~/.config/tspace/settings.txt` (`$XDG_CONFIG_HOME/tspace`). Command-line
options override it for that run.

## Session log

`$XDG_STATE_HOME/tspace/session.log` (by default
`~/.local/state/tspace/session.log`). Set the level with `TSPACE_LOG`, for
example `TSPACE_LOG=debug`.
