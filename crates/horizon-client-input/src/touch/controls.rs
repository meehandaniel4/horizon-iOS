//! The on-screen controls while playing: a movement stick on the left,
//! dragging anywhere else turns the view, a quick tap uses (places, eats,
//! opens) and holding still attacks (breaks), plus buttons for jump, sneak,
//! attack, use, drop, inventory, chat, perspective and pause, and a tap on a
//! hotbar slot selects it.
//!
//! Every control can be moved, resized, faded or hidden in the layout editor
//! (the Edit button), like a phone shooter's HUD editor; [`TouchSettings`] is
//! the saved layout. Positions are in GUI units (physical pixels over the GUI
//! scale), the units the HUD is drawn in, so the controls keep their size on
//! every screen, and they stay inside the screen's safe area (clear of the
//! notch and the home bar).
use std::collections::HashMap;
use std::time::{Duration, Instant};
use winit::event::{MouseButton, TouchPhase};

/// An on-screen button while playing. The game presses the controller
/// button bound to `action()`, so a rebind in the controller settings moves
/// the touch button with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TouchButton {
    Jump,
    Sneak,
    Attack,
    Use,
    Drop,
    Inventory,
    Chat,
    Perspective,
    Pause,
    /// Opens the layout editor.
    Edit,
    /// F3: the debug screen.
    Debug,
    /// Held while the stick is pushed fully forward; not on screen.
    Sprint,
    /// Swap item with offhand (F / key.swapOffhand).
    SwapHands,
    /// Zoom in (Z / zoom).
    Zoom,
}

impl TouchButton {
    /// The buttons on screen, in the order they are laid out.
    pub const PLACED: [TouchButton; 13] = [
        Self::Jump,
        Self::Sneak,
        Self::Attack,
        Self::Use,
        Self::Drop,
        Self::Pause,
        Self::Chat,
        Self::Perspective,
        Self::Edit,
        Self::Debug,
        Self::Inventory,
        Self::SwapHands,
        Self::Zoom,
    ];

    /// The key mapping it stands for; `None` for pause (the controller's
    /// Start), F3 (the keyboard's key, which no controller button has) and
    /// the editor's button.
    pub fn action(self) -> Option<&'static str> {
        Some(match self {
            Self::Jump => "key.jump",
            Self::Sneak => "key.sneak",
            Self::Attack => "key.attack",
            Self::Use => "key.use",
            Self::Inventory => "key.inventory",
            Self::Chat => "key.chat",
            Self::Drop => "key.drop",
            Self::Perspective => "key.togglePerspective",
            Self::Sprint => "key.sprint",
            Self::SwapHands => "key.swapOffhand",
            Self::Zoom | Self::Pause | Self::Edit | Self::Debug => return None,
        })
    }

    /// Its name in the settings file.
    fn key(self) -> &'static str {
        match self {
            Self::Jump => "jump",
            Self::Sneak => "sneak",
            Self::Attack => "attack",
            Self::Use => "use",
            Self::Drop => "drop",
            Self::Inventory => "inventory",
            Self::Chat => "chat",
            Self::Perspective => "perspective",
            Self::Pause => "pause",
            Self::Edit => "edit",
            Self::Debug => "debug",
            Self::Sprint => "sprint",
            Self::SwapHands => "swap_hands",
            Self::Zoom => "zoom",
        }
    }
}

/// Something the layout editor moves: a button, the movement stick or the hotbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Button(TouchButton),
    Stick,
    Hotbar,
}

impl Control {
    /// Every control, in the settings file's order.
    pub fn all() -> impl Iterator<Item = Control> {
        TouchButton::PLACED
            .into_iter()
            .map(Control::Button)
            .chain([Control::Stick, Control::Hotbar])
    }

    fn key(self) -> &'static str {
        match self {
            Self::Button(b) => b.key(),
            Self::Stick => "stick",
            Self::Hotbar => "hotbar",
        }
    }

    /// The translation key of its name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Button(TouchButton::Jump) => "horizon.touch.jump",
            Self::Button(TouchButton::Sneak) => "horizon.touch.sneak",
            Self::Button(TouchButton::Attack) => "horizon.touch.attack",
            Self::Button(TouchButton::Use) => "horizon.touch.use",
            Self::Button(TouchButton::Drop) => "horizon.touch.drop",
            Self::Button(TouchButton::Inventory) => "horizon.touch.inventory",
            Self::Button(TouchButton::Chat) => "horizon.touch.chat",
            Self::Button(TouchButton::Perspective) => "horizon.touch.perspective",
            Self::Button(TouchButton::Pause) => "horizon.touch.pause",
            Self::Button(TouchButton::Edit) => "horizon.touch.edit",
            Self::Button(TouchButton::Debug) => "horizon.touch.debug",
            Self::Button(TouchButton::Sprint) => "key.sprint",
            Self::Button(TouchButton::SwapHands) => "horizon.touch.swap",
            Self::Button(TouchButton::Zoom) => "horizon.touch.zoom",
            Self::Stick => "horizon.touch.stick",
            Self::Hotbar => "horizon.touch.hotbar",
        }
    }

    /// The hotbar keeps vanilla's size.
    pub fn resizable(self) -> bool {
        self != Self::Hotbar
    }

    /// The editor's own button and the hotbar always show.
    pub fn hideable(self) -> bool {
        !matches!(self, Self::Button(TouchButton::Edit) | Self::Hotbar)
    }
}

/// The smallest and largest size, as a multiple of the default.
pub const SIZES: (f32, f32) = (0.5, 2.5);
/// The faintest and fullest opacity.
pub const OPACITIES: (f32, f32) = (0.2, 1.0);
/// One press of a size or opacity button.
const STEP: f32 = 0.1;

/// Where one control is and how it looks; the default is where the layout
/// puts it, at its own size, fully opaque, shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Its centre as a fraction of the safe area (0 to 1 across, 0 to 1
    /// down); `None` is the default position.
    pub centre: Option<(f32, f32)>,
    pub size: f32,
    pub opacity: f32,
    pub hidden: bool,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            centre: None,
            size: 1.0,
            opacity: 1.0,
            hidden: false,
        }
    }
}

/// The saved layout: each control's placement where it differs from the default.
#[derive(Clone, Debug, PartialEq)]
pub struct TouchSettings {
    placements: HashMap<Control, Placement>,
    pub sensitivity: f32,
}

impl Default for TouchSettings {
    fn default() -> Self {
        Self {
            placements: HashMap::new(),
            sensitivity: 1.0,
        }
    }
}

impl TouchSettings {
    pub fn placement(&self, control: Control) -> Placement {
        self.placements.get(&control).copied().unwrap_or_default()
    }

    pub fn set(&mut self, control: Control, placement: Placement) {
        if placement == Placement::default() {
            self.placements.remove(&control);
        } else {
            self.placements.insert(control, placement);
        }
    }

    /// Read the settings file: one `name=x,y,size,opacity,hidden` line per
    /// changed control (`x,y` as `-,-` for the default position). Lines it
    /// can't read are skipped, so an old or hand-edited file still loads.
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("sensitivity=") {
                if let Ok(s) = val.trim().parse::<f32>() {
                    if s.is_finite() {
                        settings.sensitivity = s.clamp(0.1, 3.0);
                    }
                }
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let Some(control) = Control::all().find(|c| c.key() == name.trim()) else {
                continue;
            };
            let parts: Vec<&str> = value.split(',').map(str::trim).collect();
            let [x, y, size, opacity, hidden] = parts[..] else {
                continue;
            };
            let number = |s: &str| s.parse::<f32>().ok().filter(|v| v.is_finite());
            let centre = match (number(x), number(y)) {
                (Some(x), Some(y)) => Some((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))),
                _ => None,
            };
            let (Some(size), Some(opacity)) = (number(size), number(opacity)) else {
                continue;
            };
            settings.set(
                control,
                Placement {
                    centre,
                    size: size.clamp(SIZES.0, SIZES.1),
                    opacity: opacity.clamp(OPACITIES.0, OPACITIES.1),
                    hidden: hidden == "1" && control.hideable(),
                },
            );
        }
        settings
    }

    /// The settings file's text ([`parse`](Self::parse) reads it back).
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("sensitivity={:.2}\n", self.sensitivity));
        for control in Control::all() {
            let Some(p) = self.placements.get(&control) else {
                continue;
            };
            let (x, y) = match p.centre {
                Some((x, y)) => (format!("{x:.4}"), format!("{y:.4}")),
                None => ("-".to_owned(), "-".to_owned()),
            };
            out.push_str(&format!(
                "{}={x},{y},{:.2},{:.2},{}\n",
                control.key(),
                p.size,
                p.opacity,
                u8::from(p.hidden)
            ));
        }
        out
    }
}

/// A rectangle in GUI units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    pub fn contains(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn centre(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    fn around((cx, cy): (f32, f32), w: f32, h: f32) -> Self {
        Self {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            w,
            h,
        }
    }

    /// Moved to lie within `bounds` (as far as it fits).
    fn kept_in(self, bounds: Area) -> Self {
        Self {
            x: self.x.min(bounds.x + bounds.w - self.w).max(bounds.x),
            y: self.y.min(bounds.y + bounds.h - self.h).max(bounds.y),
            ..self
        }
    }
}

/// Vanilla's hotbar: 182 by 22 units, centred at the bottom, slots 20 apart.
pub const HOTBAR_WIDTH: f32 = 182.0;
pub const HOTBAR_HEIGHT: f32 = 22.0;
/// How far the stick's knob travels from where the finger came down, at size 1.
pub const STICK_RADIUS: f32 = 26.0;
/// A big button's side and a small one's, at size 1.
const BIG: f32 = 30.0;
const SMALL: f32 = 22.0;
const GAP: f32 = 4.0;
/// A finger that moved further than this before HOLD enters Dragging instead of Mining.
pub const TOUCH_SLOP: f32 = 9.0;
/// Held this long without moving beyond touch slop, a touch on the view enters mining.
pub const HOLD: Duration = Duration::from_millis(180);
/// Duration of a quick tap attack click (held so the engine registers the click).
pub const ATTACK_PULSE: Duration = Duration::from_millis(40);
/// A tapped button stays down this long: the game samples keys once a tick (50 ms).
const PULSE: Duration = Duration::from_millis(80);

/// One control as laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub control: Control,
    /// A button's face; the stick's area (where a finger becomes the
    /// stick); the hotbar's 182 by 22.
    pub area: Area,
    pub opacity: f32,
    pub hidden: bool,
}

/// Where the controls are on a screen `width` by `height` GUI units.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    /// The part of the screen clear of the notch and the home bar.
    pub safe: Area,
    pub items: Vec<Item>,
    /// How far the stick's knob travels.
    pub stick_radius: f32,
    /// Where the stick rests while no finger holds it.
    pub stick_home: (f32, f32),
}

impl Layout {
    /// The layout with every control where `settings` puts it.
    pub fn new(width: f32, height: f32, safe: Area, settings: &TouchSettings) -> Self {
        let mut layout = Self::defaults(width, height, safe);
        let screen = Area {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        };
        for item in &mut layout.items {
            let p = settings.placement(item.control);
            let size = if item.control.resizable() {
                p.size
            } else {
                1.0
            };
            let centre = p.centre.map_or(item.area.centre(), |(fx, fy)| {
                (safe.x + fx * safe.w, safe.y + fy * safe.h)
            });
            let (w, h) = match item.control {
                // A moved or resized stick takes a square around its home.
                Control::Stick if p.centre.is_some() || size != 1.0 => {
                    let side = STICK_RADIUS * 4.4 * size;
                    (side, side)
                }
                _ => (item.area.w * size, item.area.h * size),
            };
            if item.control == Control::Stick && p.centre.is_none() && size == 1.0 {
                // The default stick area stays as it is: most of the left side.
            } else {
                item.area = Area::around(centre, w, h).kept_in(screen);
            }
            item.opacity = p.opacity;
            item.hidden = p.hidden;
            if item.control == Control::Stick {
                layout.stick_radius = STICK_RADIUS * size;
                if p.centre.is_some() || size != 1.0 {
                    layout.stick_home = item.area.centre();
                }
            }
        }
        layout
    }

    /// The default layout: action buttons in the bottom right, pause, chat,
    /// view and edit along the top right, the inventory beside the hotbar,
    /// the stick on the bottom left.
    fn defaults(width: f32, height: f32, safe: Area) -> Self {
        let right = safe.x + safe.w;
        let bottom = safe.y + safe.h;
        let col = |i: f32| right - GAP - BIG - i * (BIG + GAP);
        let row = |i: f32| bottom - GAP - BIG - i * (BIG + GAP);
        let big = |x, y| Area {
            x,
            y,
            w: BIG,
            h: BIG,
        };
        let top = |i: f32| Area {
            x: right - GAP - SMALL - i * (SMALL + GAP),
            y: safe.y + GAP,
            w: SMALL,
            h: SMALL,
        };
        let hotbar = Area {
            x: ((width - HOTBAR_WIDTH) / 2.0).floor(),
            y: bottom - HOTBAR_HEIGHT,
            w: HOTBAR_WIDTH,
            h: HOTBAR_HEIGHT,
        };
        let stick = Area {
            x: safe.x,
            y: safe.y + safe.h * 0.3,
            w: (safe.w * 0.4).min(hotbar.x - safe.x).max(0.0),
            h: safe.h * 0.7,
        };
        let area = |control: Control| match control {
            Control::Button(TouchButton::Jump) => big(col(0.0), row(0.0)),
            Control::Button(TouchButton::Sneak) => big(col(1.0), row(0.0)),
            Control::Button(TouchButton::Attack) => big(col(0.0), row(1.0)),
            Control::Button(TouchButton::Use) => big(col(1.0), row(1.0)),
            Control::Button(TouchButton::Drop) => big(col(0.0), row(2.0)),
            Control::Button(TouchButton::Pause) => top(0.0),
            Control::Button(TouchButton::Chat) => top(1.0),
            Control::Button(TouchButton::Perspective) => top(2.0),
            Control::Button(TouchButton::Edit) => top(3.0),
            Control::Button(TouchButton::Debug) => top(4.0),
            Control::Button(TouchButton::SwapHands) => big(col(1.0), row(2.0)),
            Control::Button(TouchButton::Zoom) => big(col(0.0), row(3.0)),
            Control::Button(TouchButton::Inventory | TouchButton::Sprint) => Area {
                x: hotbar.x + hotbar.w + GAP,
                y: hotbar.y,
                w: SMALL,
                h: SMALL,
            },
            Control::Stick => stick,
            Control::Hotbar => hotbar,
        };
        let items = Control::all()
            .map(|control| {
                let hidden = matches!(
                    control,
                    Control::Button(TouchButton::SwapHands | TouchButton::Zoom)
                );
                Item {
                    control,
                    area: area(control),
                    opacity: 1.0,
                    hidden,
                }
            })
            .collect();
        let r = STICK_RADIUS;
        Self {
            width,
            height,
            safe,
            items,
            stick_radius: r,
            stick_home: (
                stick.x + GAP * 3.0 + r * 1.3,
                stick.y + stick.h - GAP * 3.0 - r * 1.3,
            ),
        }
    }

    pub fn item(&self, control: Control) -> &Item {
        self.items
            .iter()
            .find(|i| i.control == control)
            .expect("every control is laid out")
    }

    pub fn hotbar(&self) -> Area {
        self.item(Control::Hotbar).area
    }

    /// How far the hotbar was moved from vanilla's place (the game draws
    /// the hotbar, hearts, food and experience bar that much over).
    pub fn hotbar_offset(&self) -> (f32, f32) {
        let hotbar = self.hotbar();
        (
            hotbar.x - ((self.width - HOTBAR_WIDTH) / 2.0).floor(),
            hotbar.y - (self.height - HOTBAR_HEIGHT),
        )
    }

    /// The shown button under `at`, if any.
    pub fn button_at(&self, at: (f32, f32)) -> Option<TouchButton> {
        self.items.iter().find_map(|i| match i.control {
            Control::Button(b) if !i.hidden && i.area.contains(at) => Some(b),
            _ => None,
        })
    }

    /// The hotbar slot (0 to 8) under `at`.
    pub fn slot_at(&self, at: (f32, f32)) -> Option<usize> {
        let hotbar = self.hotbar();
        hotbar
            .contains(at)
            .then(|| (((at.0 - hotbar.x - 1.0) / 20.0).floor().max(0.0) as usize).min(8))
    }

    /// The control the editor picks at `at`, shown or hidden: a button over
    /// the hotbar over the stick's area.
    pub fn control_at(&self, at: (f32, f32)) -> Option<Control> {
        let rank = |c: Control| match c {
            Control::Button(_) => 0,
            Control::Hotbar => 1,
            Control::Stick => 2,
        };
        self.items
            .iter()
            .filter(|i| i.area.contains(at))
            .min_by_key(|i| rank(i.control))
            .map(|i| i.control)
    }
}

/// The layout editor's toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorButton {
    Smaller,
    Bigger,
    Fainter,
    Stronger,
    Hide,
    Add,
    SensDown,
    SensUp,
    Reset,
    Done,
}

impl EditorButton {
    pub const ALL: [EditorButton; 10] = [
        Self::Smaller,
        Self::Bigger,
        Self::Fainter,
        Self::Stronger,
        Self::Hide,
        Self::Add,
        Self::SensDown,
        Self::SensUp,
        Self::Reset,
        Self::Done,
    ];

    /// The translation key of its label; `hidden`: the selected control is hidden.
    pub fn label(self, hidden: bool) -> &'static str {
        match self {
            Self::Smaller => "horizon.touch.edit.smaller",
            Self::Bigger => "horizon.touch.edit.bigger",
            Self::Fainter => "horizon.touch.edit.fainter",
            Self::Stronger => "horizon.touch.edit.stronger",
            Self::Hide if hidden => "horizon.touch.edit.show",
            Self::Hide => "horizon.touch.edit.hide",
            Self::Add => "horizon.touch.edit.add",
            Self::SensDown => "horizon.touch.edit.sens_down",
            Self::SensUp => "horizon.touch.edit.sens_up",
            Self::Reset => "horizon.touch.edit.reset",
            Self::Done => "horizon.touch.edit.done",
        }
    }

    /// The toolbar buttons across the safe area in two rows.
    pub fn toolbar(layout: &Layout) -> Vec<(EditorButton, Area)> {
        let (w1, h1) = (48.0, 18.0);
        let row1 = [
            Self::Smaller,
            Self::Bigger,
            Self::Fainter,
            Self::Stronger,
            Self::Hide,
            Self::Add,
            Self::Done,
        ];
        let total1 = row1.len() as f32 * (w1 + GAP) - GAP;
        let x0 = (layout.safe.x + (layout.safe.w - total1) / 2.0).max(layout.safe.x);
        let y0 = layout.safe.y + GAP + SMALL + GAP * 2.0;

        let mut out: Vec<(EditorButton, Area)> = row1
            .iter()
            .enumerate()
            .map(|(i, b)| {
                (
                    *b,
                    Area {
                        x: x0 + i as f32 * (w1 + GAP),
                        y: y0,
                        w: w1,
                        h: h1,
                    },
                )
            })
            .collect();

        // Row 2: SensDown, SensUp, Reset
        let (w2, h2) = (56.0, 18.0);
        let total2 = 3.0 * w2 + 60.0 + 3.0 * GAP;
        let x2 = (layout.safe.x + (layout.safe.w - total2) / 2.0).max(layout.safe.x);
        let y2 = y0 + h1 + GAP;

        out.push((
            Self::SensDown,
            Area {
                x: x2,
                y: y2,
                w: w2,
                h: h2,
            },
        ));
        out.push((
            Self::SensUp,
            Area {
                x: x2 + w2 + GAP + 60.0 + GAP,
                y: y2,
                w: w2,
                h: h2,
            },
        ));
        out.push((
            Self::Reset,
            Area {
                x: x2 + (w2 + GAP) * 2.0 + 60.0,
                y: y2,
                w: w2,
                h: h2,
            },
        ));

        out
    }

    /// Area for displaying sensitivity value between SensDown and SensUp.
    pub fn sens_display_area(layout: &Layout) -> Area {
        let y0 = layout.safe.y + GAP + SMALL + GAP * 2.0;
        let (w2, h2) = (56.0, 18.0);
        let total2 = 3.0 * w2 + 60.0 + 3.0 * GAP;
        let x2 = (layout.safe.x + (layout.safe.w - total2) / 2.0).max(layout.safe.x);
        let y2 = y0 + 18.0 + GAP;
        Area {
            x: x2 + w2 + GAP,
            y: y2,
            w: 60.0,
            h: h2,
        }
    }
}

impl Layout {
    /// Dialog box and option areas for adding extra buttons (F, Z, Close).
    pub fn add_dialog_areas(&self) -> (Area, Area, Area, Area) {
        let (w, h) = (200.0, 92.0);
        let box_area = Area {
            x: self.safe.x + (self.safe.w - w) / 2.0,
            y: self.safe.y + (self.safe.h - h) / 2.0,
            w,
            h,
        };
        let f_btn = Area {
            x: box_area.x + 8.0,
            y: box_area.y + 10.0,
            w: w - 16.0,
            h: 22.0,
        };
        let z_btn = Area {
            x: box_area.x + 8.0,
            y: box_area.y + 36.0,
            w: w - 16.0,
            h: 22.0,
        };
        let close_btn = Area {
            x: box_area.x + 8.0,
            y: box_area.y + 62.0,
            w: w - 16.0,
            h: 22.0,
        };
        (box_area, f_btn, z_btn, close_btn)
    }
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    finger: u64,
    control: Control,
    /// The finger's place less the control's centre when it was picked up.
    grab: (f32, f32),
}

#[derive(Clone, Debug)]
struct Editor {
    draft: TouchSettings,
    selected: Option<Control>,
    drag: Option<Drag>,
    show_add_menu: bool,
}

/// What the editor shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorView {
    pub selected: Option<Control>,
    /// The selected control's placement.
    pub placement: Placement,
    pub sensitivity: f32,
    pub show_add_menu: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookState {
    Pending,
    Dragging,
    Mining,
}

#[derive(Clone, Copy, Debug)]
enum Role {
    Stick {
        origin: (f32, f32),
        at: (f32, f32),
    },
    Look {
        last: (f32, f32),
        start: (f32, f32),
        since: Instant,
        state: LookState,
        usable: bool,
    },
    Button(TouchButton),
    /// A finger on the hotbar picks its slot as it lands and then does nothing.
    Hotbar,
}

/// A mouse action to execute (press or release).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchMouseAction {
    Press(MouseButton),
    Release(MouseButton),
}

/// What the controls want this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchFrame {
    /// Buttons to hold down now.
    pub held: Vec<TouchButton>,
    /// The movement stick, -1 to 1, up (forward) positive.
    pub stick: [f32; 2],
    /// How far the view was dragged since the last frame, in GUI units.
    pub look: (f32, f32),
    /// A hotbar slot tapped since the last frame.
    pub slot: Option<usize>,
    /// Mouse actions triggered this frame (e.g. Bedrock tap attack and mining).
    pub mouse_actions: Vec<TouchMouseAction>,
}

/// The on-screen controls while playing, and their layout editor.
#[derive(Default, Debug)]
pub struct TouchControls {
    /// The layout in use (loaded from the settings file).
    pub settings: TouchSettings,
    editor: Option<Editor>,
    /// A layout the editor finished, for the game to save.
    saved: Option<TouchSettings>,
    fingers: HashMap<u64, Role>,
    /// Sneak toggles: one tap down, the next back up.
    sneaking: bool,
    /// The game toggles sneaking itself on each press (the controller's
    /// toggle sneak): Sneak is then a tap, and the game's state says whether
    /// it is down. Set by the game.
    pub sneak_taps: bool,
    /// Buttons tapped and held for a pulse, until then.
    pulses: Vec<(TouchButton, Instant)>,
    look: (f32, f32),
    slot: Option<usize>,
    mouse_actions: Vec<TouchMouseAction>,
    tap_releases: Vec<(MouseButton, Instant)>,
    mining_touches: usize,
    pub holding_use: bool,
    /// Whether the held item is usable (food, bow, potion, shield).
    pub held_usable: bool,
}

impl TouchControls {
    /// The controls laid out on a screen `width` by `height` GUI units: the
    /// editor's draft while it is open.
    pub fn layout(&self, width: f32, height: f32, safe: Area) -> Layout {
        let settings = self.editor.as_ref().map_or(&self.settings, |e| &e.draft);
        Layout::new(width, height, safe, settings)
    }

    pub fn editing(&self) -> bool {
        self.editor.is_some()
    }

    pub fn editor(&self) -> Option<EditorView> {
        self.editor.as_ref().map(|e| EditorView {
            selected: e.selected,
            placement: e.selected.map(|c| e.draft.placement(c)).unwrap_or_default(),
            sensitivity: e.draft.sensitivity,
            show_add_menu: e.show_add_menu,
        })
    }

    /// A layout the editor's Done just kept, to write to the settings file.
    pub fn take_saved(&mut self) -> Option<TouchSettings> {
        self.saved.take()
    }

    /// A finger came down, moved, lifted or was cancelled at `at`.
    pub fn touch(
        &mut self,
        layout: &Layout,
        id: u64,
        phase: TouchPhase,
        at: (f32, f32),
        now: Instant,
    ) {
        if self.editor.is_some() {
            self.edit_touch(layout, id, phase, at);
            return;
        }
        use TouchPhase::*;
        match phase {
            Started => {
                let role = if let Some(button) = layout.button_at(at) {
                    match button {
                        TouchButton::Sneak if !self.sneak_taps => {
                            self.sneaking = !self.sneaking;
                        }
                        TouchButton::Edit => {
                            self.release_all();
                            self.editor = Some(Editor {
                                draft: self.settings.clone(),
                                selected: None,
                                drag: None,
                                show_add_menu: false,
                            });
                            return;
                        }
                        _ => self.pulse(button, now),
                    }
                    Role::Button(button)
                } else if let Some(slot) = layout.slot_at(at) {
                    self.slot = Some(slot);
                    Role::Hotbar
                } else if layout.item(Control::Stick).area.contains(at)
                    && !layout.item(Control::Stick).hidden
                    && !self
                        .fingers
                        .values()
                        .any(|r| matches!(r, Role::Stick { .. }))
                {
                    Role::Stick { origin: at, at }
                } else {
                    Role::Look {
                        last: at,
                        start: at,
                        since: now,
                        state: LookState::Pending,
                        usable: self.held_usable,
                    }
                };
                self.fingers.insert(id, role);
            }
            Moved => match self.fingers.get_mut(&id) {
                Some(Role::Stick { at: stick, .. }) => *stick = at,
                Some(Role::Look {
                    last,
                    start,
                    state,
                    ..
                }) => {
                    self.look.0 += at.0 - last.0;
                    self.look.1 += at.1 - last.1;
                    *last = at;
                    if *state == LookState::Pending && distance(*start, at) > TOUCH_SLOP {
                        *state = LookState::Dragging;
                    }
                }
                _ => {}
            },
            Ended | Cancelled => {
                if let Some(Role::Look { state, .. }) = self.fingers.remove(&id) {
                    match state {
                        LookState::Mining => {
                            if self.mining_touches > 0 {
                                self.mining_touches -= 1;
                                if self.mining_touches == 0 {
                                    let btn = if self.holding_use {
                                        self.holding_use = false;
                                        MouseButton::Right
                                    } else {
                                        MouseButton::Left
                                    };
                                    self.mouse_actions
                                        .push(TouchMouseAction::Release(btn));
                                }
                            }
                        }
                        LookState::Pending if phase == Ended => {
                            // Quick single tap (< 180 ms without move beyond touchSlop):
                            // Right click: place block or use/interact
                            self.mouse_actions
                                .push(TouchMouseAction::Press(MouseButton::Right));
                            self.tap_releases
                                .push((MouseButton::Right, now + ATTACK_PULSE));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// A touch while the editor is open: the toolbar, or picking up and
    /// dragging a control.
    fn edit_touch(&mut self, layout: &Layout, id: u64, phase: TouchPhase, at: (f32, f32)) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if editor.show_add_menu {
            if phase == TouchPhase::Started {
                let (box_area, f_btn, z_btn, close_btn) = layout.add_dialog_areas();
                if f_btn.contains(at) {
                    let mut p = editor.draft.placement(Control::Button(TouchButton::SwapHands));
                    p.hidden = false;
                    editor.draft.set(Control::Button(TouchButton::SwapHands), p);
                    editor.selected = Some(Control::Button(TouchButton::SwapHands));
                    editor.show_add_menu = false;
                } else if z_btn.contains(at) {
                    let mut p = editor.draft.placement(Control::Button(TouchButton::Zoom));
                    p.hidden = false;
                    editor.draft.set(Control::Button(TouchButton::Zoom), p);
                    editor.selected = Some(Control::Button(TouchButton::Zoom));
                    editor.show_add_menu = false;
                } else if close_btn.contains(at) || !box_area.contains(at) {
                    editor.show_add_menu = false;
                }
            }
            return;
        }
        match phase {
            TouchPhase::Started => {
                if let Some((button, _)) = EditorButton::toolbar(layout)
                    .into_iter()
                    .find(|(_, area)| area.contains(at))
                {
                    self.editor_button(button);
                    return;
                }
                editor.selected = layout.control_at(at);
                editor.drag = editor.selected.map(|control| {
                    let (cx, cy) = layout.item(control).area.centre();
                    Drag {
                        finger: id,
                        control,
                        grab: (at.0 - cx, at.1 - cy),
                    }
                });
            }
            TouchPhase::Moved => {
                let Some(drag) = editor.drag.filter(|d| d.finger == id) else {
                    return;
                };
                let safe = layout.safe;
                if safe.w <= 0.0 || safe.h <= 0.0 {
                    return;
                }
                let (cx, cy) = (at.0 - drag.grab.0, at.1 - drag.grab.1);
                let mut p = editor.draft.placement(drag.control);
                p.centre = Some((
                    ((cx - safe.x) / safe.w).clamp(0.0, 1.0),
                    ((cy - safe.y) / safe.h).clamp(0.0, 1.0),
                ));
                editor.draft.set(drag.control, p);
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if editor.drag.is_some_and(|d| d.finger == id) {
                    editor.drag = None;
                }
            }
        }
    }

    /// A toolbar button: it changes the selected control, or every button
    /// when none is selected (Hide needs one).
    fn editor_button(&mut self, button: EditorButton) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if button == EditorButton::Done {
            let draft = editor.draft.clone();
            self.settings = draft.clone();
            self.saved = Some(draft);
            self.editor = None;
            return;
        }
        if button == EditorButton::Add {
            editor.show_add_menu = !editor.show_add_menu;
            return;
        }
        if button == EditorButton::SensDown {
            editor.draft.sensitivity =
                (((editor.draft.sensitivity - 0.05) * 100.0).round() / 100.0).clamp(0.10, 3.00);
            return;
        }
        if button == EditorButton::SensUp {
            editor.draft.sensitivity =
                (((editor.draft.sensitivity + 0.05) * 100.0).round() / 100.0).clamp(0.10, 3.00);
            return;
        }
        if button == EditorButton::Reset {
            if let Some(c) = editor.selected {
                editor.draft.set(c, Placement::default());
            } else {
                for c in Control::all() {
                    editor.draft.set(c, Placement::default());
                }
                editor.draft.sensitivity = 1.0;
            }
            return;
        }
        let targets: Vec<Control> = match editor.selected {
            Some(c) => vec![c],
            None if button == EditorButton::Hide => Vec::new(),
            None => Control::all().collect(),
        };
        for control in targets {
            let mut p = editor.draft.placement(control);
            match button {
                EditorButton::Smaller if control.resizable() => {
                    p.size = step(p.size, -STEP, SIZES);
                }
                EditorButton::Bigger if control.resizable() => {
                    p.size = step(p.size, STEP, SIZES);
                }
                EditorButton::Fainter => p.opacity = step(p.opacity, -STEP, OPACITIES),
                EditorButton::Stronger => p.opacity = step(p.opacity, STEP, OPACITIES),
                EditorButton::Hide if control.hideable() => p.hidden = !p.hidden,
                _ => {}
            }
            editor.draft.set(control, p);
        }
    }

    fn pulse(&mut self, button: TouchButton, now: Instant) {
        self.pulses.push((button, now + PULSE));
    }

    /// Lets go of everything (a menu opened, the app went to the background).
    pub fn release_all(&mut self) {
        let was_mining = self.mining_touches > 0 || !self.tap_releases.is_empty();
        let was_use = self.holding_use;
        self.fingers.clear();
        self.pulses.clear();
        self.mining_touches = 0;
        self.holding_use = false;
        self.tap_releases.clear();
        self.look = (0.0, 0.0);
        self.slot = None;
        if was_mining {
            let btn = if was_use {
                MouseButton::Right
            } else {
                MouseButton::Left
            };
            self.mouse_actions.push(TouchMouseAction::Release(btn));
        }
        if let Some(editor) = &mut self.editor {
            editor.drag = None;
        }
    }

    /// Whether any finger is on the controls.
    pub fn active(&self) -> bool {
        !self.fingers.is_empty()
    }

    /// The stick's centre and its knob while a finger holds it, for drawing.
    pub fn stick(&self, layout: &Layout) -> Option<((f32, f32), (f32, f32))> {
        self.fingers.values().find_map(|r| match *r {
            Role::Stick { origin, at } => Some((origin, knob(origin, at, layout.stick_radius))),
            _ => None,
        })
    }

    /// Whether `button` is down now, for drawing it pressed (with
    /// `sneak_taps`, the game knows whether sneaking is on).
    pub fn pressed(&self, button: TouchButton) -> bool {
        (button == TouchButton::Sneak && self.sneaking && !self.sneak_taps)
            || self
                .fingers
                .values()
                .any(|r| matches!(r, Role::Button(b) if *b == button))
    }

    /// The state to apply this frame; the look and the tapped slot are
    /// taken. Nothing while the editor is open.
    pub fn frame(&mut self, layout: &Layout, now: Instant) -> TouchFrame {
        if self.editor.is_some() {
            return TouchFrame::default();
        }
        self.pulses.retain(|(_, until)| *until > now);
        let mut held: Vec<TouchButton> = self.pulses.iter().map(|(b, _)| *b).collect();
        let mut stick = [0.0; 2];
        let radius = layout.stick_radius;
        for role in self.fingers.values_mut() {
            match role {
                Role::Button(b)
                    if *b != TouchButton::Edit && (*b != TouchButton::Sneak || self.sneak_taps) =>
                {
                    held.push(*b);
                }
                Role::Stick { origin, at } => {
                    let k = knob(*origin, *at, radius);
                    stick = [(k.0 - origin.0) / radius, -(k.1 - origin.1) / radius];
                    if stick[1] > 0.9 {
                        held.push(TouchButton::Sprint);
                    }
                }
                Role::Look {
                    since,
                    state,
                    usable,
                    ..
                } => {
                    if *state == LookState::Pending && now.duration_since(*since) >= HOLD {
                        *state = LookState::Mining;
                        self.mining_touches += 1;
                        if self.mining_touches == 1 {
                            let button = if *usable {
                                self.holding_use = true;
                                MouseButton::Right
                            } else {
                                self.holding_use = false;
                                MouseButton::Left
                            };
                            self.mouse_actions.push(TouchMouseAction::Press(button));
                        }
                    }
                }
                _ => {}
            }
        }
        self.tap_releases.retain(|(btn, until)| {
            if *until <= now {
                if self.mining_touches == 0 {
                    self.mouse_actions
                        .push(TouchMouseAction::Release(*btn));
                }
                false
            } else {
                true
            }
        });
        if self.sneaking && !self.sneak_taps {
            held.push(TouchButton::Sneak);
        }
        held.sort_by_key(|b| *b as u8);
        held.dedup();
        TouchFrame {
            held,
            stick,
            look: std::mem::take(&mut self.look),
            slot: self.slot.take(),
            mouse_actions: std::mem::take(&mut self.mouse_actions),
        }
    }
}

/// `value` moved by `by`, kept within `bounds`, rounded to the step.
fn step(value: f32, by: f32, bounds: (f32, f32)) -> f32 {
    (((value + by) * 10.0).round() / 10.0).clamp(bounds.0, bounds.1)
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// `at`, held within `radius` of `origin`.
fn knob(origin: (f32, f32), at: (f32, f32), radius: f32) -> (f32, f32) {
    let d = distance(origin, at);
    if d <= radius {
        return at;
    }
    let k = radius / d;
    (
        origin.0 + (at.0 - origin.0) * k,
        origin.1 + (at.1 - origin.1) * k,
    )
}

#[cfg(test)]
mod tests;
