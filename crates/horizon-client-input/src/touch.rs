//! Touch screens. Over menus, [`TouchMouse`] makes the first finger the mouse
//! (tapping clicks, dragging horizontally slides, swiping vertically scrolls).
//! While playing, [`TouchControls`] are the on-screen controls, whose layout
//! the player edits (controls.rs), with their icons (icons.rs). iOS sends
//! only touches.
mod controls;
pub mod icons;

pub use controls::*;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase, WindowEvent};

/// Minimum movement (in physical pixels) before a touch transitions from
/// a pending tap to vertical scrolling or horizontal dragging.
const MENU_TOUCH_SLOP: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TouchMouseMode {
    Pending,
    Scrolling,
    Dragging,
}

/// A touch screen as a mouse: the first finger down moves the cursor.
/// A quick tap clicks; dragging vertically scrolls lists via `MouseWheel`;
/// dragging horizontally acts as mouse drag (e.g. for sliders). Other fingers
/// are ignored.
#[derive(Debug)]
pub struct TouchMouse {
    finger: Option<u64>,
    start: (f64, f64),
    last: (f64, f64),
    mode: TouchMouseMode,
}

impl Default for TouchMouse {
    fn default() -> Self {
        Self {
            finger: None,
            start: (0.0, 0.0),
            last: (0.0, 0.0),
            mode: TouchMouseMode::Pending,
        }
    }
}

impl TouchMouse {
    /// The finger acting as the mouse.
    pub fn finger(&self) -> Option<u64> {
        self.finger
    }

    /// Releases any active touch and resets mode.
    pub fn release(&mut self) -> Vec<WindowEvent> {
        let mut out = Vec::new();
        if self.mode == TouchMouseMode::Dragging {
            out.push(WindowEvent::MouseInput {
                device_id: winit::event::DeviceId::dummy(),
                state: ElementState::Released,
                button: MouseButton::Left,
            });
        }
        self.finger = None;
        self.mode = TouchMouseMode::Pending;
        out
    }

    /// The mouse events `event` stands for. A touch by another finger gives
    /// none; any other event passes through unchanged.
    pub fn translate(&mut self, event: WindowEvent) -> Vec<WindowEvent> {
        let WindowEvent::Touch(Touch {
            device_id,
            phase,
            location,
            id,
            ..
        }) = event
        else {
            return vec![event];
        };
        let moved = WindowEvent::CursorMoved {
            device_id,
            position: location,
        };
        let button = |state| WindowEvent::MouseInput {
            device_id,
            state,
            button: MouseButton::Left,
        };
        match phase {
            TouchPhase::Started if self.finger.is_none() => {
                self.finger = Some(id);
                self.start = (location.x, location.y);
                self.last = (location.x, location.y);
                self.mode = TouchMouseMode::Pending;
                vec![moved]
            }
            TouchPhase::Moved if self.finger == Some(id) => {
                let dx = location.x - self.start.0;
                let dy = location.y - self.start.1;
                let dist = (dx * dx + dy * dy).sqrt();
                let step_dy = location.y - self.last.1;
                self.last = (location.x, location.y);

                let mut out = Vec::new();
                if self.mode == TouchMouseMode::Pending && dist > MENU_TOUCH_SLOP {
                    if dy.abs() >= dx.abs() {
                        self.mode = TouchMouseMode::Scrolling;
                    } else {
                        self.mode = TouchMouseMode::Dragging;
                        out.push(button(ElementState::Pressed));
                    }
                }

                match self.mode {
                    TouchMouseMode::Pending => {
                        out.push(moved);
                    }
                    TouchMouseMode::Scrolling => {
                        out.push(moved);
                        out.push(WindowEvent::MouseWheel {
                            device_id,
                            delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, step_dy)),
                            phase: TouchPhase::Moved,
                        });
                    }
                    TouchMouseMode::Dragging => {
                        out.push(moved);
                    }
                }
                out
            }
            TouchPhase::Ended if self.finger == Some(id) => {
                let mut out = Vec::new();
                match self.mode {
                    TouchMouseMode::Pending => {
                        out.push(moved);
                        out.push(button(ElementState::Pressed));
                        out.push(button(ElementState::Released));
                    }
                    TouchMouseMode::Scrolling => {
                        // Scrolling finished: no click on whatever was under the finger
                    }
                    TouchMouseMode::Dragging => {
                        out.push(moved);
                        out.push(button(ElementState::Released));
                    }
                }
                self.finger = None;
                self.mode = TouchMouseMode::Pending;
                out
            }
            TouchPhase::Cancelled if self.finger == Some(id) => {
                let mut out = Vec::new();
                if self.mode == TouchMouseMode::Dragging {
                    out.push(button(ElementState::Released));
                }
                self.finger = None;
                self.mode = TouchMouseMode::Pending;
                out
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;
    use winit::event::DeviceId;

    fn touch(id: u64, phase: TouchPhase, x: f64, y: f64) -> WindowEvent {
        WindowEvent::Touch(Touch {
            device_id: DeviceId::dummy(),
            phase,
            location: PhysicalPosition::new(x, y),
            force: None,
            id,
        })
    }

    #[derive(Debug, PartialEq)]
    enum Out {
        Move(f64, f64),
        Button(ElementState),
        Wheel(f64, f64),
    }

    fn outs(events: Vec<WindowEvent>) -> Vec<Out> {
        events
            .into_iter()
            .map(|e| match e {
                WindowEvent::CursorMoved { position, .. } => Out::Move(position.x, position.y),
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => Out::Button(state),
                WindowEvent::MouseWheel { delta, .. } => match delta {
                    MouseScrollDelta::PixelDelta(p) => Out::Wheel(p.x, p.y),
                    MouseScrollDelta::LineDelta(x, y) => Out::Wheel(f64::from(x), f64::from(y)),
                },
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_tap_is_a_left_click_where_the_finger_is() {
        let mut t = TouchMouse::default();
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Started, 10.0, 20.0))),
            [Out::Move(10.0, 20.0)]
        );
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Ended, 10.0, 20.0))),
            [
                Out::Move(10.0, 20.0),
                Out::Button(ElementState::Pressed),
                Out::Button(ElementState::Released)
            ]
        );
    }

    #[test]
    fn a_vertical_swipe_scrolls_the_list_without_clicking() {
        let mut t = TouchMouse::default();
        t.translate(touch(1, TouchPhase::Started, 100.0, 200.0));
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Moved, 100.0, 180.0))),
            [Out::Move(100.0, 180.0), Out::Wheel(0.0, -20.0)]
        );
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Moved, 100.0, 160.0))),
            [Out::Move(100.0, 160.0), Out::Wheel(0.0, -20.0)]
        );
        assert!(t.translate(touch(1, TouchPhase::Ended, 100.0, 160.0)).is_empty());
    }

    #[test]
    fn a_horizontal_drag_moves_slider_with_the_button_held() {
        let mut t = TouchMouse::default();
        t.translate(touch(1, TouchPhase::Started, 50.0, 50.0));
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Moved, 70.0, 50.0))),
            [Out::Button(ElementState::Pressed), Out::Move(70.0, 50.0)]
        );
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Moved, 80.0, 50.0))),
            [Out::Move(80.0, 50.0)]
        );
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Ended, 80.0, 50.0))),
            [Out::Move(80.0, 50.0), Out::Button(ElementState::Released)]
        );
    }

    #[test]
    fn a_second_finger_is_ignored_until_the_first_lifts() {
        let mut t = TouchMouse::default();
        t.translate(touch(1, TouchPhase::Started, 0.0, 0.0));
        assert!(
            t.translate(touch(2, TouchPhase::Started, 9.0, 9.0))
                .is_empty()
        );
        assert!(
            t.translate(touch(2, TouchPhase::Moved, 8.0, 8.0))
                .is_empty()
        );
        assert!(
            t.translate(touch(2, TouchPhase::Ended, 8.0, 8.0))
                .is_empty()
        );
        assert!(t.translate(touch(1, TouchPhase::Cancelled, 1.0, 1.0)).is_empty());
        assert_eq!(
            outs(t.translate(touch(2, TouchPhase::Started, 3.0, 4.0))),
            [Out::Move(3.0, 4.0)]
        );
    }

    #[test]
    fn other_events_pass_through() {
        let mut t = TouchMouse::default();
        let out = t.translate(WindowEvent::Focused(true));
        assert!(matches!(out[..], [WindowEvent::Focused(true)]));
    }
}
