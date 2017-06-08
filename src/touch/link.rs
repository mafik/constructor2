use std::rc::Rc;
use std::cell::RefCell;

use Link;
use WorldPoint;
use DisplayPoint;
use TouchReceiver;
use LinkTerminator;
use vm::Vm;

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
    fn continue_touch(mut self: Box<Self>,
                      vm: &mut Vm,
                      display: DisplayPoint,
                      new_pos: WorldPoint)
                      -> Option<Box<TouchReceiver>> {
        {
            let mut link = self.link.borrow_mut();
            match self.side {
                LinkSide::A => {
                    link.a = LinkTerminator::Point(new_pos);
                }
                LinkSide::B => {
                    link.b = LinkTerminator::Point(new_pos);
                }
            }
        }
        self.pos = new_pos;
        Some(self)
    }
    fn end_touch(self: Box<Self>, _: &mut Vm) {
        let mut link = self.link.borrow_mut();
        let blueprint_rc = link.blueprint.upgrade().unwrap();
        let mut blueprint = blueprint_rc.borrow_mut();
        let frame = blueprint.query_frame(self.pos);
        match frame {
            Some(frame) => {
                match self.side {
                    LinkSide::A => link.a = LinkTerminator::Frame(frame),
                    LinkSide::B => link.b = LinkTerminator::Frame(frame),
                }
            }
            None => {
                let i = blueprint
                    .links
                    .iter()
                    .enumerate()
                    .find(|tup| Rc::ptr_eq(&self.link, &tup.1))
                    .unwrap()
                    .0;
                blueprint.links.swap_remove(i);
            }
        }
    }
}
