use std::rc::Weak;
use std::cell::RefCell;

use menu::Action;
use WorldPoint;
use TouchReceiver;

pub struct MovePointAction {
    pub point: Weak<RefCell<WorldPoint>>,
}

impl Action for MovePointAction {
    fn start(self: Box<Self>, touch: WorldPoint) -> Option<Box<TouchReceiver>> {
        self.point.upgrade().and(Some(Box::new(MovePointTouchReceiver{
            point: self.point,
            last_touch: touch,
        })))
    }
}

pub struct MovePointTouchReceiver {
    pub point: Weak<RefCell<WorldPoint>>,
    pub last_touch: WorldPoint,
}

impl TouchReceiver for MovePointTouchReceiver {
    fn continue_touch(mut self: Box<Self>, touch: WorldPoint) -> Option<Box<TouchReceiver>> {
        match self.point.upgrade() {
            Some(point_rc) => {
                let mut point = point_rc.borrow_mut();
                let delta = touch - self.last_touch;
                *point = *point + delta;
                self.last_touch = touch;
                Some(self)
            },
            _ => None,
        }
    }
    fn end_touch(self: Box<Self>) {}
}
