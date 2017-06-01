use std::rc::Rc;
use std::cell::RefCell;

use Link;
use WorldPoint;
use TouchReceiver;
use LinkTerminator;

#[derive(Clone, Copy)]
pub enum LinkSide {
    A,
    B,
}

pub struct DragLink {
    pub side: LinkSide,
    pub link: Rc<RefCell<Link>>,
    pub pos: WorldPoint,
}

impl TouchReceiver for DragLink {
    fn continue_touch(&self, new_pos: WorldPoint) -> Option<Box<TouchReceiver>> {
        {
            let mut link = self.link.borrow_mut();
            let delta = new_pos - self.pos;

            match self.side {
                LinkSide::A => {
                    link.a = LinkTerminator::Point(new_pos);
                }
                LinkSide::B => {
                    link.b = LinkTerminator::Point(new_pos);
                }
            }
        }
        Some(Box::new(DragLink {
                          side: self.side,
                          link: self.link.clone(),
                          pos: new_pos,
                      }))
    }
    fn end_touch(&self) {}
}
