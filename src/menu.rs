use canvas::Canvas;
use std::rc::{Rc, Weak};
use std::cell::Cell;
use TouchReceiver;
use Visible;
use WorldPoint;
use DisplayPoint;
use PARAM_RADIUS;

pub trait Action {
    fn start(self: Box<Self>, DisplayPoint, WorldPoint) -> Option<Box<TouchReceiver>>;
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
            .and_then(move |entry| entry.action.start(display, world))
    }
}

pub struct VisibleMenu {
    menu: Menu,
    page: Cell<usize>,
    last_touch: Cell<DisplayPoint>,
}

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
        // self.menu.entries.iter().enumerate()
        use std::f64::consts::PI;
        let margin: f64 = 1.;
        let near = PARAM_RADIUS + margin;
        let far = near + PARAM_RADIUS * 2.;
        let a = PI / 8.;
        let near_a = a - (margin / 2.).atan2(near);
        let far_a = a - (margin / 2.).atan2(far);
        let mut mid = PI * 0.25 * 3.;
        for _ in 0..7 {
            c.beginPath();
            c.arc(0., 0., near, mid - near_a, mid + near_a, false);
            c.arc(0., 0., far, mid + far_a, mid - far_a, true);

            c.fill();
            mid += PI / 4.;
        }
    }

    fn start_touch(&self,
                   display: &DisplayPoint,
                   world: &WorldPoint)
                   -> Option<Box<TouchReceiver>> {
        return None;
    }
}

impl TouchReceiver for Rc<VisibleMenu> {
    fn continue_touch(self: Box<Self>,
                      next: DisplayPoint,
                      _: WorldPoint)
                      -> Option<Box<TouchReceiver>> {
        let prev = self.last_touch.get();
        let delta = next - prev;
        let len = delta.x.hypot(delta.y);
        if len < PARAM_RADIUS {
            self.last_touch.set(prev + delta / PARAM_RADIUS * 0.2);
        }
        Some(self)
    }
    fn end_touch(self: Box<Self>) {}
}
