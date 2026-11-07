//! Everything drawn over the arena: the status bar, radar, kill feed,
//! cards (title, scores, help, menus), all in plain text cells.

use crate::game::{Game, MENU, PAUSE_MENU, Phase, Role, TOAST_FOR};
use crate::render::{self, dim, pickup_colour, player_colour, team_colour};
use crate::world::TEAM_NAMES;
use crate::ships;
use crate::world::PickupKind;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};

const INK: Color = Color::Rgb(10, 12, 20);
const TEXT: Color = Color::Rgb(215, 220, 235);
const MUTED: Color = Color::Rgb(125, 132, 155);
const ACCENT: Color = Color::Rgb(90, 220, 255);
const WARN: Color = Color::Rgb(255, 170, 60);

const BANNER: [&str; 2] = ["▀█▀ █▀▀ █▀█ ▄▀█ █▀▀ █▀▀", " █  ▄▄█ █▀▀ █▀█ █▄▄ ██▄"];

pub const MIN_W: u16 = 60;
pub const MIN_H: u16 = 20;

pub fn draw(f: &mut Frame, g: &Game) {
    let full = f.area();
    f.render_widget(Block::default().style(Style::default().bg(INK)), full);
    if full.width < MIN_W || full.height < MIN_H {
        let p = Paragraph::new(format!("tspace needs at least {MIN_W}x{MIN_H}; this terminal is {}x{}.", full.width, full.height)).style(Style::default().fg(TEXT)).wrap(Wrap { trim: true });
        f.render_widget(p, full);
        return;
    }
    let playing = g.phase == Phase::Play;
    let view = if playing { Rect { height: full.height - 1, ..full } } else { full };
    let centre = camera_centre(g);
    let cam = render::draw_world(f.buffer_mut(), view, &g.world, g.me, centre, &g.fx);
    if playing {
        render::draw_names(f.buffer_mut(), view, &g.world, g.me, cam);
        radar(f, g, view);
        feed(f, g, view);
        status(f, g, Rect { y: full.height - 1, height: 1, ..full });
        if !g.exact_keys && g.time < 12.0 && !g.is_guest() && !g.spectate && !g.pad {
            let hint = "Tip: a terminal with the kitty keyboard protocol lets you thrust, turn and fire at once";
            let w = (hint.len() as u16).min(view.width);
            f.render_widget(Paragraph::new(hint).style(Style::default().fg(MUTED)), Rect { x: view.x + (view.width - w) / 2, y: view.y + view.height - 1, width: w, height: 1 });
        }
    }
    toast(f, g, full);
    match g.phase {
        Phase::Title => title(f, g, full),
        Phase::Browse => browse(f, g, full),
        Phase::Ticket => ticket(f, g, full),
        Phase::Connecting => {
            card(f, full, "Connecting", vec![Line::from("Reaching the host…"), Line::from(""), hint_line("Esc cancels")], ACCENT, 44);
        }
        Phase::Play => {
            if let Some((winner, left)) = g.world.s.over {
                let name = if g.world.s.teams { format!("{} team", TEAM_NAMES[winner as usize % 2]) } else { g.world.player(winner as usize).map_or("?".into(), |p| p.name.clone()) };
                scores(f, g, full, Some(format!("{name} wins! Next match in {}s", left.ceil() as i32)));
            } else if g.scores {
                scores(f, g, full, None);
            } else if let Some(p) = g.world.player(g.me).filter(|p| !p.alive && !g.spectate) {
                dead(f, g, p, full);
            }
            if let Some(k) = g.paused_menu {
                pause(f, g, k, full);
            }
        }
    }
    if g.help {
        help(f, full);
    }
}

/// Our ship, or where we died, or (on the title) whichever bot we follow.
fn camera_centre(g: &Game) -> (f32, f32) {
    match g.world.player(g.me) {
        Some(p) => (p.x, p.y),
        None => (g.world.arena.px_w() / 2.0, g.world.arena.px_h() / 2.0),
    }
}

fn hint_line(s: &str) -> Line<'static> {
    Line::from(Span::styled(s.to_string(), Style::default().fg(MUTED)))
}

fn card(f: &mut Frame, full: Rect, title: &str, lines: Vec<Line>, accent: Color, width: u16) -> Rect {
    let w = width.min(full.width.saturating_sub(4)).max(20);
    let h = (lines.len() as u16 + 2).min(full.height);
    let area = Rect { x: full.x + (full.width - w) / 2, y: full.y + (full.height.saturating_sub(h)) / 2, width: w, height: h };
    f.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(format!(" {title} "), Style::default().fg(accent).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(INK).fg(TEXT));
    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), area);
    area
}

fn radar(f: &mut Frame, g: &Game, view: Rect) {
    // square in dots: twice as many columns as rows
    let rows = (view.height / 4).clamp(6, 14);
    let cols = rows * 2;
    let area = Rect { x: view.x + view.width - cols - 2, y: view.y, width: cols + 2, height: rows + 2 };
    f.render_widget(Clear, area);
    let block = Block::bordered().border_type(BorderType::Rounded).border_style(Style::default().fg(dim(ACCENT, 0.5))).style(Style::default().bg(INK));
    let inner = block.inner(area);
    f.render_widget(block, area);
    render::draw_radar(f.buffer_mut(), inner, &g.world, g.me, (g.time * 3.0).fract() < 0.7);
}

fn feed(f: &mut Frame, g: &Game, view: Rect) {
    for (k, (line, t)) in g.feed.iter().enumerate() {
        let age = g.time - t;
        let colour = if age < 4.0 { TEXT } else { MUTED };
        let w = (line.chars().count() as u16 + 1).min(view.width / 2);
        let area = Rect { x: view.x + 1, y: view.y + k as u16, width: w, height: 1 };
        f.render_widget(Paragraph::new(line.as_str()).style(Style::default().fg(colour).bg(INK)), area);
    }
}

fn toast(f: &mut Frame, g: &Game, full: Rect) {
    let Some((msg, t)) = &g.toast else { return };
    if g.time - t > TOAST_FOR {
        return;
    }
    let w = (msg.chars().count() as u16 + 4).min(full.width - 2);
    let y = if g.phase == Phase::Play { full.y + full.height / 4 } else { full.y + 1 };
    let area = Rect { x: full.x + (full.width - w) / 2, y, width: w, height: 1 };
    f.render_widget(Paragraph::new(msg.as_str()).alignment(Alignment::Center).style(Style::default().fg(INK).bg(ACCENT)), area);
}

fn status(f: &mut Frame, g: &Game, area: Rect) {
    let Some(p) = g.world.player(g.me) else { return };
    let st = p.kind.stats();
    let mut spans = vec![Span::styled(format!(" {} ", st.name), Style::default().fg(INK).bg(player_colour(&g.world, g.me)).add_modifier(Modifier::BOLD)), Span::raw(" ")];
    let bar_w = 20usize;
    let frac = if p.max_energy > 0.0 { (p.energy / p.max_energy).clamp(0.0, 1.0) } else { 0.0 };
    let filled = (frac * bar_w as f32).round() as usize;
    let colour = if frac > 0.6 {
        Color::Rgb(110, 230, 120)
    } else if frac > 0.3 {
        Color::Rgb(240, 210, 80)
    } else if frac > 0.25 || (g.time * 4.0).fract() < 0.5 {
        Color::Rgb(255, 90, 80)
    } else {
        // low energy: the bar pulses
        Color::Rgb(120, 40, 36)
    };
    spans.push(Span::styled("█".repeat(filled), Style::default().fg(colour)));
    spans.push(Span::styled("░".repeat(bar_w - filled), Style::default().fg(dim(colour, 0.35))));
    spans.push(Span::styled(format!(" {:>4}/{:<4} ", p.energy as i32, p.max_energy as i32), Style::default().fg(TEXT)));
    spans.push(Span::styled(format!(" K {} D {} ", p.kills, p.deaths), Style::default().fg(TEXT)));
    if g.world.s.teams {
        spans.extend(team_line(g).spans);
    }
    let mines = g.world.s.mines.iter().filter(|m| m.owner as usize == g.me).count();
    spans.push(Span::styled(format!(" mines {}/{} ", st.mines as usize - mines.min(st.mines as usize), st.mines), Style::default().fg(MUTED)));
    for (t, kind) in [(p.scatter, PickupKind::Scatter), (p.shroud, PickupKind::Shroud), (p.overdrive, PickupKind::Overdrive)] {
        if t > 0.0 {
            spans.push(Span::styled(format!(" {} {}s ", kind.name(), t.ceil() as i32), Style::default().fg(pickup_colour(kind))));
        }
    }
    if p.max_energy > st.energy + 1.0 {
        spans.push(Span::styled(" Overcharged ", Style::default().fg(pickup_colour(PickupKind::Overcharge))));
    }
    let right = match &g.role {
        Role::Host(_) => format!(" hosting · {} humans · Tab scores ", g.world.count(false)),
        Role::Guest(gu) => format!(" in {}'s game · Tab scores ", gu.host_name),
        Role::Solo if g.spectate => format!(" demo · watching {} · Esc menu ", p.name),
        Role::Solo => " Tab scores · Esc menu · H help ".into(),
    };
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let pad = (area.width as usize).saturating_sub(used + right.chars().count());
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled(right, Style::default().fg(MUTED)));
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Rgb(22, 26, 40))), area);
}

fn dead(f: &mut Frame, g: &Game, p: &crate::world::Player, full: Rect) {
    let by = p.last_hit.and_then(|k| g.world.player(k as usize)).map(|k| k.name.clone());
    let headline = match (p.deaths, by) {
        (0, _) => "Warping in".to_string(),
        (_, Some(n)) => format!("Shot down by {n}"),
        (_, None) => "Shot down".to_string(),
    };
    let mut ship_spans = vec![Span::raw(" ")];
    for (k, kind) in ships::ALL.iter().enumerate() {
        let st = kind.stats();
        let style = if *kind == p.kind { Style::default().fg(INK).bg(ACCENT) } else { Style::default().fg(TEXT) };
        ship_spans.push(Span::styled(format!(" {} {} ", k + 1, st.name), style));
        ship_spans.push(Span::raw(" "));
    }
    let lines = vec![
        Line::from(Span::styled(headline, Style::default().fg(WARN).add_modifier(Modifier::BOLD))),
        Line::from(format!("Back in {:.1}s as {} — {}", p.respawn.max(0.0), p.kind.stats().name, p.kind.stats().blurb)),
        Line::from(""),
        Line::from(ship_spans),
        hint_line(" 1-4 picks a ship for your next life"),
    ];
    let area = Rect { y: full.y + full.height * 2 / 3 - 3, height: 7, ..full };
    card(f, area, "Down", lines, WARN, 64);
}

fn scores(f: &mut Frame, g: &Game, full: Rect, headline: Option<String>) {
    let mut lines = Vec::new();
    if let Some(h) = &headline {
        lines.push(Line::from(Span::styled(h.clone(), Style::default().fg(WARN).add_modifier(Modifier::BOLD))));
        lines.push(Line::from(""));
    }
    if g.world.s.teams {
        lines.push(team_line(g));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(format!("{:<3}{:<14}{:<8}{:>6}{:>7}", "", "Pilot", "Ship", "Kills", "Deaths"), Style::default().fg(MUTED))));
    for (rank, &i) in g.world.ranking().iter().enumerate() {
        let p = g.world.player(i).unwrap();
        let tag = if p.bot { "·" } else { " " };
        let style = if i == g.me { Style::default().fg(player_colour(&g.world, i)).add_modifier(Modifier::BOLD) } else { Style::default().fg(player_colour(&g.world, i)) };
        let name: String = p.name.chars().take(12).collect();
        lines.push(Line::from(Span::styled(format!("{:<3}{:<13}{}{:<8}{:>6}{:>7}", rank + 1, name, tag, p.kind.stats().name, p.kills, p.deaths), style)));
    }
    lines.push(Line::from(""));
    let goal = if g.world.s.teams { format!("First team to {} kills wins.", g.world.s.kill_limit) } else { format!("First to {} kills wins.", g.world.s.kill_limit) };
    lines.push(hint_line(&format!("{goal} · marks a bot.")));
    if let Role::Host(_) = &g.role {
        lines.push(Line::from(""));
        match g.host_ticket() {
            Some(t) => {
                lines.push(Line::from(Span::styled("Invite ticket for internet play (also in ticket.txt in the state folder):", Style::default().fg(MUTED))));
                lines.push(Line::from(Span::styled(t.to_string(), Style::default().fg(ACCENT))));
            }
            None => lines.push(hint_line("Getting an invite ticket… (LAN players can join already)")),
        }
    }
    let width = if g.host_ticket().is_some() { 80 } else { 50 };
    let title = format!("{} · {}", if headline.is_some() { "Match over" } else { "Scores" }, g.world.arena.name);
    card(f, full, &title, lines, ACCENT, width);
}

fn pause(f: &mut Frame, g: &Game, k: usize, full: Rect) {
    let mut lines = Vec::new();
    for (i, item) in PAUSE_MENU.iter().enumerate() {
        let style = if i == k { Style::default().fg(INK).bg(ACCENT) } else { Style::default().fg(TEXT) };
        lines.push(Line::from(Span::styled(format!("  {item}  "), style)));
    }
    lines.push(Line::from(""));
    let note = if matches!(g.role, Role::Solo) { "The game waits while this is open." } else { "The game goes on while this is open." };
    lines.push(hint_line(note));
    card(f, full, "Menu", lines, ACCENT, 42);
}

fn title(f: &mut Frame, g: &Game, full: Rect) {
    let mut lines: Vec<Line> = BANNER.iter().map(|b| Line::from(Span::styled(b.to_string(), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)))).collect();
    lines.push(hint_line("zero-friction arena dogfights in Braille"));
    lines.push(Line::from(""));
    for (i, item) in MENU.iter().enumerate() {
        let value = match i {
            4 => format!(" ‹ {} ›", arena_name(g.settings.arena)),
            5 => format!(" ‹ {} ›", if g.settings.teams { "Teams" } else { "Free-for-all" }),
            6 => format!(" ‹ {} ›", g.settings.bots),
            7 => format!(" ‹ {} ›", g.settings.level.name()),
            8 => format!(" ‹ {} ›", g.settings.ship.stats().name),
            _ => String::new(),
        };
        let text = format!("{item}{value}");
        let style = if i == g.menu { Style::default().fg(INK).bg(ACCENT) } else { Style::default().fg(TEXT) };
        lines.push(Line::from(Span::styled(format!("  {text:<28}"), style)));
    }
    lines.push(Line::from(""));
    let blurb = match g.menu {
        4 => match g.settings.arena {
            "hollow" => "small, for two to four · ← → to change".to_string(),
            _ => "big, for up to eight · ← → to change".to_string(),
        },
        5 if g.settings.teams => "Red against Blue, no friendly fire · ← →".to_string(),
        5 => "everyone for themselves · ← → to change".to_string(),
        8 => g.settings.ship.stats().blurb.to_string(),
        6 | 7 => "← → to change".to_string(),
        _ => "↑↓ choose · Enter go · H help · Esc quit".to_string(),
    };
    lines.push(hint_line(&blurb));
    let area = card(f, full, "tspace", lines.into_iter().map(|l| l.alignment(Alignment::Center)).collect(), ACCENT, 40);
    let credit = format!("{} · inspired by SubSpace (1997) · movement code from Zubspace (MIT)", crate::game::version_string());
    let w = (credit.chars().count() as u16).min(full.width);
    if area.y + area.height < full.height {
        f.render_widget(Paragraph::new(credit).style(Style::default().fg(MUTED).bg(INK)), Rect { x: full.x + (full.width - w) / 2, y: full.height - 1, width: w, height: 1 });
    }
}

/// "Red 12 · Blue 9", in the teams' colours.
fn team_line(g: &Game) -> Line<'static> {
    let t = |k: u8| Span::styled(format!(" {} {} ", TEAM_NAMES[k as usize], g.world.team_kills(k)), Style::default().fg(team_colour(k)).add_modifier(Modifier::BOLD));
    Line::from(vec![t(0), Span::styled("·", Style::default().fg(MUTED)), t(1)])
}

fn arena_name(id: &str) -> String {
    crate::arena::BUILTIN.iter().find(|(a, _)| *a == id).and_then(|(_, t)| t.lines().find_map(|l| l.strip_prefix("name:"))).map_or(id.to_string(), |n| n.trim().to_string())
}

fn browse(f: &mut Frame, g: &Game, full: Rect) {
    let hosts = g.browser.as_ref().map(|b| b.hosts()).unwrap_or_default();
    let mut lines = Vec::new();
    if hosts.is_empty() {
        lines.push(Line::from("Looking for games on this network…"));
        lines.push(hint_line("A friend picks Host a game; it shows up here."));
    }
    for (k, h) in hosts.iter().enumerate() {
        let style = if k == g.browse_pick { Style::default().fg(INK).bg(ACCENT) } else { Style::default().fg(TEXT) };
        lines.push(Line::from(Span::styled(format!("  {}'s game · {} humans  ", h.beacon.name, h.beacon.players), style)));
    }
    lines.push(Line::from(""));
    lines.push(hint_line("Enter join · Esc back"));
    card(f, full, "Join a LAN game", lines, ACCENT, 50);
}

fn ticket(f: &mut Frame, g: &Game, full: Rect) {
    let shown = if g.ticket_text.is_empty() { "(paste the ticket here)".to_string() } else { g.ticket_text.clone() };
    let lines = vec![
        Line::from("Paste the host's invite ticket (from their scores card or ticket.txt):"),
        Line::from(""),
        Line::from(Span::styled(shown, Style::default().fg(if g.ticket_text.is_empty() { MUTED } else { ACCENT }))),
        Line::from(""),
        hint_line("Enter join · Backspace erase · Esc back"),
    ];
    card(f, full, "Join with a ticket", lines, ACCENT, 80);
}

fn help(f: &mut Frame, full: Rect) {
    let k = |key: &str, what: &str| Line::from(vec![Span::styled(format!("{key:<16}"), Style::default().fg(ACCENT)), Span::raw(what.to_string())]);
    let lines = vec![
        k("W  ↑", "thrust (no friction: you keep drifting)"),
        k("S  ↓", "reverse thrust, to brake"),
        k("A D  ← →", "turn"),
        k("Space", "guns (hold)"),
        k("F", "bomb: blast damage"),
        k("X", "mine: waits, then goes off near enemies"),
        k("E", "afterburner (hold, with thrust): burns energy"),
        k("1 2 3 4", "Dart, Lance, Anvil, Weaver (warps you out)"),
        k("Tab", "scores (and the host's invite ticket)"),
        k("Esc", "menu · F8 sound · H this help"),
        k("Gamepad", "stick flies · RT guns · LT burn · A bomb · X mine · Y scores"),
        Line::from(""),
        Line::from("Energy is your shield and your ammunition: it refills on its own."),
        Line::from(vec![
            Span::raw("Pickups: "),
            Span::styled("Overcharge", Style::default().fg(pickup_colour(PickupKind::Overcharge))),
            Span::raw(" more energy, "),
            Span::styled("Scatter", Style::default().fg(pickup_colour(PickupKind::Scatter))),
            Span::raw(" spread shot, "),
            Span::styled("Shroud", Style::default().fg(pickup_colour(PickupKind::Shroud))),
            Span::raw(" off radar, "),
            Span::styled("Overdrive", Style::default().fg(pickup_colour(PickupKind::Overdrive))),
            Span::raw(" speed."),
        ]),
        Line::from(""),
        hint_line("Esc or Enter closes this"),
    ];
    card(f, full, "How to fly", lines, ACCENT, 72);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Audio;
    use crate::bots::Level;
    use crate::game::{Settings, Start};
    use crate::input::Input;
    use crate::ships::Kind;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// The whole screen draws, in both modes and both arenas, at the
    /// recording size and a small one, with the scores card up.
    #[test]
    fn screens_draw() {
        for (arena, teams) in [("foundry", false), ("hollow", true)] {
            let settings = Settings { arena, teams, bots: 5, level: Level::Hard, ship: Kind::Lance };
            let mut g = Game::new(Start::Demo, settings, true);
            let (mut input, mut audio) = (Input::new(true), Audio::new(true));
            for _ in 0..600 {
                g.tick(&mut input, &mut audio);
            }
            for (w, h) in [(100, 30), (60, 20), (40, 10)] {
                let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                t.draw(|f| draw(f, &g)).unwrap();
                g.scores = true;
                t.draw(|f| draw(f, &g)).unwrap();
                g.scores = false;
            }
            assert_eq!(g.world.s.teams, teams);
        }
    }
}
