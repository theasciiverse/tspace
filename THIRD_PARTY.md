# Third-party code

## Zubspace

`src/zubspace.rs` is adapted from Zubspace by Peter Kieltyka and
contributors (https://github.com/pkieltyka/zubspace), commit `245b40b`
(2026-11-05):

- `src/gameplay/player_movement.rs`: `step_player_movement` (trimmed to
  thrust, reverse, rotation, afterburner, top-speed clamp and recharge),
  `truncate_movement_vector`, `rotate_orientation_step`,
  `integrate_player_collision`, `movement_axis_overlaps_blocked_tile`
- `src/gameplay/shared_rules.rs`: `reference_game_player_hull_overlaps_tile`
- `src/gameplay/weapon_motion.rs`: `step_terrain`

Changes: removed the SubSpace-specific parts (wormhole gravity, rockets,
turrets, flaggers, repels, shutdown, EMP), replaced the Continuum heading
table with a plain sine/cosine heading, and dropped the tests that check
against Continuum protocol encodings. Only code is used: nothing from
Zubspace's `assets/` (graphics, sounds, maps, zone settings), which its
`ASSETS.md` says is not covered by the licence.

```
MIT License

Copyright (c) 2026 Peter Kieltyka and zubspace contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Asciiverse siblings

Loop, input, networking (`src/net.rs`, `src/lan.rs`) and the synthesiser
layout follow Panzer (MIT). The Braille canvas, the invite-ticket encoding
and the session-log setup come from torched. torched as a whole is
GPL-2.0-only because it is a port of Xscorch, but those pieces are
Asciiverse's own code, written for torched, not taken from Xscorch, so
they are used here under tspace's MIT licence.
