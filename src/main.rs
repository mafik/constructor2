/*
TODOs:
- Pass the results of running process as events back to the main thread (DONE)


- Implement multiple blueprints


- Top-level blueprint instances hold global variables
- Blueprints are defined as part of the VM
- What about closures? "local blueprints" need to be scoped to avoid polluting global namespace
- Future: add locally defined blueprints (closures) - able to refer to the parent local variables as if they were globals
- Now: all blueprints are defined at top level

- All blueprints should be closures - this would make the implementation simpler.
- A blueprint can be added in another blueprint as a frame (it will be replicated for each machine if it's a local frame).
- A blueprint can use blueprints from the same or parent scopes
- Local blueprints can exist only within the scope of parent blueprint

- Future: provide "interfaces" / "contracts" / "protocols" - ability to pass and use instance of any blueprint


On hold:
- Cleanups - a - lot
- Menu improvements (draw background below menu entry name)
- Multiple mouse handlers at the same time
- Performance monitoring
- Interactive error reporting
- Move per-client parameters to separate struct
- Serve Iosevka from hyper
- Support concurrent access from many clients (different dpi and viewports)
*/

extern crate hyper;
extern crate euclid;
extern crate serde;
extern crate serde_json;

mod http;
mod canvas;
mod json_canvas;
mod touch;
mod machine;
mod blueprint;
mod vm;
mod event;
mod menu;
mod process;

use std::time::Instant;
use std::thread;
use std::sync::mpsc;
use std::any::Any;
use std::sync::{Arc, Weak};
use std::cell::{RefCell, Ref, RefMut};
use std::ops::Deref;
use std::f64::consts::PI;

use canvas::*;
use json_canvas::*;
use euclid::*;

use touch::*;
use machine::*;
use blueprint::*;
use vm::*;
use menu::*;

use serde::ser::{Serialize, Serializer, SerializeSeq, SerializeStruct, SerializeTuple,
                 SerializeTupleVariant};

struct SerializableVec<'a, T: 'a + Serialize>(&'a Vec<Arc<RefCell<T>>>);

impl<'a, T: Serialize> Serialize for SerializableVec<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use std::ops::Deref;
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for (i, b) in self.0.iter().enumerate() {
            seq.serialize_element(b.borrow().deref())?;
        }
        seq.end()

    }
}

struct SerializablePoint2D<'a, T: 'a>(&'a TypedPoint2D<f64, T>);

impl<'a, T: 'a> Serialize for SerializablePoint2D<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(2)?;
        tup.serialize_element(&self.0.x)?;
        tup.serialize_element(&self.0.y)?;
        tup.end()
    }
}

struct SerializableSize2D<'a, T: 'a>(&'a TypedSize2D<f64, T>);

impl<'a, T: 'a> Serialize for SerializableSize2D<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(2)?;
        tup.serialize_element(&self.0.width)?;
        tup.serialize_element(&self.0.height)?;
        tup.end()
    }
}



pub struct DisplayMillimetreSpace; // milimeters @ half meter
pub struct WorldMillimetreSpace; // above * viewport zoom + viewport offset
pub struct DisplayPixelSpace; // display mm * pixel density [px/mm]

pub type WorldPoint = TypedPoint2D<f64, WorldMillimetreSpace>;
pub type DisplayPoint = TypedPoint2D<f64, DisplayMillimetreSpace>;
pub type WorldSize = TypedSize2D<f64, WorldMillimetreSpace>;
pub type PixelPoint = TypedPoint2D<f64, DisplayPixelSpace>;

const MM_PER_INCH: f64 = 25.4;

struct Display {
    size: PixelPoint,
    dpi: f64,
    eye_distance_meters: f64,
}

impl Display {
    fn pixel_size(&self) -> ScaleFactor<f64, DisplayPixelSpace, DisplayMillimetreSpace> {
        ScaleFactor::new(MM_PER_INCH / self.dpi) * self.distance_factor()
    }
    fn distance_factor(&self) -> ScaleFactor<f64, DisplayMillimetreSpace, DisplayMillimetreSpace> {
        ScaleFactor::new(0.5 / self.eye_distance_meters)
    }
    fn to_millimetre(&self, point: PixelPoint) -> DisplayPoint {
        let s = self.pixel_size();
        (point - self.size * 0.5) * s
    }
    //fn to_pixel(&self, point: DisplayPoint) -> PixelPoint {
    //    let s = self.pixel_size().inv();
    //    point * s + self.size * 0.5
    //}
    fn setup_canvas(&self, canvas: &mut Canvas) {}
}

trait TouchReceiver {
    fn continue_touch(
        self: Box<Self>,
        vm: &mut Vm,
        display: DisplayPoint,
        world: WorldPoint,
    ) -> Option<Box<TouchReceiver>>;
    fn end_touch(self: Box<Self>, &mut Vm);
}

// TODO: rename to VisibleLayer
trait Visible {
    fn draw(&self, c: &mut Canvas);
    fn make_menu(&self, d: DisplayPoint, w: WorldPoint) -> Option<Menu>;
}

#[derive(Clone)]
pub struct FrameParam {
    frame: Arc<RefCell<Frame>>,
    param_index: usize,
}

impl FrameParam {
    fn center(&self) -> WorldPoint {
        let frame = self.frame.borrow();
        frame.pos +
            WorldPoint::new(
                PARAM_RADIUS - frame.size.width * 0.5,
                frame.size.height * 0.5 + -PARAM_RADIUS +
                    (PARAM_RADIUS * 2. + PARAM_SPACING) * (self.param_index as f64 + 1.),
            )
    }
}

impl Visible for FrameParam {
    fn draw(&self, c: &mut Canvas) {
        let center = self.center();
        let param = &self.frame.borrow().typ.parameters[self.param_index];
        c.fillStyle("white");
        c.fillCircle(center.x, center.y, PARAM_RADIUS);
        c.fillStyle("black");
        c.fillText(
            param.name,
            center.x + PARAM_RADIUS + PARAM_SPACING,
            center.y,
        );
    }
    fn make_menu(&self, d: DisplayPoint, w: WorldPoint) -> Option<Menu> {
        let center = self.center();
        let q = w - center;
        let dist = q.dot(q).sqrt();
        if dist < PARAM_RADIUS {
            Some(Menu {
                entries: vec![
                    Entry {
                        name: "Connect".to_string(),
                        color: None,
                        shortcuts: vec!["LMB".to_string()],
                        action: Box::new(ConnectParamAction::new(self)),
                    },
                ],
                color: "#888".to_string(),
            })
        } else {
            None
        }
    }
}

struct ConnectParamAction {
    frame_param: FrameParam,
}

impl ConnectParamAction {
    fn new(frame_param: &FrameParam) -> ConnectParamAction {
        ConnectParamAction { frame_param: frame_param.clone() }
    }
}

impl Action for ConnectParamAction {
    fn start(
        self: Box<Self>,
        _: &mut Vm,
        d: DisplayPoint,
        w: WorldPoint,
    ) -> Option<Box<TouchReceiver>> {
        let frame_param = (*self).frame_param;
        let blueprint_weak = frame_param.frame.borrow().blueprint.clone();
        let blueprint_rc = blueprint_weak.upgrade().unwrap();
        let mut blueprint = blueprint_rc.borrow_mut();
        let link_rc = Arc::new(RefCell::new(Link {
            blueprint: blueprint_weak,
            a: LinkTerminator::FrameParam(frame_param),
            b: LinkTerminator::Point(w),
            order: 0,
        }));
        blueprint.links.push(link_rc.clone());
        Some(Box::new(DragLink {
            side: LinkSide::B,
            link: link_rc,
            pos: w,
        }))
    }
}

impl Visible for Arc<RefCell<Frame>> {
    fn draw(&self, c: &mut Canvas) {
        let frame = self.borrow();
        if frame.typ.parameters.len() > 0 {
            c.strokeStyle("#888");
            c.beginPath();
            let last_frame_param = FrameParam {
                frame: self.clone(),
                param_index: frame.typ.parameters.len() - 1,
            };
            let last_param_center = last_frame_param.center();
            c.moveTo(last_param_center.x, last_param_center.y);
            let end = frame.box_cast(&last_param_center);
            c.lineTo(end.x, end.y);
            c.stroke();
        }
        for param_index in 0..frame.typ.parameters.len() {
            let frame_param = FrameParam {
                frame: self.clone(),
                param_index: param_index,
            };
            frame_param.draw(c);
        }
        c.translate(frame.pos.x, frame.pos.y);
        c.fillStyle("white");
        c.translate(-frame.size.width / 2., -frame.size.height / 2.);
        c.fillRect(0., 0., frame.size.width, frame.size.height);
        c.fillStyle("black");
        c.fillText(frame.typ.name, 0., 0.);
        c.beginPath();
        c.rect(0., 0., frame.size.width, frame.size.height);
        c.clip();
        let blueprint_rc = frame.blueprint.upgrade().unwrap();
        let blueprint = blueprint_rc.borrow();
        blueprint.with_object(self, |o| { (frame.typ.draw)(o, c); });
    }
    fn make_menu(&self, d: DisplayPoint, w: WorldPoint) -> Option<Menu> {
        let mut q;
        let mut s;
        let mut param_count;
        {
            let frame = self.borrow();
            q = w - frame.pos;
            s = frame.size * 0.5;
            param_count = frame.typ.parameters.len();
        }
        fn range_check(x: f64, range: f64) -> bool {
            x < range && x > -range
        }
        let hit = range_check(q.x, s.width) && range_check(q.y, s.height);

        if hit {
            let s2 = s * 0.5;
            fn choose_drag_mode(x: f64, range: f64) -> DragMode {
                if x < -range {
                    DragMode::StretchLow
                } else if x > range {
                    DragMode::StretchHigh
                } else {
                    DragMode::Drag
                }
            }
            let horizontal = choose_drag_mode(q.x, s2.width);
            let vertical = choose_drag_mode(q.y, s2.height);
            let name = if horizontal == DragMode::Drag && vertical == DragMode::Drag {
                "Move"
            } else {
                "Resize"
            }.to_string();
            Some(Menu {
                entries: vec![
                    Entry {
                        name: name,
                        color: None,
                        shortcuts: vec!["LMB".to_string()],
                        action: Box::new(DragFrameAction::new(self, horizontal, vertical)),
                    },
                    Entry {
                        name: "Run".to_string(),
                        color: None,
                        shortcuts: vec!["Space".to_string()],
                        action: Box::new(RunAction::new(self)),
                    },
                    Entry {
                        name: "Delete".to_string(),
                        color: None,
                        shortcuts: vec!["Delete".to_string()],
                        action: Box::new(DeleteFrameAction::new(self)),
                    },
                ],
                color: "#888".to_string(),
            })
        } else {
            for param_index in 0..param_count {
                let frame_param = FrameParam {
                    frame: self.clone(),
                    param_index: param_index,
                };
                let menu = frame_param.make_menu(d, w);
                if menu.is_some() {
                    return menu;
                }
            }
            None

        }
    }
}

struct RunAction {
    frame: Weak<RefCell<Frame>>,
}

impl RunAction {
    fn new(frame: &Arc<RefCell<Frame>>) -> RunAction {
        RunAction { frame: Arc::downgrade(frame) }
    }
}

impl Action for RunAction {
    fn start(
        self: Box<Self>,
        vm: &mut Vm,
        d: DisplayPoint,
        w: WorldPoint,
    ) -> Option<Box<TouchReceiver>> {
        println!("Running frame!");
        let frame = (*self).frame.upgrade();
        if frame.is_none() {
            return None;
        }
        let frame = frame.unwrap();
        let blueprint = frame.borrow().blueprint.upgrade();
        if blueprint.is_none() {
            return None;
        }
        let blueprint = blueprint.unwrap();
        let object = Arc::downgrade(&blueprint.borrow().get_object(&frame));
        vm.tasks.push_back(object);
        None
    }
}

struct AddFrameAction {
    typ: &'static Type,
}

impl AddFrameAction {
    fn new(typ: &'static Type) -> AddFrameAction {
        AddFrameAction { typ: typ }
    }
}

impl Action for AddFrameAction {
    fn start(
        self: Box<Self>,
        vm: &mut Vm,
        d: DisplayPoint,
        w: WorldPoint,
    ) -> Option<Box<TouchReceiver>> {
        let blueprint = vm.active_blueprint.upgrade().unwrap();
        let frame = Frame::new(self.typ, &blueprint, true);
        frame.borrow_mut().pos = w;

        Box::new(DragFrameAction::new(&frame, DragMode::Drag, DragMode::Drag)).start(vm, d, w)
    }
}

struct DeleteFrameAction {
    frame: Weak<RefCell<Frame>>,
}

impl DeleteFrameAction {
    fn new(frame: &Arc<RefCell<Frame>>) -> DeleteFrameAction {
        DeleteFrameAction { frame: Arc::downgrade(frame) }
    }
}

impl Action for DeleteFrameAction {
    fn start(
        self: Box<Self>,
        _: &mut Vm,
        d: DisplayPoint,
        w: WorldPoint,
    ) -> Option<Box<TouchReceiver>> {
        println!("Deleting frame!");
        let frame = (*self).frame.upgrade();
        if frame.is_none() {
            return None;
        }
        let frame = frame.unwrap();
        let blueprint = frame.borrow().blueprint.upgrade();
        if let Some(blueprint) = blueprint {
            let mut blueprint = blueprint.borrow_mut();
            if let Some((index, _)) = blueprint.frames.iter().enumerate().find(|x| {
                Arc::ptr_eq(x.1, &frame)
            })
            {
                blueprint.frames.swap_remove(index);

                blueprint.links.retain(|link_rc| {
                    fn side_retain(f: &Arc<RefCell<Frame>>, t: &LinkTerminator) -> bool {
                        match t {
                            &LinkTerminator::Frame(ref other_frame) => !Arc::ptr_eq(f, other_frame),
                            &LinkTerminator::FrameParam(ref frame_param) => {
                                !Arc::ptr_eq(f, &frame_param.frame)
                            }
                            _ => true,
                        }
                    }
                    let link = link_rc.borrow();
                    return side_retain(&frame, &link.a) && side_retain(&frame, &link.b);
                });
                for machine in blueprint.machines.iter() {
                    let mut machine = machine.borrow_mut();
                    machine.objects.retain(|o_rc| {
                        !Arc::ptr_eq(&o_rc.borrow().frame, &frame)
                    });
                }
                let frame = Arc::try_unwrap(frame).ok().unwrap();
            }
        }
        None
    }
}

struct DragFrameAction {
    frame: Weak<RefCell<Frame>>,
    horizontal: DragMode,
    vertical: DragMode,
}

impl DragFrameAction {
    fn new(
        frame_rc: &Arc<RefCell<Frame>>,
        horizontal: DragMode,
        vertical: DragMode,
    ) -> DragFrameAction {
        DragFrameAction {
            frame: Arc::downgrade(frame_rc),
            horizontal: horizontal,
            vertical: vertical,
        }
    }
}

impl Action for DragFrameAction {
    fn start(
        self: Box<Self>,
        _: &mut Vm,
        d: DisplayPoint,
        w: WorldPoint,
    ) -> Option<Box<TouchReceiver>> {
        Some(Box::new(DragFrame {
            horizontal: self.horizontal,
            vertical: self.vertical,
            frame: (*self).frame,
            pos: w,
        }))
    }
}

pub struct Frame {
    blueprint: Weak<RefCell<Blueprint>>,
    typ: &'static Type,
    pos: WorldPoint,
    size: WorldSize,
    global: bool,
}

impl Serialize for Frame {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Frame", 4)?;
        s.serialize_field("type", &self.typ.name)?;
        s.serialize_field("pos", &SerializablePoint2D(&self.pos))?;
        s.serialize_field("size", &SerializableSize2D(&self.size))?;
        s.serialize_field("global", &self.global)?;
        s.end()
    }
}

const PARAM_RADIUS: f64 = 5.;
const PARAM_SPACING: f64 = 2.;

impl Frame {
    fn new(
        typ: &'static Type,
        blueprint: &Arc<RefCell<Blueprint>>,
        global: bool,
    ) -> Arc<RefCell<Frame>> {
        let f = Arc::new(RefCell::new(Frame {
            blueprint: Arc::downgrade(blueprint),
            typ: typ,
            pos: WorldPoint::zero(),
            size: WorldSize::new(10., 10.),
            global: global,
        }));
        blueprint.borrow_mut().frames.push(f.clone());
        for machine_cell in blueprint.borrow().machines.iter() {
            let mut machine = machine_cell.borrow_mut();
            let mut object = Object {
                machine: Arc::downgrade(machine_cell),
                frame: f.clone(),
                execute: false,
                data: Box::new(()),
            };
            (typ.init)(&mut object);
            machine.push(object);
            if global {
                break;
            }
        }
        return f;
    }
    fn hit_test(&self, p: &WorldPoint) -> bool {
        let q = *p - self.pos;
        let s = self.size * 0.5;
        fn range_check(x: f64, range: f64) -> bool {
            x < range && x > -range
        }
        range_check(q.x, s.width) && range_check(q.y, s.height)
    }
    fn box_cast(&self, p: &WorldPoint) -> WorldPoint {
        let p = *p - self.pos;
        let s = self.size * 0.5;
        let clamp = |value| if value < -1. {
            -1.
        } else if value > 1. {
            1.
        } else {
            value
        };
        let (x, y) = (p.x / s.width, p.y / s.height);
        let (x, y) = (clamp(x), clamp(y));
        WorldPoint::new(x * s.width, y * s.height) + self.pos
    }
}

impl Serialize for FrameParam {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use std::ops::Deref;
        let mut s = serializer.serialize_struct("FrameParam", 2)?;
        let frame = self.frame.borrow();
        let blueprint = frame.blueprint.upgrade().unwrap();
        let blueprint = blueprint.borrow();
        s.serialize_field(
            "frame",
            &blueprint.frame_index(&self.frame),
        )?;
        s.serialize_field("param_index", &self.param_index)?;
        s.end()
    }
}

pub enum LinkTerminator {
    Frame(Arc<RefCell<Frame>>),
    FrameParam(FrameParam),
    Point(WorldPoint),
}

impl Serialize for LinkTerminator {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use std::ops::Deref;
        match self {
            &LinkTerminator::Point(ref point) => {
                let mut s = serializer.serialize_tuple_variant(
                    "LinkTerminator",
                    2,
                    "Point",
                    1,
                )?;
                s.serialize_field(&SerializablePoint2D(point))?;
                s.end()
            }
            &LinkTerminator::Frame(ref frame_rc) => {
                let mut s = serializer.serialize_tuple_variant(
                    "LinkTerminator",
                    0,
                    "Frame",
                    1,
                )?;
                s.serialize_field(&SerializeFrameIndex(frame_rc))?;
                s.end()
            }
            &LinkTerminator::FrameParam(ref frame_param) => {
                let mut s = serializer.serialize_tuple_variant(
                    "LinkTerminator",
                    1,
                    "FrameParam",
                    1,
                )?;
                s.serialize_field(frame_param)?;
                s.end()
            }
        }
    }
}

impl LinkTerminator {
    fn get_pos_quick(&self) -> WorldPoint {
        match self {
            &LinkTerminator::Frame(ref frame) => frame.borrow().pos,
            &LinkTerminator::FrameParam(ref frame_param) => frame_param.center(),
            &LinkTerminator::Point(point) => point,
        }
    }
    fn get_pos(&self, other: &LinkTerminator) -> WorldPoint {
        match self {
            &LinkTerminator::Frame(ref frame) => frame.borrow().box_cast(&other.get_pos_quick()),
            _ => self.get_pos_quick(),
        }
    }
}

pub struct Link {
    blueprint: Weak<RefCell<Blueprint>>,
    a: LinkTerminator,
    b: LinkTerminator,
    order: i32,
}

impl Serialize for Link {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Link", 3)?;
        s.serialize_field("a", &self.a)?;
        s.serialize_field("b", &self.b)?;
        s.serialize_field("order", &self.order)?;
        s.end()
    }
}

impl Visible for Arc<RefCell<Link>> {
    fn draw(&self, c: &mut Canvas) {
        let link = self.borrow();
        let start = link.a.get_pos(&link.b);
        let end = link.b.get_pos(&link.a);
        let v = start - end;
        let length2 = v.dot(v);
        let length = length2.sqrt();
        let angle = (-v.y).atan2(-v.x);

        c.save();
        c.translate(start.x, start.y);
        c.rotate(angle);
        c.fillStyle("#000");
        c.fillCircle(0., 0., PARAM_RADIUS * 0.5);

        const ARROW_WIDTH: f64 = PARAM_RADIUS * 0.5;
        const ARROW_LENGTH: f64 = 5.0;

        c.strokeStyle("#000");
        c.beginPath();
        c.moveTo(0., 0.);
        c.lineTo(length - ARROW_LENGTH * 0.5, 0.);
        c.stroke();

        let vlen = length.max(PARAM_RADIUS * 2.0) * 0.05;
        let tanh = vlen.tanh();
        let r = ARROW_WIDTH / (vlen + 1.0);
        let l = ARROW_LENGTH * tanh;
        let l = l.max(r + 1.0);
        let a = PI * 0.5 * (1.0 - tanh);

        c.beginPath();
        c.ellipse(
            length - l,
            0.,
            r,
            ARROW_WIDTH,
            0.,
            PI * 0.5 - a,
            PI * 1.5 + a,
            false,
        );
        c.lineTo(length, 0.);
        c.fill();

        c.restore();
    }
    fn make_menu(&self, d: DisplayPoint, w: WorldPoint) -> Option<Menu> {
        None
    }
}

pub struct Object {
    machine: Weak<RefCell<Machine>>,
    frame: Arc<RefCell<Frame>>,
    execute: bool,
    data: Box<Any>,
}

impl Object {
    pub fn typ(&self) -> &'static Type {
        let frame = self.frame.borrow();
        frame.typ
    }
}

struct SerializeFrameIndex<'a>(&'a Arc<RefCell<Frame>>);

impl<'a> Serialize for SerializeFrameIndex<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let frame = self.0.borrow();
        let blueprint = frame.blueprint.upgrade().unwrap();
        let blueprint = blueprint.borrow();
        serializer.serialize_some(&blueprint.frame_index(self.0))
    }
}

impl Serialize for Object {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Object", 3)?;
        s.serialize_field(
            "frame",
            &SerializeFrameIndex(&self.frame),
        )?;
        s.serialize_field("execute", &self.execute);
        let data = (self.frame.borrow().typ.serialize)(self);
        s.serialize_field("data", &data);
        s.end()
    }
}

type ObjectCell = Arc<RefCell<Object>>;
type RunArg = Vec<ObjectCell>;
type RunArgs = Vec<RunArg>;

struct Parameter {
    name: &'static str,
    runnable: bool,
    output: bool,
}

pub struct Type {
    name: &'static str,
    parameters: &'static [Parameter],
    init: &'static (Fn(&mut Object) + Sync),
    run: &'static (Fn(&mut Vm, &ObjectCell, RunArgs) + Sync),
    update: Option<&'static (Fn(&mut Vm, &ObjectCell, Box<Any + Send>) + Sync)>,
    draw: &'static (Fn(&Object, &mut Canvas) + Sync),
    serialize: &'static (Fn(&Object) -> Vec<u8> + Sync),
    deserialize: &'static (Fn(&mut Object, Vec<u8>) + Sync),
}

static text_type: Type = Type {
    name: "Text",
    parameters: &[],
    init: &|o: &mut Object| { o.data = Box::new("".to_string()); },
    run: &|vm: &mut Vm, o: &ObjectCell, args: RunArgs| {},
    update: None,
    draw: &|o: &Object, canvas: &mut Canvas| {
        let font_metrics = canvas.get_font_metrics(6.);

        canvas.fillStyle("black");
        canvas.fillText(
            o.data.downcast_ref::<String>().unwrap(),
            2.,
            2. + font_metrics.ascent as f64,
        );
    },
    serialize: &|o: &Object| -> Vec<u8> {
        o.data
            .downcast_ref::<String>()
            .unwrap()
            .clone()
            .into_bytes()
    },
    deserialize: &|o: &mut Object, data: Vec<u8>| {
        o.data = Box::new(String::from_utf8(data).ok().unwrap());
    },
};

static empty_type: Type = Type {
    name: "Empty",
    parameters: &[],
    init: &|o: &mut Object| {},
    run: &|vm: &mut Vm, o: &ObjectCell, args: RunArgs| {},
    update: None,
    draw: &|o: &Object, canvas: &mut Canvas| {},
    serialize: &|o: &Object| -> Vec<u8> { Vec::new() },
    deserialize: &|o: &mut Object, data: Vec<u8>| {},
};

fn new_text(
    blueprint_rc: &Arc<RefCell<Blueprint>>,
    text: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {

    let frame_rc = Frame::new(&text_type, &blueprint_rc, true);
    let mut frame = frame_rc.borrow_mut();
    frame.pos = WorldPoint::new(x, y);
    frame.size = WorldSize::new(width, height);
    let blueprint = blueprint_rc.borrow();
    let machine_rc = blueprint.active_machine.upgrade().unwrap();
    let object_rc = machine_rc.borrow().get_object(&frame_rc);
    object_rc.borrow_mut().data = Box::new(text.to_string());
}

fn main() {
    let mut vm = Vm::new();
    match Vm::load_json(&vm) {
        Result::Ok(_) => (),
        Result::Err(_) => {
            let blueprint = Blueprint::new(&vm);
            blueprint.borrow_mut().rename("Default".to_string());
            vm.borrow_mut().activate(&blueprint);
            let machine = Machine::new(&blueprint);
            blueprint.borrow_mut().activate(&machine);
        }
    }

    vm.borrow_mut().run();
}
