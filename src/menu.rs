use canvas::Canvas;
use std::rc::{Rc, Weak};
use std::cell::Cell;
use vm::Vm;
use TouchReceiver;
use Visible;
use WorldPoint;
use DisplayPoint;
use PARAM_RADIUS;

pub trait Action {
    fn start(self: Box<Self>, &mut Vm, DisplayPoint, WorldPoint) -> Option<Box<TouchReceiver>>;
}

pub struct Entry {
    pub name: String,
    pub color: Option<String>,
    pub shortcuts: Vec<String>,
    pub action: Box<Action>,
}

pub struct Menu {
    pub entries: Vec<Entry>,
    pub color: String,
}

impl Menu {
    pub fn activate_shortcut(self,
                             vm: &mut Vm,
                             used_shortcut: String,
                             display: DisplayPoint,
                             world: WorldPoint)
                             -> Option<Box<TouchReceiver>> {
        self.entries
            .into_iter()
            .find(|entry| {
                      entry
                          .shortcuts
                          .iter()
                          .any(|entry_shortcut| &used_shortcut == entry_shortcut)
                  })
            .and_then(move |entry| entry.action.start(vm, display, world))
    }
}

pub struct VisibleMenu {
    menu: Menu,
    page: Cell<usize>,
    last_touch: Cell<DisplayPoint>,
}

use std::f64::consts::PI;
const MARGIN: f64 = 1.;
const NEAR: f64 = PARAM_RADIUS + MARGIN;
const FAR: f64 = NEAR + PARAM_RADIUS * 2.;
const ANGLE: f64 = PI / 8.;
const ANGLE_START: f64 = ANGLE * 6.;

impl VisibleMenu {
    pub fn new(menu: Menu, touch: DisplayPoint) -> Rc<VisibleMenu> {
        Rc::new(VisibleMenu {
                    menu: menu,
                    page: Cell::new(0),
                    last_touch: Cell::new(touch),
                })
    }
}

impl Visible for Rc<VisibleMenu> {
    fn draw(&self, c: &mut Canvas) {
        let pos = self.last_touch.get();
        c.translate(pos.x, pos.y);
        c.fillStyle(self.menu.color.as_ref());
        c.fillCircle(0., 0., PARAM_RADIUS);
        //
        let near_a = ANGLE - (MARGIN / 2.).atan2(NEAR);
        let far_a = ANGLE - (MARGIN / 2.).atan2(FAR);
        let mut mid = ANGLE_START;
        for (i, entry) in self.menu.entries.iter().enumerate() {
            c.beginPath();
            c.arc(0., 0., NEAR, mid - near_a, mid + near_a, false);
            c.arc(0., 0., FAR, mid + far_a, mid - far_a, true);
            c.fillStyle(self.menu.color.as_ref());
            c.fill();
            let x = mid.cos();
            let y = mid.sin();
            if x < -0.1 {
                c.textAlign("right");
            } else if x > 0.1 {
                c.textAlign("left");
            } else {
                c.textAlign("center");
            }
            if y < -0.9 {
                c.textBaseline("bottom");
            } else if y > 0.9 {
                c.textBaseline("top");
            } else {
                c.textBaseline("middle");
            }
            let mut text = entry.name.clone();
            if !entry.shortcuts.is_empty() {
                text.push_str(" [");
                text.push_str(entry.shortcuts.join(",").as_ref());
                text.push_str("]");
            }
            c.fillStyle("#000");
            c.fillText(text.as_ref(), x * (FAR + 3.), y * (FAR + 3.));
            mid += ANGLE * 2.;
        }
    }
    fn make_menu(&self, d: DisplayPoint, w: WorldPoint) -> Option<Menu> {
        None
    }
}

impl TouchReceiver for Rc<VisibleMenu> {
    fn continue_touch(self: Box<Self>,
                      vm: &mut Vm,
                      display: DisplayPoint,
                      world: WorldPoint)
                      -> Option<Box<TouchReceiver>> {
        let prev = self.last_touch.get();
        let delta = display - prev;
        let len = delta.x.hypot(delta.y);
        let l = len / PARAM_RADIUS;
        let a = delta.y.atan2(delta.x);
        if len > FAR {
            return Rc::try_unwrap(*self)
                       .ok()
                       .unwrap()
                       .menu
                       .entries
                       .into_iter()
                       .next()
                       .unwrap()
                       .action
                       .start(vm, display, world);
        }
        /*
        if len < PARAM_RADIUS {
            self.last_touch
                .set(prev + DisplayPoint::new(a.cos(), a.sin()) * l * 0.2);
        }
         */
        Some(self)
    }
    fn end_touch(self: Box<Self>, _: &mut Vm) {}
}
