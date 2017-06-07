use std::rc::Rc;
use std::cell::RefCell;

use Frame;
use WorldPoint;
use TouchReceiver;

#[derive(Clone, Copy)]
pub enum DragMode {
    StretchLow,
    StretchHigh,
    Drag,
}

pub struct DragFrame {
    pub vertical: DragMode,
    pub horizontal: DragMode,
    pub frame: Rc<RefCell<Frame>>,
    pub pos: WorldPoint,
}

impl TouchReceiver for DragFrame {
    fn continue_touch(mut self: Box<Self>, new_pos: WorldPoint) -> Option<Box<TouchReceiver>> {
        {
            let mut frame = self.frame.borrow_mut();
            let delta = new_pos - self.pos;

            fn drag(mode: DragMode, pos_val: &mut f64, size_val: &mut f64, delta: f64) {
                match mode {
                    DragMode::StretchLow => {
                        *size_val = (*size_val - delta).max(10.0);
                        *pos_val += delta * 0.5;
                    }
                    DragMode::Drag => {
                        *pos_val += delta;
                    }
                    DragMode::StretchHigh => {
                        *size_val = (*size_val + delta).max(10.0);
                        *pos_val += delta * 0.5;
                    }
                }
            }
            let mut pos = frame.pos;
            let mut size = frame.size;
            drag(self.vertical, &mut pos.y, &mut size.height, delta.y);
            drag(self.horizontal, &mut pos.x, &mut size.width, delta.x);
            frame.pos = pos;
            frame.size = size;
        }
        self.pos = new_pos;
        return Some(self);
    }
    fn end_touch(self: Box<Self>) {}
}
