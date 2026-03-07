use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    canvas, div, px, App, Bounds, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, Pixels, Point, Render, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::Icon;
use gpui_component::Sizable;

use crate::icons::IconName;

const VOLUME_STEP: f32 = 0.1;
const SEEK_STEP_SECS: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Error,
}

struct AudioBackend {
    sink: rodio::Sink,
    _stream: rodio::OutputStream,
}

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
    progress_bounds: Bounds<Pixels>,
    error_message: Option<String>,
}

impl AudioPlayer {
    pub fn new(audio_bytes: Vec<u8>, content_type: String, cx: &mut Context<Self>) -> Self {
        let duration = Self::probe_duration(&audio_bytes, &content_type);

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
            progress_bounds: Bounds::default(),
            error_message: None,
        }
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
        sink.set_volume(self.effective_volume());

        self.backend = Some(Arc::new(Mutex::new(AudioBackend {
            sink,
            _stream: stream,
        })));
        true
    }

    fn effective_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }

    fn sync_playback_state(&mut self) {
        if self.playback_state != PlaybackState::Playing {
            return;
        }

        let Some(ref backend) = self.backend else {
            self.playback_state = PlaybackState::Stopped;
            return;
        };

        if let Ok(backend) = backend.lock() {
            let sink_empty = backend.sink.empty();
            let sink_position = backend.sink.get_pos();

            self.current_position = if sink_empty {
                self.total_duration.unwrap_or(sink_position)
            } else {
                self.clamp_to_duration(sink_position)
            };

            if sink_empty {
                self.playback_state = PlaybackState::Stopped;
            }
        }
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

        if self.has_reached_end() {
            self.current_position = Duration::ZERO;
        }

        self.backend = None;
        if !self.ensure_backend() {
            cx.notify();
            return;
        }

        let cursor = Cursor::new((*self.audio_bytes).clone());
        let Ok(source) = rodio::Decoder::new(cursor) else {
            self.error_message = Some("Failed to decode audio format".to_string());
            self.playback_state = PlaybackState::Error;
            cx.notify();
            return;
        };

        let start_position = self.current_position;

        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.append(source);

                if start_position > Duration::ZERO {
                    if let Err(error) = backend.sink.try_seek(start_position) {
                        log::warn!("Failed to seek audio on play: {error:?}");
                        self.error_message =
                            Some("Seeking is not supported for this audio".to_string());
                        self.current_position = Duration::ZERO;
                    } else {
                        self.current_position = self.clamp_to_duration(backend.sink.get_pos());
                    }
                }
            }
        }

        self.playback_state = PlaybackState::Playing;
        cx.notify();
    }

    fn pause(&mut self, cx: &mut Context<Self>) {
        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.pause();
                self.current_position = self.clamp_to_duration(backend.sink.get_pos());
            }
        }

        self.playback_state = PlaybackState::Paused;
        cx.notify();
    }

    fn resume(&mut self, cx: &mut Context<Self>) {
        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.play();
                self.current_position = self.clamp_to_duration(backend.sink.get_pos());
            }
        }

        self.playback_state = PlaybackState::Playing;
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
        cx.notify();
    }

    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.muted = !self.muted;
        self.apply_volume();
        cx.notify();
    }

    pub fn volume_up(&mut self, cx: &mut Context<Self>) {
        self.volume = (self.volume + VOLUME_STEP).min(1.0);
        if self.muted && self.volume > 0.0 {
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
        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                backend.sink.set_volume(self.effective_volume());
            }
        }
    }

    fn seek_by(&mut self, delta_secs: f32, cx: &mut Context<Self>) {
        let target = if delta_secs.is_sign_negative() {
            self.current_position
                .saturating_sub(Duration::from_secs_f32(delta_secs.abs()))
        } else {
            self.current_position + Duration::from_secs_f32(delta_secs)
        };

        self.seek_to(target, cx);
    }

    fn seek_to(&mut self, target: Duration, cx: &mut Context<Self>) {
        let Some(total_duration) = self.total_duration else {
            return;
        };

        let clamped = target.min(total_duration);

        if let Some(ref backend) = self.backend {
            if let Ok(backend) = backend.lock() {
                if let Err(error) = backend.sink.try_seek(clamped) {
                    log::warn!("Failed to seek audio: {error:?}");
                    self.error_message =
                        Some("Seeking is not supported for this audio".to_string());
                    cx.notify();
                    return;
                }

                self.current_position = self.clamp_to_duration(backend.sink.get_pos());
                self.error_message = None;
                cx.notify();
                return;
            }
        }

        self.current_position = clamped;
        self.error_message = None;
        cx.notify();
    }

    fn seek_to_pointer(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(target) = self.pointer_seek_target(position) else {
            return;
        };
        self.seek_to(target, cx);
    }

    fn pointer_seek_target(&self, position: Point<Pixels>) -> Option<Duration> {
        let total_duration = self.total_duration?;
        let width = self.progress_bounds.size.width;
        if width <= px(0.0) {
            return None;
        }

        let inner_pos = position.x - self.progress_bounds.left();
        let fraction = (inner_pos.clamp(px(0.0), width) / width).clamp(0.0, 1.0);
        Some(total_duration.mul_f32(fraction))
    }

    fn clamp_to_duration(&self, position: Duration) -> Duration {
        if let Some(total_duration) = self.total_duration {
            position.min(total_duration)
        } else {
            position
        }
    }

    fn has_reached_end(&self) -> bool {
        self.total_duration
            .map(|total| self.current_position >= total)
            .unwrap_or(false)
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

            (self.current_position.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
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

    fn status_text(&self) -> &'static str {
        match self.playback_state {
            PlaybackState::Stopped => "Ready",
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Error => "Unavailable",
        }
    }

    fn on_progress_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.seek_to_pointer(event.position, cx);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "space" => self.toggle_playback(cx),
            "left" => self.seek_by(-SEEK_STEP_SECS, cx),
            "right" => self.seek_by(SEEK_STEP_SECS, cx),
            "home" => self.seek_to(Duration::ZERO, cx),
            "end" => {
                if let Some(total) = self.total_duration {
                    self.seek_to(total, cx);
                }
            }
            "m" => self.toggle_mute(cx),
            _ => {}
        }
    }
}

impl Focusable for AudioPlayer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AudioPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_playback_state();
        if self.playback_state == PlaybackState::Playing {
            window.request_animation_frame();
        }

        let theme = cx.theme();
        let entity = cx.entity().clone();

        let is_playing = self.playback_state == PlaybackState::Playing;
        let is_stopped = self.playback_state == PlaybackState::Stopped;
        let is_error = self.playback_state == PlaybackState::Error;
        let progress = self.progress_fraction();
        let position_text = Self::format_duration(self.current_position);
        let duration_text = self
            .total_duration
            .map(Self::format_duration)
            .unwrap_or_else(|| "--:--".to_string());
        let format_label = self.audio_format_label().to_string();
        let size_text = Self::format_size(self.audio_bytes.len());
        let volume_text = if self.muted {
            "Muted".to_string()
        } else {
            format!("{}%", self.volume_percent())
        };
        let error_msg = self.error_message.clone();
        div()
            .id("audio-player")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .items_center()
            .justify_start()
            .bg(theme.muted)
            .px(px(20.0))
            .py(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .max_w(px(560.0))
                    .rounded(px(10.0))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(16.0))
                            .px(px(20.0))
                            .py(px(18.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_start()
                                    .gap(px(16.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.0))
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(theme.foreground)
                                                    .child(format_label.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(theme.muted_foreground)
                                                    .child(format!(
                                                        "{size_text} • {duration_text}"
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(if is_error {
                                                theme.danger
                                            } else {
                                                theme.muted_foreground
                                            })
                                            .child(self.status_text()),
                                    ),
                            )
                            .when_some(error_msg, |el, msg| {
                                el.child(
                                    div()
                                        .px(px(12.0))
                                        .py(px(8.0))
                                        .rounded(px(8.0))
                                        .bg(theme.danger.opacity(0.14))
                                        .border_1()
                                        .border_color(theme.danger.opacity(0.32))
                                        .text_color(theme.danger)
                                        .text_size(px(11.0))
                                        .child(msg),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .id("audio-progress-track")
                                            .relative()
                                            .w_full()
                                            .h(px(24.0))
                                            .flex()
                                            .items_center()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(Self::on_progress_mouse_down),
                                            )
                                            .child(
                                                div()
                                                    .relative()
                                                    .w_full()
                                                    .h(px(5.0))
                                                    .rounded(px(999.0))
                                                    .bg(theme.border)
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .left(px(0.0))
                                                            .top(px(0.0))
                                                            .bottom(px(0.0))
                                                            .rounded(px(999.0))
                                                            .bg(theme.accent)
                                                            .w(gpui::relative(progress)),
                                                    )
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .top(px(-4.0))
                                                            .left(gpui::relative(progress))
                                                            .ml(-px(6.0))
                                                            .size(px(12.0))
                                                            .rounded(px(999.0))
                                                            .bg(theme.background)
                                                            .border_1()
                                                            .border_color(theme.accent),
                                                    )
                                                    .child({
                                                        let entity = entity.clone();
                                                        canvas(
                                                            move |bounds, _, cx| {
                                                                entity.update(cx, |player, _| {
                                                                    if player.progress_bounds
                                                                        != bounds
                                                                    {
                                                                        player.progress_bounds =
                                                                            bounds;
                                                                    }
                                                                });
                                                            },
                                                            |_, _, _, _| {},
                                                        )
                                                        .absolute()
                                                        .size_full()
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .justify_between()
                                            .text_size(px(11.0))
                                            .text_color(theme.muted_foreground)
                                            .child(position_text)
                                            .child(duration_text),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(16.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                Button::new("audio-play-pause")
                                                    .icon(
                                                        Icon::new(if is_playing {
                                                            IconName::Pause
                                                        } else {
                                                            IconName::Play
                                                        })
                                                        .size(px(16.0)),
                                                    )
                                                    .primary()
                                                    .small()
                                                    .tooltip(if is_playing {
                                                        "Pause"
                                                    } else {
                                                        "Play"
                                                    })
                                                    .on_click({
                                                        let entity = entity.clone();
                                                        move |_, _window, cx| {
                                                            entity.update(cx, |player, cx| {
                                                                player.toggle_playback(cx);
                                                            });
                                                        }
                                                    }),
                                            )
                                            .child(
                                                Button::new("audio-stop")
                                                    .icon(
                                                        Icon::new(IconName::Square).size(px(13.0)),
                                                    )
                                                    .ghost()
                                                    .small()
                                                    .tooltip("Stop")
                                                    .disabled(is_stopped || is_error)
                                                    .on_click({
                                                        let entity = entity.clone();
                                                        move |_, _window, cx| {
                                                            entity.update(cx, |player, cx| {
                                                                player.stop(cx);
                                                            });
                                                        }
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                Button::new("audio-mute-toggle")
                                                    .icon(
                                                        Icon::new(if self.muted {
                                                            IconName::VolumeX
                                                        } else {
                                                            IconName::Volume2
                                                        })
                                                        .size(px(14.0)),
                                                    )
                                                    .ghost()
                                                    .small()
                                                    .tooltip(if self.muted {
                                                        "Unmute"
                                                    } else {
                                                        "Mute"
                                                    })
                                                    .on_click({
                                                        let entity = entity.clone();
                                                        move |_, _window, cx| {
                                                            entity.update(cx, |player, cx| {
                                                                player.toggle_mute(cx);
                                                            });
                                                        }
                                                    }),
                                            )
                                            .child(
                                                Button::new("audio-vol-down")
                                                    .label("-")
                                                    .ghost()
                                                    .small()
                                                    .tooltip("Volume down")
                                                    .on_click({
                                                        let entity = entity.clone();
                                                        move |_, _window, cx| {
                                                            entity.update(cx, |player, cx| {
                                                                player.volume_down(cx);
                                                            });
                                                        }
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .min_w(px(44.0))
                                                    .text_size(px(11.0))
                                                    .text_color(theme.muted_foreground)
                                                    .child(volume_text),
                                            )
                                            .child(
                                                Button::new("audio-vol-up")
                                                    .label("+")
                                                    .ghost()
                                                    .small()
                                                    .tooltip("Volume up")
                                                    .on_click({
                                                        let entity = entity.clone();
                                                        move |_, _window, cx| {
                                                            entity.update(cx, |player, cx| {
                                                                player.volume_up(cx);
                                                            });
                                                        }
                                                    }),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
