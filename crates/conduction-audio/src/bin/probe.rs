//! conduction-audio-probe — 単一デッキでの動作確認 CLI。
//!
//! ```text
//! usage: conduction-audio-probe <path/to/audio> [channel_volume]
//! ```
//!
//! Enter キーで停止。デフォルトチャンネルボリュームは 1.0。

use std::io::{self, BufRead};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use conduction_audio::{Deck, DeckId, Mixer, OutputDevice};

fn main() -> anyhow::Result<()> {
    init_tracing();

    let mut args = std::env::args().skip(1);
    let Some(path_arg) = args.next() else {
        eprintln!("usage: conduction-audio-probe <path/to/audio> [channel_volume]");
        std::process::exit(2);
    };
    let path = PathBuf::from(path_arg);
    let volume: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);

    let device = OutputDevice::open_default()?;
    // 単一デッキ用途だが、Mixer 経由のほうが実効ボリューム計算を統一できる。
    let mut mixer = Mixer::new(&device, None)?;
    mixer.set_channel_volume(DeckId::A, volume);

    let deck_a: &mut Deck = mixer.deck_a();
    deck_a.load(&device, None, &path)?;
    deck_a.play();

    println!("[probe] playing: {}", path.display());
    if let Some(d) = deck_a.duration() {
        println!("[probe] duration: {:.2}s", d.as_secs_f64());
    } else {
        println!("[probe] duration: unknown");
    }
    println!("[probe] channel volume: {:.2}", volume);
    println!("[probe] press Enter to stop, or wait for completion.");

    let (tx, rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let stdin = io::stdin();
        let _ = stdin.lock().lines().next();
        let _ = tx.send(());
    });

    let mut last_report = std::time::Instant::now();
    loop {
        let deck = mixer.deck_a();
        if deck.is_finished() {
            println!("[probe] finished.");
            break;
        }
        if rx.try_recv().is_ok() {
            deck.stop();
            println!("[probe] stopped by user.");
            break;
        }
        if last_report.elapsed() >= Duration::from_secs(1) {
            let pos = deck.position();
            match deck.duration() {
                Some(d) => println!(
                    "[probe] pos: {:6.2}s / {:6.2}s",
                    pos.as_secs_f64(),
                    d.as_secs_f64()
                ),
                None => println!("[probe] pos: {:6.2}s", pos.as_secs_f64()),
            }
            last_report = std::time::Instant::now();
        }
        thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
