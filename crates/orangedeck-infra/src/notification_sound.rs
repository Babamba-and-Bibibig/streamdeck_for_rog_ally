//! A short original chime streamed to the Linux audio server, with no sound asset or private file.
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};

static PLAYING: AtomicBool = AtomicBool::new(false);

struct PlayingGuard;
impl Drop for PlayingGuard {
    fn drop(&mut self) {
        PLAYING.store(false, Ordering::Release);
    }
}

pub fn play_notification_sound() {
    // Coalesce simultaneous notices and keep at most one audio worker/player alive.
    if PLAYING.swap(true, Ordering::AcqRel) {
        return;
    }
    let guard = PlayingGuard;
    let worker = std::thread::Builder::new()
        .name("orangedeck-chime".to_owned())
        .spawn(move || {
            let _guard = guard;
            for (program, args) in [
                (
                    "pw-play",
                    vec![
                        "--raw",
                        "--format",
                        "s16",
                        "--rate",
                        "24000",
                        "--channels",
                        "1",
                        "--media-role",
                        "Notification",
                        "-",
                    ],
                ),
                (
                    "paplay",
                    vec![
                        "--raw",
                        "--format=s16le",
                        "--rate=24000",
                        "--channels=1",
                        "--client-name=OrangeDeck",
                    ],
                ),
            ] {
                let Ok(mut child) = Command::new(program)
                    .args(args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                else {
                    continue;
                };
                if let Some(mut input) = child.stdin.take() {
                    let _ = input.write_all(&chime());
                }
                if child.wait().is_ok_and(|status| status.success()) {
                    return;
                }
            }
            tracing::warn!(
                "Notification sound could not be played; visual alerts remain available"
            );
        });
    if worker.is_err() {
        tracing::warn!("Notification sound worker could not start");
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn chime() -> Vec<u8> {
    let mut pcm = Vec::with_capacity(12_000);
    for sample in 0..6_000 {
        let time = sample as f32 / 24_000.0;
        let envelope = (time * 100.0).min(1.0) * (1.0 - time / 0.25).max(0.0).powi(2);
        let wave = (time * 660.0 * std::f32::consts::TAU).sin()
            + 0.45 * (time * 990.0 * std::f32::consts::TAU).sin();
        pcm.extend_from_slice(&((wave * envelope * 5_000.0) as i16).to_le_bytes());
    }
    pcm
}
