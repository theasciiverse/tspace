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
```

You need a truecolour terminal with UTF-8, at least 60x20; 100x30 or bigger
is better, and a bigger terminal shows more of the arena. Play is best in a
terminal with the kitty keyboard protocol (kitty, WezTerm, foot, Ghostty and
others). Elsewhere a terminal repeats only the last key held, so you cannot
thrust, turn and fire at the same time.

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

## Playing together

One player hosts and the others join. There is no central server. The
host runs the game, and bots keep the empty seats filled.

- **Same network:** the host picks *Host a game*. The others pick *Join a
  LAN game* and the host shows up in their list (UDP port 31459).
- **Over the internet:** the host's scores card (Tab) shows an invite
  ticket, which is also saved in `~/.local/state/tspace/ticket.txt`. The
  others paste it into *Join with a ticket*, or run `tspace --join TICKET`.
  The connection uses [iroh](https://iroh.computer), which goes through a
  relay when a direct connection can't be made.

## Session log

`$XDG_STATE_HOME/tspace/session.log` (by default
`~/.local/state/tspace/session.log`). Set the level with `TSPACE_LOG`, for
example `TSPACE_LOG=debug`.
