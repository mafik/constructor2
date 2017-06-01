use std::rc::Weak;
use std::cell::RefCell;

use Vm;
use WorldPoint;
use TouchReceiver;

pub struct NavTouchReceiver {
    pub vm: Weak<RefCell<Vm>>,
    pub pos: WorldPoint,
}

impl TouchReceiver for NavTouchReceiver {
    fn continue_touch(&self, mut new_pos: WorldPoint) -> Option<Box<TouchReceiver>> {
        {
            let strong = self.vm.upgrade().unwrap();
            let mut vm = strong.borrow_mut();
            let delta = new_pos - self.pos;
            vm.center = vm.center + delta;
            new_pos = new_pos - delta;
        }
        Some(Box::new(NavTouchReceiver {
                          vm: self.vm.clone(),
                          pos: new_pos,
                      }))
    }
    fn end_touch(&self) {}
}
