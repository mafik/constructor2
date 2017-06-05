use std::rc::{Rc, Weak};
use std::cell::RefCell;

use Vm;
use Frame;
use Link;
use machine::Machine;
use Object;
use WorldPoint;

pub struct Blueprint {
    vm: Weak<RefCell<Vm>>,
    name: String,
    pub frames: Vec<Rc<RefCell<Frame>>>,
    pub links: Vec<Rc<RefCell<Link>>>,
    pub machines: Vec<Rc<RefCell<Machine>>>,
    pub active_machine: Weak<RefCell<Machine>>,
}

impl Blueprint {
    pub fn new(vm_cell: &Rc<RefCell<Vm>>, name: String, activate: bool) -> Rc<RefCell<Blueprint>> {
        let mut vm = vm_cell.borrow_mut();
        let b = Rc::new(RefCell::new(Blueprint {
                                         vm: Rc::downgrade(vm_cell),
                                         name: name,
                                         frames: Vec::new(),
                                         links: Vec::new(),
                                         machines: Vec::new(),
                                         active_machine: Weak::new(),
                                     }));
        if activate {
            vm.active_blueprint = Rc::downgrade(&b);
        }
        vm.blueprints.push(b.clone());
        return b;
    }

    /*
    Blueprint is a list of several elements drawn in a "draw-order".
    On mouse movement, the same elements are considered in a reverse-draw-order.
    Those elements are:
    - links
    - parameters
    - frames (objects)
    - UI toggles
     */

    pub fn with_object<F: FnMut(&mut Object)>(&self, frame_rc: &Rc<RefCell<Frame>>, mut f: F) {
        let machine_rc = if frame_rc.borrow().global {
            self.machines[0].clone()
        } else {
            self.active_machine.upgrade().unwrap()
        };
        machine_rc.borrow_mut().with_object(frame_rc, f);
    }

    pub fn query_frame(&self, p: WorldPoint) -> Option<Rc<RefCell<Frame>>> {
        self.frames.iter().find(|frame_rc| frame_rc.borrow().hit_test(&p)).cloned()
    }
}
