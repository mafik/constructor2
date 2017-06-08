//use std::any::Any;
use std::rc::{Rc, Weak};
use std::cell::RefCell;

use blueprint::Blueprint;
use Object;
use Frame;

pub struct Machine {
    blueprint: Weak<RefCell<Blueprint>>,
    pub objects: Vec<Rc<RefCell<Object>>>,
}

impl Machine {
    pub fn new(blueprint: &Rc<RefCell<Blueprint>>, activate: bool) -> Rc<RefCell<Machine>> {
        let m = Rc::new(RefCell::new(Machine {
                                         blueprint: Rc::downgrade(blueprint),
                                         objects: Vec::new(),
                                     }));
        // TODO: init all objects (from blueprint frames)
        let mut blueprint = blueprint.borrow_mut();
        if activate {
            blueprint.active_machine = Rc::downgrade(&m);
        };
        blueprint.machines.push(m.clone());
        return m;
    }

    pub fn push(&mut self, object: Object) {
        self.objects.push(Rc::new(RefCell::new(object)));
    }

    pub fn get_object(&self, frame_rc: &Rc<RefCell<Frame>>) -> Rc<RefCell<Object>> {
        self.objects
            .iter()
            .find(|o| Rc::ptr_eq(&o.borrow().frame, frame_rc))
            .unwrap()
            .clone()
    }

    pub fn with_object<F: FnMut(&mut Object)>(&mut self, frame_rc: &Rc<RefCell<Frame>>, mut f: F) {
        let object = self.objects
            .iter_mut()
            .find(|o| Rc::ptr_eq(&o.borrow().frame, frame_rc))
            .unwrap();
        use std::ops::DerefMut;
        f(object.borrow_mut().deref_mut());
    }
}
