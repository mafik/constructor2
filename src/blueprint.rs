use std::rc::{Rc, Weak};
use std::cell::RefCell;
extern crate serde;

use Vm;
use Frame;
use Link;
use machine::Machine;
use Object;
use WorldPoint;

pub struct Blueprint {
    name: String,
    pub frames: Vec<Rc<RefCell<Frame>>>,
    pub links: Vec<Rc<RefCell<Link>>>,
    pub machines: Vec<Rc<RefCell<Machine>>>,
    pub active_machine: Weak<RefCell<Machine>>,
}

use self::serde::ser::{Serialize, Serializer, SerializeSeq, SerializeStruct};

impl Serialize for Blueprint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut serializer = serializer.serialize_struct("Blueprint", 0)?;
        serializer.end()
        /*
        use std::ops::Deref;
        serializer.serialize_str(self.name.as_ref())?;
        let mut frame_seq = serializer.serialize_seq(Some(self.frames.len()))?;
        for frame in self.frames.iter() {
            let i = 42;
            frame_seq.serialize_element(&i)?;
        }
        frame_seq.end()
         */
    }
}

impl Blueprint {
    pub fn new(vm: &mut Vm, name: String, activate: bool) -> Rc<RefCell<Blueprint>> {
        let b = Rc::new(RefCell::new(Blueprint {
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
        self.frames
            .iter()
            .find(|frame_rc| frame_rc.borrow().hit_test(&p))
            .cloned()
    }

    pub fn frame_index(&self, frame: &Rc<RefCell<Frame>>) -> u32 {
        for (i, other) in self.frames.iter().enumerate() {
            if Rc::ptr_eq(frame, other) {
                return i as u32;
            }
        }
        panic!("Bad frame reference");
    }

    pub fn machine_index(&self, machine: &Rc<RefCell<Machine>>) -> u32 {
        for (i, other) in self.machines.iter().enumerate() {
            if Rc::ptr_eq(machine, other) {
                return i as u32;
            }
        }
        panic!("Bad machine reference");
    }
}
