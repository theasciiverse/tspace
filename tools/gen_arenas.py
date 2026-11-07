#!/usr/bin/env python3
"""Generates the built-in arenas from one quadrant of rectangles each,
mirrored four ways. Run once per change; the text files in arenas/ are
what the game loads and may be hand-edited afterwards.

    python3 tools/gen_arenas.py          # writes arenas/*.txt
"""

ARENAS = {
    # 160x160, for up to eight
    'foundry': dict(
        name='Foundry', w=160, h=160,
        walls=[
            (18, 6, 19, 20), (6, 18, 14, 19),          # corner base
            (30, 12, 32, 14), (12, 30, 14, 32), (46, 18, 48, 20), (18, 46, 20, 48),
            (32, 32, 34, 34), (52, 52, 54, 54),         # pillars
            (38, 28, 58, 29), (28, 38, 29, 58),         # long bars
            (64, 64, 73, 65), (64, 64, 65, 73),         # centre ring corners
            (60, 8, 61, 22), (8, 60, 22, 61),           # side baffles
            (70, 20, 79, 21), (20, 70, 21, 79),         # axis spurs
        ] + [(42 + 4 * k, 42 + 4 * k, 43 + 4 * k, 43 + 4 * k) for k in range(4)],  # stepping stones
        spawns=[(9, 9), (14, 10), (10, 14)],
        pickups=[(40, 40), (70, 10), (10, 70), (77, 77)],
    ),
    # 96x72, for two to four: a corridor round the edge, a hall in the middle
    'hollow': dict(
        name='Hollow', w=96, h=72,
        walls=[
            (8, 8, 26, 9), (8, 8, 9, 20),               # corner L
            (36, 14, 47, 15),                           # bar over the hall
            (20, 24, 22, 26),                           # pillar
            (34, 26, 35, 35),                           # hall sides
            (30, 4, 31, 9),                             # top baffle
            (4, 30, 9, 31),                             # side baffle
        ],
        spawns=[(4, 4), (16, 4), (4, 14)],
        pickups=[(24, 18), (47, 35)],
    ),
}


def build(a):
    w, h = a['w'], a['h']
    g = [['.'] * w for _ in range(h)]

    def put(x, y, c):
        for xx in (x, w - 1 - x):
            for yy in (y, h - 1 - y):
                g[yy][xx] = c

    for x0, y0, x1, y1 in a['walls']:
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                put(x, y, '#')
    for x in range(w):
        put(x, 0, '#')
    for y in range(h):
        put(0, y, '#')
    for x, y in a['spawns']:
        put(x, y, 'S')
    for x, y in a['pickups']:
        put(x, y, '*')
    head = '# tspace arena. Legend: # wall, . open, S spawn point, * pickup spot.\n'
    return head + f"name: {a['name']}\n" + ''.join(''.join(r) + '\n' for r in g)


if __name__ == '__main__':
    for key, a in ARENAS.items():
        with open(f'arenas/{key}.txt', 'w') as f:
            f.write(build(a))
