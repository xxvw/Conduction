//! conduction-audio-probe-mix — 2 デッキミキサーの動作確認 CLI。
//!
//! ```text
//! usage: conduction-audio-probe-mix <path_a> <path_b>
//! ```
//!
//! 対話コマンド（1 行 1 コマンド、Enter で送信）:
//!
//! ```text
//!   pa            Deck A の再生 / 一時停止トグル
//!   pb            Deck B の再生 / 一時停止トグル
//!   cf <-1..1>    クロスフェーダー位置（-1 = Full A, 0 = Center, +1 = Full B）
//!   va <0..2>     Deck A チャンネルボリューム
//!   vb <0..2>     Deck B チャンネルボリューム
//!   m  <0..2>     マスターボリューム
//!   s             状態表示
//!   q             終了
//! ```

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use conduction_audio::{DeckId, Mixer, OutputDevice, TempoRange};

fn main() -> anyhow::Result<()> {
    init_tracing();

    let mut args = std::env::args().skip(1);
    let (Some(path_a), Some(path_b)) = (args.next(), args.next()) else {
        eprintln!("usage: conduction-audio-probe-mix <path_a> <path_b>");
        std::process::exit(2);
    };
    let path_a = PathBuf::from(path_a);
    let path_b = PathBuf::from(path_b);

    let device = OutputDevice::open_default()?;
    let mut mixer = Mixer::new(&device, None)?;

    mixer.deck_a().load(&device, None, &path_a)?;
    mixer.deck_b().load(&device, None, &path_b)?;

    // 初期: Deck A 側に振り切り、両チャンネルボリュームは 1.0
    mixer.set_crossfader(-1.0);

    println!("[probe-mix] Deck A: {}", path_a.display());
    println!("[probe-mix] Deck B: {}", path_b.display());
    print_help();
    print_status(&mut mixer);

    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        prompt();
        line.clear();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        match handle(input, &mut mixer) {
            Action::Continue => {}
            Action::Quit => break,
        }
    }

    println!("[probe-mix] bye.");
    Ok(())
}

enum Action {
    Continue,
    Quit,
}

fn handle(input: &str, mixer: &mut Mixer) -> Action {
    let mut parts = input.split_whitespace();
    let Some(cmd) = parts.next() else {
        return Action::Continue;
    };

    match cmd {
        "q" | "quit" | "exit" => return Action::Quit,
        "h" | "help" | "?" => print_help(),
        "s" | "status" => print_status(mixer),
        "pa" => toggle(mixer, DeckId::A),
        "pb" => toggle(mixer, DeckId::B),
        "cf" => match parse_f32(parts.next()) {
            Some(v) => {
                mixer.set_crossfader(v);
                println!("[probe-mix] crossfader = {:.3}", mixer.crossfader());
            }
            None => println!("[probe-mix] cf requires a value in [-1, 1]"),
        },
        "va" => match parse_f32(parts.next()) {
            Some(v) => {
                mixer.set_channel_volume(DeckId::A, v);
                println!(
                    "[probe-mix] deck A ch vol = {:.3} (eff {:.3})",
                    mixer.deck_a().channel_volume(),
                    mixer.deck_a().effective_volume(),
                );
            }
            None => println!("[probe-mix] va requires a value in [0, 2]"),
        },
        "vb" => match parse_f32(parts.next()) {
            Some(v) => {
                mixer.set_channel_volume(DeckId::B, v);
                println!(
                    "[probe-mix] deck B ch vol = {:.3} (eff {:.3})",
                    mixer.deck_b().channel_volume(),
                    mixer.deck_b().effective_volume(),
                );
            }
            None => println!("[probe-mix] vb requires a value in [0, 2]"),
        },
        "m" | "master" => match parse_f32(parts.next()) {
            Some(v) => {
                mixer.set_master_volume(v);
                println!("[probe-mix] master = {:.3}", mixer.master_volume());
            }
            None => println!("[probe-mix] m requires a value in [0, 2]"),
        },
        "ta" => apply_tempo(mixer, DeckId::A, parts.next()),
        "tb" => apply_tempo(mixer, DeckId::B, parts.next()),
        "ra" => apply_range(mixer, DeckId::A, parts.next()),
        "rb" => apply_range(mixer, DeckId::B, parts.next()),
        other => println!("[probe-mix] unknown command: {other} (type 'h' for help)"),
    }
    Action::Continue
}

fn apply_tempo(mixer: &mut Mixer, id: DeckId, arg: Option<&str>) {
    let Some(v) = parse_f32(arg) else {
        println!("[probe-mix] t{{a,b}} requires a value in [-1, 1]");
        return;
    };
    let deck = mixer.deck(id);
    deck.set_tempo_adjust(v);
    println!(
        "[probe-mix] deck {id:?} tempo_adjust = {:+.3}  speed = {:.4}x  (range ±{}%)",
        deck.tempo_adjust(),
        deck.playback_speed(),
        deck.tempo_range().as_percent(),
    );
}

fn apply_range(mixer: &mut Mixer, id: DeckId, arg: Option<&str>) {
    let range = match arg {
        Some("6") => TempoRange::Six,
        Some("10") => TempoRange::Ten,
        Some("16") => TempoRange::Sixteen,
        _ => {
            println!("[probe-mix] r{{a,b}} requires one of: 6 | 10 | 16");
            return;
        }
    };
    let deck = mixer.deck(id);
    deck.set_tempo_range(range);
    println!(
        "[probe-mix] deck {id:?} tempo range = ±{}%  speed = {:.4}x",
        deck.tempo_range().as_percent(),
        deck.playback_speed(),
    );
}

fn toggle(mixer: &mut Mixer, id: DeckId) {
    let deck = mixer.deck(id);
    if deck.is_playing() {
        deck.pause();
        println!("[probe-mix] deck {id:?} paused");
    } else {
        deck.play();
        println!("[probe-mix] deck {id:?} playing");
    }
}

fn parse_f32(s: Option<&str>) -> Option<f32> {
    s.and_then(|s| s.parse().ok())
}

fn print_help() {
    println!(
        "\nCommands:
  pa | pb        toggle play/pause
  cf <-1..1>     crossfader (-1 = full A, +1 = full B)
  va <0..2>      deck A channel volume
  vb <0..2>      deck B channel volume
  ta <-1..1>     deck A tempo adjust (fader position)
  tb <-1..1>     deck B tempo adjust (fader position)
  ra <6|10|16>   deck A tempo range (±%)
  rb <6|10|16>   deck B tempo range (±%)
  m  <0..2>      master volume
  s              show status
  h              show this help
  q              quit\n"
    );
}

fn print_status(mixer: &mut Mixer) {
    let cf = mixer.crossfader();
    let master = mixer.master_volume();

    let a = DeckSnapshot::of(mixer, DeckId::A);
    let b = DeckSnapshot::of(mixer, DeckId::B);

    println!(
        "\n--- status ---
  crossfader: {cf:+.3}     master: {master:.3}
  Deck A [{s}]  ch {ch:.2}  eff {eff:.2}  spd {spd:.4}x (±{rng}%, adj {adj:+.2})  pos {pos:6.2}/{dur:6.2}s",
        s = a.state,
        ch = a.ch_vol,
        eff = a.eff_vol,
        spd = a.speed,
        rng = a.range_pct,
        adj = a.tempo_adjust,
        pos = a.pos,
        dur = a.dur,
    );
    println!(
        "  Deck B [{s}]  ch {ch:.2}  eff {eff:.2}  spd {spd:.4}x (±{rng}%, adj {adj:+.2})  pos {pos:6.2}/{dur:6.2}s\n",
        s = b.state,
        ch = b.ch_vol,
        eff = b.eff_vol,
        spd = b.speed,
        rng = b.range_pct,
        adj = b.tempo_adjust,
        pos = b.pos,
        dur = b.dur,
    );
}

struct DeckSnapshot {
    state: &'static str,
    ch_vol: f32,
    eff_vol: f32,
    speed: f32,
    range_pct: u8,
    tempo_adjust: f32,
    pos: f64,
    dur: f64,
}

impl DeckSnapshot {
    fn of(mixer: &mut Mixer, id: DeckId) -> Self {
        let state = state_str(mixer, id);
        let deck = mixer.deck(id);
        Self {
            state,
            ch_vol: deck.channel_volume(),
            eff_vol: deck.effective_volume(),
            speed: deck.playback_speed(),
            range_pct: deck.tempo_range().as_percent(),
            tempo_adjust: deck.tempo_adjust(),
            pos: deck.position().as_secs_f64(),
            dur: deck.duration().map(|d| d.as_secs_f64()).unwrap_or(0.0),
        }
    }
}

fn state_str(mixer: &mut Mixer, id: DeckId) -> &'static str {
    let deck = mixer.deck(id);
    if deck.is_finished() {
        "end "
    } else if deck.is_playing() {
        "play"
    } else if deck.is_paused() {
        "paus"
    } else {
        "idle"
    }
}

fn prompt() {
    print!("> ");
    let _ = io::stdout().flush();
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
