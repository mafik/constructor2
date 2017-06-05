use std::rc::Weak;
use std::cell::RefCell;

use WorldPoint;
use TouchReceiver;

pub struct NavTouchReceiver {
    pub nav: Weak<RefCell<WorldPoint>>,
    pub last_pos: WorldPoint,
}

impl TouchReceiver for NavTouchReceiver {
    fn continue_touch(&self, new_pos: WorldPoint) -> Option<Box<TouchReceiver>> {
        if let Some(strong_nav) = self.nav.upgrade() {
            let mut nav = strong_nav.borrow_mut();
            let delta = new_pos - self.last_pos;
            *nav = *nav + delta;
            Some(Box::new(NavTouchReceiver {
                nav: self.nav.clone(),
                last_pos: new_pos - delta,
            }))
        } else { None }
    }
    fn end_touch(&self) {}
}
