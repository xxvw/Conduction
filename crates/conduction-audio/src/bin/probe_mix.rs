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

use conduction_audio::{DeckId, Mixer, OutputDevice};

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
    let mut mixer = Mixer::new(&device)?;

    mixer.deck_a().load(&device, &path_a)?;
    mixer.deck_b().load(&device, &path_b)?;

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
        other => println!("[probe-mix] unknown command: {other} (type 'h' for help)"),
    }
    Action::Continue
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
  m  <0..2>      master volume
  s              show status
  h              show this help
  q              quit\n"
    );
}

fn print_status(mixer: &mut Mixer) {
    let cf = mixer.crossfader();
    let master = mixer.master_volume();
    let a_ch = mixer.deck_a().channel_volume();
    let a_eff = mixer.deck_a().effective_volume();
    let a_state = state_str(mixer, DeckId::A);
    let a_pos = mixer.deck_a().position().as_secs_f64();
    let a_dur = mixer
        .deck_a()
        .duration()
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let b_ch = mixer.deck_b().channel_volume();
    let b_eff = mixer.deck_b().effective_volume();
    let b_state = state_str(mixer, DeckId::B);
    let b_pos = mixer.deck_b().position().as_secs_f64();
    let b_dur = mixer
        .deck_b()
        .duration()
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    println!(
        "\n--- status ---
  crossfader: {cf:+.3}     master: {master:.3}
  Deck A [{a_state}]  ch {a_ch:.2}  eff {a_eff:.2}  pos {a_pos:6.2}/{a_dur:6.2}s
  Deck B [{b_state}]  ch {b_ch:.2}  eff {b_eff:.2}  pos {b_pos:6.2}/{b_dur:6.2}s\n"
    );
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
