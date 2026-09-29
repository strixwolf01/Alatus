//! Touchpad edge gestures service for volume, brightness, and media.
//! Reference implementation from Ayuz (KDE Plasma first).

use evdev::{AbsoluteAxisCode, Device, EventSummary, KeyCode};
use std::time::Duration;
use tokio::sync::watch;

const EDGE_PERCENT: f64 = 0.05;
const STEP_THRESHOLD: i32 = 250;

#[zbus::proxy(
    interface = "org.kde.Solid.PowerManagement.Actions.BrightnessControl",
    default_service = "org.kde.Solid.PowerManagement",
    default_path = "/org/kde/Solid/PowerManagement/Actions/BrightnessControl"
)]
pub trait BrightnessControl {
    fn brightness(&self) -> zbus::Result<i32>;
    #[zbus(name = "brightnessMax")]
    fn brightness_max(&self) -> zbus::Result<i32>;
    #[zbus(name = "setBrightness")]
    fn set_brightness(&self, value: i32) -> zbus::Result<()>;
}

async fn adjust_kde_brightness(delta_pct: i32) -> Result<(), String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = BrightnessControlProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;
    let max = proxy.brightness_max().await.map_err(|e| e.to_string())?;
    let cur = proxy.brightness().await.map_err(|e| e.to_string())?;
    let step = (max as f64 * (delta_pct as f64) / 100.0).round() as i32;
    let next = (cur + step).clamp(0, max);
    proxy.set_brightness(next).await.map_err(|e| e.to_string())
}

enum GestureState {
    Idle,
    Classifying { x: Option<i32>, y: Option<i32> },
    LeftEdge { last_y: i32 },
    RightEdge { last_y: i32 },
    TopEdge { start_x: i32, done: bool },
    Other,
}

fn try_classify(state: &mut GestureState, left: i32, right: i32, top: i32) {
    if let GestureState::Classifying {
        x: Some(x),
        y: Some(y),
    } = *state
    {
        *state = if x < left {
            GestureState::LeftEdge { last_y: y }
        } else if x > right {
            GestureState::RightEdge { last_y: y }
        } else if y < top {
            GestureState::TopEdge {
                start_x: x,
                done: false,
            }
        } else {
            GestureState::Other
        };
    }
}

async fn run_action(program: &str, args: &[&str]) {
    let _ = tokio::process::Command::new(program)
        .args(args)
        .status()
        .await;
}

pub fn find_touchpad() -> Option<Device> {
    for (_, device) in evdev::enumerate() {
        let name = device.name().unwrap_or_default().to_lowercase();
        if !name.contains("touchpad") && !name.contains("touch") {
            continue;
        }
        if let Some(axes) = device.supported_absolute_axes() {
            let has_x = axes.contains(AbsoluteAxisCode::ABS_X)
                || axes.contains(AbsoluteAxisCode::ABS_MT_POSITION_X);
            let has_y = axes.contains(AbsoluteAxisCode::ABS_Y)
                || axes.contains(AbsoluteAxisCode::ABS_MT_POSITION_Y);
            if has_x && has_y {
                return Some(device);
            }
        }
    }
    None
}

pub fn touchpad_bounds(device: &Device) -> Option<(i32, i32)> {
    let abs_state = device.get_abs_state().ok()?;
    let x_max = {
        let mt = abs_state[AbsoluteAxisCode::ABS_MT_POSITION_X.0 as usize].maximum;
        if mt > 0 {
            mt
        } else {
            abs_state[AbsoluteAxisCode::ABS_X.0 as usize].maximum
        }
    };
    let y_max = {
        let mt = abs_state[AbsoluteAxisCode::ABS_MT_POSITION_Y.0 as usize].maximum;
        if mt > 0 {
            mt
        } else {
            abs_state[AbsoluteAxisCode::ABS_Y.0 as usize].maximum
        }
    };
    if x_max <= 0 || y_max <= 0 {
        return None;
    }
    Some((x_max, y_max))
}

pub async fn run_gestures_listener(mut shutdown: watch::Receiver<bool>) {
    tracing::info!("Starting Touchpad Gestures service...");

    loop {
        if *shutdown.borrow() {
            break;
        }

        let device = match find_touchpad() {
            Some(d) => d,
            None => {
                tracing::debug!("Touchpad not found or not accessible yet. Retrying in 10s...");
                tokio::select! {
                    _ = shutdown.changed() => break,
                    _ = tokio::time::sleep(Duration::from_secs(10)) => continue,
                }
            }
        };

        let (x_max, y_max) = match touchpad_bounds(&device) {
            Some(b) => b,
            None => {
                tokio::select! {
                    _ = shutdown.changed() => break,
                    _ = tokio::time::sleep(Duration::from_secs(10)) => continue,
                }
            }
        };

        let left_bound = (x_max as f64 * EDGE_PERCENT) as i32;
        let right_bound = (x_max as f64 * (1.0 - EDGE_PERCENT)) as i32;
        let top_bound = (y_max as f64 * EDGE_PERCENT) as i32;

        let mut stream = match device.into_event_stream() {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!("Unable to open touchpad event stream: {e}. Retrying in 10s...");
                tokio::select! {
                    _ = shutdown.changed() => break,
                    _ = tokio::time::sleep(Duration::from_secs(10)) => continue,
                }
            }
        };

        tracing::info!(
            "Touchpad gestures active (X: 0..{}, Y: 0..{}, Left: <{}, Right: >{}, Top: <{})",
            x_max,
            y_max,
            left_bound,
            right_bound,
            top_bound
        );

        let mut state = GestureState::Idle;

        loop {
            let event = tokio::select! {
                _ = shutdown.changed() => return,
                res = stream.next_event() => match res {
                    Ok(ev) => ev,
                    Err(e) => {
                        tracing::warn!("Touchpad event stream disconnected: {e}");
                        break;
                    }
                }
            };

            match event.destructure() {
                EventSummary::Key(_, KeyCode::BTN_TOUCH, val) => {
                    if val == 1 {
                        state = GestureState::Classifying { x: None, y: None };
                    } else {
                        state = GestureState::Idle;
                    }
                }
                EventSummary::AbsoluteAxis(
                    _,
                    AbsoluteAxisCode::ABS_X | AbsoluteAxisCode::ABS_MT_POSITION_X,
                    val,
                ) => {
                    if let GestureState::Classifying { x, .. } = &mut state {
                        *x = Some(val);
                        try_classify(&mut state, left_bound, right_bound, top_bound);
                    } else if let GestureState::TopEdge { start_x, done } = &mut state {
                        if !*done {
                            let dx = val - *start_x;
                            if dx.abs() >= STEP_THRESHOLD {
                                *done = true;
                                if dx < 0 {
                                    run_action("playerctl", &["previous"]).await;
                                } else {
                                    run_action("playerctl", &["next"]).await;
                                }
                            }
                        }
                    }
                }
                EventSummary::AbsoluteAxis(
                    _,
                    AbsoluteAxisCode::ABS_Y | AbsoluteAxisCode::ABS_MT_POSITION_Y,
                    val,
                ) => {
                    if let GestureState::Classifying { y, .. } = &mut state {
                        *y = Some(val);
                        try_classify(&mut state, left_bound, right_bound, top_bound);
                    } else {
                        match &mut state {
                            GestureState::LeftEdge { last_y } => {
                                let dy = val - *last_y;
                                if dy.abs() >= STEP_THRESHOLD {
                                    *last_y = val;
                                    run_action("pactl", &["set-sink-mute", "@DEFAULT_SINK@", "0"])
                                        .await;
                                    if dy < 0 {
                                        run_action(
                                            "pactl",
                                            &["set-sink-volume", "@DEFAULT_SINK@", "+5%"],
                                        )
                                        .await;
                                    } else {
                                        run_action(
                                            "pactl",
                                            &["set-sink-volume", "@DEFAULT_SINK@", "-5%"],
                                        )
                                        .await;
                                    }
                                }
                            }
                            GestureState::RightEdge { last_y } => {
                                let dy = val - *last_y;
                                if dy.abs() >= STEP_THRESHOLD {
                                    *last_y = val;
                                    let delta = if dy < 0 { 5 } else { -5 };
                                    if adjust_kde_brightness(delta).await.is_err() {
                                        let arg = if delta > 0 { "5%+" } else { "5%-" };
                                        run_action("brightnessctl", &["set", arg]).await;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                EventSummary::Key(
                    _,
                    KeyCode::BTN_TOOL_DOUBLETAP | KeyCode::BTN_TOOL_TRIPLETAP,
                    1,
                ) => {
                    state = GestureState::Other;
                }
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_SLOT, val) if val > 0 => {
                    state = GestureState::Other;
                }
                _ => {}
            }
        }
    }
}
