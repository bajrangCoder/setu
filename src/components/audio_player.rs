use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    div, px, App, Context, ElementId, FocusHandle, Focusable, IntoElement, Render, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::Icon;
use gpui_component::Sizable;

use crate::icons::IconName;

const WAVEFORM_BARS: usize = 48;
const VOLUME_STEP: f32 = 0.1;
const TICKER_INTERVAL_MS: u64 = 100;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Error,
}

#[allow(dead_code)]
struct AudioBackend {
    sink: rodio::Sink,
    _stream: rodio::OutputStream,
}

#[allow(dead_code)]
pub struct AudioPlayer {
    audio_bytes: Arc<Vec<u8>>,
    content_type: String,
    playback_state: PlaybackState,
    volume: f32,
    muted: bool,
    current_position: Duration,
    total_duration: Option<Duration>,
    focus_handle: FocusHandle,
    backend: Option<Arc<Mutex<AudioBackend>>>,
    _ticker: Option<gpui::Task<()>>,
    waveform: Arc<Vec<f32>>,
    error_message: Option<String>,
}

impl AudioPlayer {
    pub fn new(audio_bytes: Vec<u8>, content_type: String, cx: &mut Context<Self>) -> Self {
        let duration = Self::probe_duration(&audio_bytes, &content_type);
        let waveform = Arc::new(Self::generate_waveform(&audio_bytes, WAVEFORM_BARS));

        Self {
            audio_bytes: Arc::new(audio_bytes),
            content_type,
            playback_state: PlaybackState::Stopped,
            volume: 0.8,
            muted: false,
            current_position: Duration::ZERO,
            total_duration: duration,
            focus_handle: cx.focus_handle(),
            backend: None,
            _ticker: None,
            waveform,
            error_message: None,
        }
    }

    fn generate_waveform(bytes: &[u8], num_bars: usize) -> Vec<f32> {
        if bytes.is_empty() || num_bars == 0 {
            return vec![0.15; num_bars];
        }

        let chunk_size = bytes.len() / num_bars;
        if chunk_size == 0 {
            return vec![0.15; num_bars];
        }

        let mut bars: Vec<f32> = Vec::with_capacity(num_bars);
        for i in 0..num_bars {
            let start = i * chunk_size;
            let end = ((i + 1) * chunk_size).min(bytes.len());

            let mut sum: u64 = 0;
            let mut count = 0u64;
            for &b in &bytes[start..end] {
                let centered = (b as i16 - 128).unsigned_abs() as u64;
                sum += centered;
                count += 1;
            }

            let avg = if count > 0 {
                sum as f32 / count as f32
            } else {
                0.0
            };
            let normalized = (avg / 128.0).clamp(0.0, 1.0);
            let scaled = 0.08 + normalized * 0.92;
            bars.push(scaled);
        }

        bars
    }

    fn probe_duration(bytes: &[u8], _content_type: &str) -> Option<Duration> {
        use rodio::Source;
        let cursor = Cursor::new(bytes.to_vec());
        let source = rodio::Decoder::new(cursor).ok()?;
        source.total_duration()
    }

    fn ensure_backend(&mut self) -> bool {
        if self.backend.is_some() {
            return true;
        }

        let Ok(stream) = rodio::OutputStreamBuilder::open_default_stream() else {
            log::error!("Failed to open audio output stream");
            self.error_message = Some("No audio output device found".to_string());
            self.playback_state = PlaybackState::Error;
            return false;
        };
        let sink = rodio::Sink::connect_new(stream.mixer());

        let effective_volume = if self.muted { 0.0 } else { self.volume };
        sink.set_volume(effective_volume);

        self.backend = Some(Arc::new(Mutex::new(AudioBackend {
            sink,
            _stream: stream,
        })));
        true
    }

    fn start_ticker(&mut self, cx: &mut Context<Self>) {
        let entity = cx.entity().clone();
        self._ticker = Some(cx.spawn(async move |_this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(TICKER_INTERVAL_MS))
                .await;

            let should_continue = cx.update(|app| entity.update(app, |player, cx| player.tick(cx)));

            match should_continue {
                Ok(true) => {}
                _ => break,
            }
        }));
    }

    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        if self.playback_state != PlaybackState::Playing {
            return false;
        }

        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                if backend.sink.empty() {
                    self.playback_state = PlaybackState::Stopped;
                    self.current_position = Duration::ZERO;
                    cx.notify();
                    return false;
                }

                self.current_position += Duration::from_millis(TICKER_INTERVAL_MS);

                if let Some(total) = self.total_duration {
                    if self.current_position > total {
                        self.current_position = total;
                    }
                }
            }
        }

        cx.notify();
        true
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        match self.playback_state {
            PlaybackState::Stopped | PlaybackState::Error => self.play(cx),
            PlaybackState::Playing => self.pause(cx),
            PlaybackState::Paused => self.resume(cx),
        }
    }

    fn play(&mut self, cx: &mut Context<Self>) {
        self.error_message = None;
        self.backend = None;
        if !self.ensure_backend() {
            cx.notify();
            return;
        }

        let cursor = Cursor::new((*self.audio_bytes).clone());
        if let Ok(source) = rodio::Decoder::new(cursor) {
            if let Some(ref backend) = self.backend {
                if let Ok(backend) = backend.lock() {
                    backend.sink.append(source);
                }
            }
        } else {
            self.error_message = Some("Failed to decode audio format".to_string());
            self.playback_state = PlaybackState::Error;
            cx.notify();
            return;
        }

        self.playback_state = PlaybackState::Playing;
        self.current_position = Duration::ZERO;
        self.start_ticker(cx);
        cx.notify();
    }

    fn pause(&mut self, cx: &mut Context<Self>) {
        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.pause();
            }
        }
        self.playback_state = PlaybackState::Paused;
        cx.notify();
    }

    fn resume(&mut self, cx: &mut Context<Self>) {
        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.play();
            }
        }
        self.playback_state = PlaybackState::Playing;
        self.start_ticker(cx);
        cx.notify();
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.stop();
            }
        }
        self.backend = None;
        self.playback_state = PlaybackState::Stopped;
        self.current_position = Duration::ZERO;
        self._ticker = None;
        cx.notify();
    }

    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.muted = !self.muted;
        self.apply_volume();
        cx.notify();
    }

    pub fn volume_up(&mut self, cx: &mut Context<Self>) {
        self.volume = (self.volume + VOLUME_STEP).min(1.0);
        if self.muted {
            self.muted = false;
        }
        self.apply_volume();
        cx.notify();
    }

    pub fn volume_down(&mut self, cx: &mut Context<Self>) {
        self.volume = (self.volume - VOLUME_STEP).max(0.0);
        self.apply_volume();
        cx.notify();
    }

    fn apply_volume(&self) {
        let effective = if self.muted { 0.0 } else { self.volume };
        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.set_volume(effective);
            }
        }
    }

    fn format_duration(d: Duration) -> String {
        let total_secs = d.as_secs();
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }

    fn progress_fraction(&self) -> f32 {
        if let Some(total) = self.total_duration {
            if total.as_millis() == 0 {
                return 0.0;
            }
            (self.current_position.as_millis() as f32 / total.as_millis() as f32).min(1.0)
        } else {
            0.0
        }
    }

    fn audio_format_label(&self) -> &str {
        let ct = self.content_type.to_lowercase();
        if ct.contains("mp3") || ct.contains("mpeg") {
            "MP3"
        } else if ct.contains("wav") {
            "WAV"
        } else if ct.contains("ogg") {
            "OGG"
        } else if ct.contains("flac") {
            "FLAC"
        } else if ct.contains("aac") {
            "AAC"
        } else if ct.contains("webm") {
            "WebM"
        } else {
            "Audio"
        }
    }

    fn format_size(bytes: usize) -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }

    fn volume_percent(&self) -> u32 {
        (self.volume * 100.0).round() as u32
    }
}

impl Focusable for AudioPlayer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AudioPlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let this_play = cx.entity().clone();
        let this_stop = cx.entity().clone();
        let this_mute = cx.entity().clone();
        let this_vol_up = cx.entity().clone();
        let this_vol_down = cx.entity().clone();

        let is_playing = self.playback_state == PlaybackState::Playing;
        let is_paused = self.playback_state == PlaybackState::Paused;
        let is_stopped = self.playback_state == PlaybackState::Stopped;
        let is_error = self.playback_state == PlaybackState::Error;
        let is_active = is_playing || is_paused;
        let progress = self.progress_fraction();
        let position_text = Self::format_duration(self.current_position);
        let duration_text = self
            .total_duration
            .map(Self::format_duration)
            .unwrap_or_else(|| "--:--".to_string());
        let format_label = self.audio_format_label().to_string();
        let size_text = Self::format_size(self.audio_bytes.len());
        let muted = self.muted;
        let volume_pct = self.volume_percent();

        let waveform = self.waveform.clone();
        let error_msg = self.error_message.clone();

        div()
            .id("audio-player")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .items_center()
            .justify_center()
            .bg(theme.muted)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(460.0))
                    .rounded(px(16.0))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    // Waveform visualization
                    .child(
                        div()
                            .id("audio-waveform-area")
                            .w_full()
                            .h(px(120.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(theme.secondary)
                            .child(
                                div()
                                    .flex()
                                    .items_end()
                                    .justify_center()
                                    .gap(px(2.0))
                                    .h(px(80.0))
                                    .px(px(16.0))
                                    .children(
                                        waveform
                                            .iter()
                                            .enumerate()
                                            .map(|(i, &height_frac)| {
                                                let bar_progress = i as f32 / waveform.len() as f32;
                                                let is_played = bar_progress <= progress;
                                                let bar_color = if is_played && is_active {
                                                    theme.accent
                                                } else {
                                                    theme.muted_foreground
                                                };
                                                let opacity = if is_played && is_active {
                                                    0.95
                                                } else if is_active {
                                                    0.3
                                                } else {
                                                    0.25
                                                };
                                                let max_h = 80.0_f32;
                                                let bar_h = max_h * height_frac;

                                                div()
                                                    .id(ElementId::NamedInteger(
                                                        "waveform-bar".into(),
                                                        i as u64,
                                                    ))
                                                    .w(px(3.0))
                                                    .h(px(bar_h))
                                                    .rounded(px(1.5))
                                                    .bg(bar_color)
                                                    .opacity(opacity)
                                                    .into_any_element()
                                            })
                                            .collect::<Vec<_>>(),
                                    ),
                            ),
                    )
                    // Progress bar
                    .child(
                        div()
                            .id("audio-progress-track")
                            .w_full()
                            .h(px(4.0))
                            .bg(theme.border)
                            .child(div().h_full().bg(theme.accent).w(gpui::relative(progress))),
                    )
                    // Controls area
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .px(px(24.0))
                            .py(px(16.0))
                            // Time row
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_color(if is_active {
                                                theme.foreground
                                            } else {
                                                theme.muted_foreground
                                            })
                                            .text_size(px(12.0))
                                            .child(position_text),
                                    )
                                    .child(
                                        div()
                                            .text_color(theme.muted_foreground)
                                            .text_size(px(12.0))
                                            .child(duration_text),
                                    ),
                            )
                            // Error message
                            .when_some(error_msg, |el, msg| {
                                el.child(
                                    div()
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .rounded(px(6.0))
                                        .bg(theme.danger)
                                        .text_color(gpui::white())
                                        .text_size(px(11.0))
                                        .child(msg),
                                )
                            })
                            // Playback controls
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    // Left: Volume controls
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(2.0))
                                            .child(
                                                Button::new("audio-vol-down")
                                                    .icon(
                                                        Icon::new(IconName::ChevronLeft)
                                                            .size(px(12.0)),
                                                    )
                                                    .ghost()
                                                    .xsmall()
                                                    .tooltip("Volume Down")
                                                    .on_click(move |_, _window, cx| {
                                                        this_vol_down.update(cx, |player, cx| {
                                                            player.volume_down(cx);
                                                        });
                                                    }),
                                            )
                                            .child(
                                                Button::new("audio-mute-toggle")
                                                    .icon(
                                                        Icon::new(if muted {
                                                            IconName::VolumeX
                                                        } else {
                                                            IconName::Volume2
                                                        })
                                                        .size(px(14.0)),
                                                    )
                                                    .ghost()
                                                    .xsmall()
                                                    .tooltip(if muted { "Unmute" } else { "Mute" })
                                                    .on_click(move |_, _window, cx| {
                                                        this_mute.update(cx, |player, cx| {
                                                            player.toggle_mute(cx);
                                                        });
                                                    }),
                                            )
                                            .child(
                                                Button::new("audio-vol-up")
                                                    .icon(
                                                        Icon::new(IconName::ChevronRight)
                                                            .size(px(12.0)),
                                                    )
                                                    .ghost()
                                                    .xsmall()
                                                    .tooltip("Volume Up")
                                                    .on_click(move |_, _window, cx| {
                                                        this_vol_up.update(cx, |player, cx| {
                                                            player.volume_up(cx);
                                                        });
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_color(theme.muted_foreground)
                                                    .text_size(px(10.0))
                                                    .ml(px(4.0))
                                                    .child(if muted {
                                                        "Muted".to_string()
                                                    } else {
                                                        format!("{}%", volume_pct)
                                                    }),
                                            ),
                                    )
                                    // Center: Play/Pause & Stop
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                Button::new("audio-stop")
                                                    .icon(
                                                        Icon::new(IconName::Square).size(px(13.0)),
                                                    )
                                                    .ghost()
                                                    .small()
                                                    .tooltip("Stop")
                                                    .when(is_stopped || is_error, |btn| {
                                                        btn.disabled(true)
                                                    })
                                                    .on_click(move |_, _window, cx| {
                                                        this_stop.update(cx, |player, cx| {
                                                            player.stop(cx);
                                                        });
                                                    }),
                                            )
                                            .child(
                                                Button::new("audio-play-pause")
                                                    .icon(
                                                        Icon::new(if is_playing {
                                                            IconName::Pause
                                                        } else {
                                                            IconName::Play
                                                        })
                                                        .size(px(20.0)),
                                                    )
                                                    .primary()
                                                    .tooltip(if is_playing {
                                                        "Pause"
                                                    } else {
                                                        "Play"
                                                    })
                                                    .on_click(move |_, _window, cx| {
                                                        this_play.update(cx, |player, cx| {
                                                            player.toggle_playback(cx);
                                                        });
                                                    }),
                                            ),
                                    )
                                    // Right: Format badge
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(3.0))
                                                    .rounded(px(4.0))
                                                    .bg(theme.secondary)
                                                    .text_color(theme.secondary_foreground)
                                                    .text_size(px(10.0))
                                                    .child(format_label),
                                            )
                                            .child(
                                                div()
                                                    .text_color(theme.muted_foreground)
                                                    .text_size(px(10.0))
                                                    .child(size_text),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.stop();
            }
        }
    }
}
