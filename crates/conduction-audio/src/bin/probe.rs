//! conduction-audio-probe — 最小再生の動作確認 CLI。
//!
//! ```text
//! usage: conduction-audio-probe <path/to/audio> [gain]
//! ```
//!
//! Enter キーで停止。デフォルトゲインは 1.0。

use std::io::{self, BufRead};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use conduction_audio::Player;

fn main() -> anyhow::Result<()> {
    init_tracing();

    let mut args = std::env::args().skip(1);
    let Some(path_arg) = args.next() else {
        eprintln!("usage: conduction-audio-probe <path/to/audio> [gain]");
        std::process::exit(2);
    };
    let path = PathBuf::from(path_arg);
    let gain: f32 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    let mut player = Player::new()?;
    player.load(&path)?;
    player.set_gain(gain);
    player.play();

    println!("[probe] playing: {}", path.display());
    if let Some(d) = player.duration() {
        println!("[probe] duration: {:.2}s", d.as_secs_f64());
    } else {
        println!("[probe] duration: unknown");
    }
    println!("[probe] gain: {:.2}", gain);
    println!("[probe] press Enter to stop, or wait for completion.");

    // stdin を別スレッドで待ち受け、メインはポーリング。
    let (tx, rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let stdin = io::stdin();
        let _ = stdin.lock().lines().next();
        let _ = tx.send(());
    });

    let mut last_report = std::time::Instant::now();
    loop {
        if player.is_finished() {
            println!("[probe] finished.");
            break;
        }
        if rx.try_recv().is_ok() {
            player.stop();
            println!("[probe] stopped by user.");
            break;
        }
        if last_report.elapsed() >= Duration::from_secs(1) {
            let pos = player.position();
            match player.duration() {
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
