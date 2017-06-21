use std::sync::Weak;
use std::cell::RefCell;

use menu::Action;
use WorldPoint;
use DisplayPoint;
use TouchReceiver;
use vm::Vm;
use std;

pub struct MovePointAction {
    point: Weak<RefCell<WorldPoint>>,
    stick: bool,
}

impl MovePointAction {
    pub fn new(point: Weak<RefCell<WorldPoint>>, stick: bool) -> MovePointAction {
        MovePointAction {
            point: point,
            stick: stick,
        }
    }
}

impl Action for MovePointAction {
    fn start(
        self: Box<Self>,
        vm: &mut Vm,
        display: DisplayPoint,
        world: WorldPoint,
    ) -> Option<Box<TouchReceiver>> {
        let p = self.point.upgrade();
        if p.is_none() {
            None
        } else {
            let p = p.unwrap();
            let stick = self.stick;
            Some(Box::new(MovePointTouchReceiver {
                point: self.point,
                last_touch: world,
                initial_point: if stick {
                    Some(p.borrow().clone())
                } else {
                    None
                },
            }))
        }
    }
}

pub struct MovePointTouchReceiver {
    point: Weak<RefCell<WorldPoint>>,
    last_touch: WorldPoint,
    initial_point: Option<WorldPoint>,
}

impl TouchReceiver for MovePointTouchReceiver {
    fn continue_touch(
        mut self: Box<Self>,
        vm: &mut Vm,
        display: DisplayPoint,
        world: WorldPoint,
    ) -> Option<Box<TouchReceiver>> {
        match self.point.upgrade() {
            Some(point_rc) => {
                let mut point = point_rc.borrow_mut();
                let delta = world - self.last_touch;
                match self.initial_point {
                    Some(initial_point) => {
                        *point = *point + delta;
                    }
                    None => {
                        *point = *point + delta;
                        self.last_touch = world;
                    }
                }
                Some(self)
            }
            _ => None,
        }
    }
    fn end_touch(self: Box<Self>, _: &mut Vm) {}
}
