//use std::any::Any;
extern crate serde_json;

use std::sync::{Arc, Weak};
use std::cell::RefCell;

use blueprint::Blueprint;
use Object;
use Frame;

pub struct Machine {
    pub blueprint: Weak<RefCell<Blueprint>>,
    pub objects: Vec<Arc<RefCell<Object>>>,
}

use serde::ser::{Serialize, Serializer};
use SerializableVec;

impl Serialize for Machine {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_some(&SerializableVec(&self.objects))
    }
}

impl Machine {
    pub fn new(blueprint: &Arc<RefCell<Blueprint>>) -> Arc<RefCell<Machine>> {
        // TODO: initialize objects
        let machine = Arc::new(RefCell::new(Machine {
            blueprint: Arc::downgrade(blueprint),
            objects: Vec::new(),
        }));

        for frame_rc in blueprint.borrow().frames.iter() {
            let frame = frame_rc.borrow();
            if frame.global {
                continue;
            }
            let mut object = Object {
                machine: Arc::downgrade(&machine),
                frame: frame_rc.clone(),
                execute: false,
                data: Box::new(()),
            };
            (frame.typ.init)(&mut object);
            machine.borrow_mut().push(object);
        }

        blueprint.borrow_mut().machines.push(machine.clone());
        return machine;
    }

    pub fn load_json(this: &Arc<RefCell<Machine>>, json: &serde_json::Value) {
        let arr = json.as_array().unwrap();
        let mut machine = this.borrow_mut();
        let bp_rc = machine.blueprint.upgrade().unwrap();
        let bp = bp_rc.borrow();
        for object_json in arr.iter() {
            let frame_idx = object_json.get("frame").unwrap().as_u64().unwrap();
            let frame_rc = bp.frames[frame_idx as usize].clone();
            let typ = frame_rc.borrow().typ;
            let execute = object_json.get("execute").unwrap().as_bool().unwrap();
            let mut object = Object {
                machine: Arc::downgrade(this),
                frame: frame_rc,
                execute: execute,
                data: Box::new(()),
            };
            let data = object_json.get("data").unwrap();
            let data = serde_json::from_value(data.clone()).ok().unwrap();
            (typ.deserialize)(&mut object, data);
            machine.push(object);
        }
    }

    pub fn push(&mut self, object: Object) {
        self.objects.push(Arc::new(RefCell::new(object)));
    }

    pub fn get_object(&self, frame_rc: &Arc<RefCell<Frame>>) -> Arc<RefCell<Object>> {
        self.objects
            .iter()
            .find(|o| Arc::ptr_eq(&o.borrow().frame, frame_rc))
            .unwrap()
            .clone()
    }

    pub fn with_object<F: FnMut(&mut Object)>(&mut self, frame_rc: &Arc<RefCell<Frame>>, mut f: F) {
        let object = self.objects
            .iter_mut()
            .find(|o| Arc::ptr_eq(&o.borrow().frame, frame_rc))
            .unwrap();
        use std::ops::DerefMut;
        f(object.borrow_mut().deref_mut());
    }
}
