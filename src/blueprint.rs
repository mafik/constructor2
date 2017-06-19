use std::rc::{Rc, Weak};
use std::cell::RefCell;
extern crate serde;
extern crate serde_json;

use Vm;
use Frame;
use Link;
use machine::Machine;
use Object;
use WorldPoint;
use WorldSize;
use SerializableVec;

pub struct Blueprint {
    vm: Weak<RefCell<Vm>>,
    pub name: String,
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
        let mut serializer = serializer.serialize_struct("Blueprint", 4)?;
        serializer.serialize_field("name", &self.name)?;
        serializer.serialize_field("frames", &SerializableVec(&self.frames))?;
        serializer.serialize_field("links", &SerializableVec(&self.links))?;
        serializer.serialize_field("machines", &SerializableVec(&self.machines))?;
        serializer.serialize_field("active_machine", &self.machine_index(&self.active_machine.upgrade().unwrap()))?;
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
    pub fn new(vm: &Rc<RefCell<Vm>>) -> Rc<RefCell<Blueprint>> {
        let bp = Rc::new(RefCell::new(Blueprint {
            vm: Rc::downgrade(vm),
            name: String::new(),
            frames: Vec::new(),
            links: Vec::new(),
            machines: Vec::new(),
            active_machine: Weak::new(),
        }));
        vm.borrow_mut().blueprints.push(bp.clone());
        return bp;
    }

    pub fn load_json(this: &Rc<RefCell<Blueprint>>, json: &serde_json::Value) {
        let blueprint_rc = this;
        let frames = json.get("frames").unwrap().as_array().unwrap();
        for frame_json in frames.iter() {
            let type_name = frame_json.get("type").unwrap().as_str().unwrap();
            let global = frame_json.get("global").unwrap().as_bool().unwrap();
            let vm_rc = blueprint_rc.borrow().vm.upgrade().unwrap();
            let vm = vm_rc.borrow();
            let typ = vm.types.iter().find(|typ| typ.name == type_name).unwrap();
            let frame = Frame::new(typ, &blueprint_rc, global);
            let pos_array = frame_json.get("pos").unwrap().as_array().unwrap();
            frame.borrow_mut().pos = WorldPoint::new(
                pos_array[0].as_f64().unwrap(),
                pos_array[1].as_f64().unwrap(),
            );
            let size_array = frame_json.get("size").unwrap().as_array().unwrap();
            frame.borrow_mut().size = WorldSize::new(
                size_array[0].as_f64().unwrap(),
                size_array[1].as_f64().unwrap(),
            );
        }

        let name = json.get("name").unwrap().as_str().unwrap();
        blueprint_rc.borrow_mut().name = String::from(name);

        let links = json.get("links").unwrap().as_array().unwrap();
        for link_json in links.iter() {

            use LinkTerminator;
            fn parse_terminator(blueprint: &Blueprint, link_json: &serde_json::Value, side: &str) -> LinkTerminator {
                let a = link_json.get(side).unwrap().as_object().unwrap();
                let terminator_type = a.keys().next().unwrap();
                use FrameParam;
                match terminator_type.as_ref() {
                    "Frame" => {
                        let frame_idx = a.get("Frame").unwrap().as_array().unwrap()[0].as_u64().unwrap();
                        LinkTerminator::Frame(blueprint.frames[frame_idx as usize].clone())
                    },
                    "FrameParam" => {
                        let ref frame_param = a.get("FrameParam").unwrap().as_array().unwrap()[0];
                        let frame_idx = frame_param.get("frame").unwrap().as_u64().unwrap().clone();
                        let param_index = frame_param.get("param_index").unwrap().as_u64().unwrap() as usize;
                        LinkTerminator::FrameParam(FrameParam {
                            frame: blueprint.frames[frame_idx as usize].clone(),
                            param_index: param_index,
                        })
                    },
                    _ => panic!("Unknown LinkTerminator type"),
                }
            }
            use std::ops::Deref;
            let mut bp = blueprint_rc.borrow_mut();
            let order = link_json.get("order").unwrap().as_i64().unwrap();
            let link = Link {
                blueprint: Rc::downgrade(blueprint_rc),
                a: parse_terminator(bp.deref(), &link_json, "a"),
                b: parse_terminator(bp.deref(), &link_json, "b"),
                order: order as i32,
            };
            bp.links.push(Rc::new(RefCell::new(link)));
        }
        let machines = json.get("machines").unwrap().as_array().unwrap();
        for machine_json in machines.iter() {
            let machine = Machine::new(&blueprint_rc);
            Machine::load_json(&machine, machine_json);
            blueprint_rc.borrow_mut().machines.push(machine);
        }
        let active_machine = json.get("active_machine").unwrap().as_i64().unwrap();
        let weak = Rc::downgrade(&blueprint_rc.borrow().machines[active_machine as usize]);
        blueprint_rc.borrow_mut().active_machine = weak;
    }

    pub fn rename(&mut self, name: String) {
        self.name = name;
    }

    pub fn activate(&mut self, machine: &Rc<RefCell<Machine>>) {
        self.active_machine = Rc::downgrade(machine);
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
