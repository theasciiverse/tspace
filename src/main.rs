//! tspace: zero-friction arena dogfights in the terminal, drawn in Braille.

mod arena;
mod audio;
mod bots;
mod game;
mod hud;
mod input;
mod lan;
mod net;
mod proto;
mod render;
mod ships;
mod world;
mod zubspace;

use game::{DT, Game, Phase, Settings, Start};
use input::Input;
use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const FRAME: Duration = Duration::from_micros(16_667);
const SNAPSHOT_EVERY: Duration = Duration::from_secs(2);

const HELP: &str = "tspace [options]

Zero-friction arena dogfights in the terminal, drawn in Braille.
In the game, H shows the controls.

  --play            straight into a game against bots
  --demo            watch eight bots fight
  --host            host a game (LAN players find it; Tab shows an invite ticket)
  --join            look for games on the local network
  --join TICKET     join a host by its invite ticket (works across the internet)
  --arena NAME      foundry (big, up to eight), lattice (pillars, four to six)
                    or hollow (small, two to four)
  --teams           Red against Blue instead of everyone for themselves
  --name NAME       your pilot name (saved; default: your login name)
  --bots N          bots in your games, 0-7 (default 5)
                    (the title screen remembers arena, mode, bots, skill and
                    ship in $XDG_CONFIG_HOME/tspace/settings.txt; options here
                    override it for one run)
  --skill LEVEL     easy, medium or hard (default medium)
  --ship NAME       dart, lance, anvil or weaver (default lance)
  --mute            no sound
  --version

Session log: $XDG_STATE_HOME/tspace/session.log (~/.local/state/...),
level from TSPACE_LOG (default info).";

/// The state folder: session log and the host's ticket.
pub fn state_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?.join("tspace");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Session log in the state folder; the terminal is the game's.
fn init_logging() -> Option<PathBuf> {
    let path = state_dir()?.join("session.log");
    let file = std::fs::File::create(&path).ok()?;
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_env("TSPACE_LOG")
        .target(env_logger::Target::Pipe(Box::new(file)))
        .format(|buf, rec| writeln!(buf, "{} {:<5} {}", buf.timestamp_millis(), rec.level(), rec.args()))
        .init();
    Some(path)
}

struct Args {
    start: Start,
    settings: Settings,
    mute: bool,
}

fn parse_args() -> Result<Option<Args>, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut a = Args { start: Start::Title, settings: Settings::load(), mute: false };
    let mut i = 0;
    let value = |i: usize, flag: &str| args.get(i + 1).filter(|v| !v.starts_with("--")).cloned().ok_or(format!("{flag} needs a value"));
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return Ok(None);
            }
            "--version" => {
                println!("{}", game::version_string());
                return Ok(None);
            }
            "--play" => a.start = Start::Solo,
            "--demo" => a.start = Start::Demo,
            "--host" => a.start = Start::Host,
            "--mute" => a.mute = true,
            "--teams" => a.settings.teams = true,
            "--join" => match args.get(i + 1).filter(|v| !v.starts_with("--")) {
                Some(t) => {
                    a.start = Start::Join(net::decode_ticket(t)?);
                    i += 1;
                }
                None => a.start = Start::Browse,
            },
            "--name" => {
                a.settings.name = game::clean_name(&value(i, "--name")?);
                a.settings.save();
                i += 1;
            }
            "--arena" => {
                let v = value(i, "--arena")?.to_lowercase();
                a.settings.arena = arena::BUILTIN.iter().map(|(id, _)| *id).find(|id| *id == v).ok_or(format!("--arena {v}: try foundry, lattice or hollow"))?;
                i += 1;
            }
            "--bots" => {
                let v = value(i, "--bots")?;
                a.settings.bots = v.parse::<usize>().map_err(|_| format!("--bots {v}: not a number"))?.min(7);
                i += 1;
            }
            "--skill" => {
                let v = value(i, "--skill")?;
                a.settings.level = bots::Level::ALL.into_iter().find(|l| l.name() == v.to_lowercase()).ok_or(format!("--skill {v}: try easy, medium or hard"))?;
                i += 1;
            }
            "--ship" => {
                let v = value(i, "--ship")?;
                a.settings.ship = ships::ALL.into_iter().find(|k| k.stats().name.eq_ignore_ascii_case(&v)).ok_or(format!("--ship {v}: try dart, lance, anvil or weaver"))?;
                i += 1;
            }
            other => return Err(format!("unknown option {other} (try --help)")),
        }
        i += 1;
    }
    Ok(Some(a))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(Some(a)) => a,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tspace: {e}");
            return ExitCode::from(2);
        }
    };
    let log_path = init_logging();
    log::info!("{} starting", game::version_string());
    // open audio before the alternate screen, so any ALSA chatter lands above it
    let mut audio = audio::Audio::new(args.mute);
    log::info!("audio available {} muted {}", audio.available(), audio.muted);
    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        let _ = execute!(io::stdout(), PushKeyboardEnhancementFlags(flags));
    }
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    log::info!("kitty keyboard protocol: {kitty}");
    let mut input = Input::new(kitty);
    input.open_pad();
    let mut game = Game::new(args.start, args.settings, kitty);
    let result = run(&mut terminal, &mut game, &mut input, &mut audio);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            eprintln!("tspace: {e}");
            if let Some(p) = log_path {
                eprintln!("session log: {}", p.display());
            }
            ExitCode::FAILURE
        }
    }
}

fn run(terminal: &mut ratatui::DefaultTerminal, game: &mut Game, input: &mut Input, audio: &mut audio::Audio) -> io::Result<()> {
    let mut last = Instant::now();
    let mut acc = 0.0f32;
    let mut snapshot = Instant::now();
    loop {
        let frame_start = Instant::now();
        input.poll_pad();
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(k) => {
                    log::trace!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                    input.on_key(k, Instant::now());
                }
                Event::Paste(s) => input.on_paste(&s),
                Event::Resize(w, h) => log::info!("resize {w}x{h}"),
                _ => {}
            }
        }
        let now = Instant::now();
        acc += now.duration_since(last).as_secs_f32().min(0.1);
        last = now;
        while acc >= DT {
            acc -= DT;
            game.tick(input, audio);
            if game.exit {
                return Ok(());
            }
        }
        terminal.draw(|f| hud::draw(f, game))?;
        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            let w = &game.world;
            match w.player(game.me) {
                Some(p) if game.phase == Phase::Play => log::debug!(
                    "snapshot t {:.1} me {} {:?} alive {} at ({:.0},{:.0}) v {:.0} energy {:.0}/{:.0} k {} d {} players {} shots {} mines {}",
                    w.s.time,
                    game.me,
                    p.kind,
                    p.alive,
                    p.x,
                    p.y,
                    p.speed(),
                    p.energy,
                    p.max_energy,
                    p.kills,
                    p.deaths,
                    w.s.players.iter().flatten().count(),
                    w.s.shots.len(),
                    w.s.mines.len()
                ),
                _ => log::debug!("snapshot phase {:?}", game.phase),
            }
        }
        let spent = frame_start.elapsed();
        if spent < FRAME {
            let _ = event::poll(FRAME - spent)?;
        }
    }
}
