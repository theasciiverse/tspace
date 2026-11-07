#!/usr/bin/env python3
"""Generates arenas/foundry.txt: one quadrant of rectangles, mirrored four ways.
Run once; the text file is what the game loads and what gets hand-edited."""
N = 160
H = N // 2
g = [['.'] * N for _ in range(N)]

walls = [
    (18, 6, 19, 20), (6, 18, 14, 19),          # corner base
    (30, 12, 32, 14), (12, 30, 14, 32), (46, 18, 48, 20), (18, 46, 20, 48),
    (32, 32, 34, 34), (52, 52, 54, 54),         # pillars
    (38, 28, 58, 29), (28, 38, 29, 58),         # long bars
    (64, 64, 73, 65), (64, 64, 65, 73),         # centre ring corners
    (60, 8, 61, 22), (8, 60, 22, 61),           # side baffles
    (70, 20, 79, 21), (20, 70, 21, 79),         # axis spurs
]
for k in range(4):                               # diagonal stepping stones
    a = 42 + 4 * k
    walls.append((a, a, a + 1, a + 1))
spawns = [(9, 9), (14, 10), (10, 14)]
pickups = [(40, 40), (70, 10), (10, 70), (77, 77)]

def put(x, y, c):
    for xx in (x, N - 1 - x):
        for yy in (y, N - 1 - y):
            g[yy][xx] = c

for x0, y0, x1, y1 in walls:
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            put(x, y, '#')
for x in range(N):
    put(x, 0, '#')
    put(0, x, '#')
for x, y in spawns:
    put(x, y, 'S')
for x, y in pickups:
    put(x, y, '*')

with open('arenas/foundry.txt', 'w') as f:
    f.write('# tspace arena. Legend: # wall, . open, S spawn point, * pickup spot.\n')
    f.write('name: Foundry\n')
    for row in g:
        f.write(''.join(row) + '\n')
