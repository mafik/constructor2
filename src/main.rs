/*
TODOs:
- move per-client parameters to separate struct

Milestones:
1. Object that runs a command on click (DONE)
2. Command is fetched from another object
3. Command is fetched from several other objects
4. String objects are editable

On hold:
- Serve Iosevka from hyper
- Support concurrent access from many clients (different dpi and viewports)
*/

extern crate hyper;
extern crate websocket;
extern crate serde_json;
extern crate euclid;
extern crate rusttype;

mod http;
mod canvas;
mod json_canvas;
mod touch;

use std::time::Instant;
use std::thread;
use std::sync::mpsc;
use std::collections::HashMap;
use std::net::TcpStream;
use serde_json::Value;
use std::any::Any;
use std::rc::{Rc, Weak};
use std::cell::{RefCell, Ref, RefMut};
use std::ops::Deref;

use canvas::*;
use json_canvas::*;
use euclid::*;

use touch::*;

pub struct WorldSpace; // milimeters @ half meter
pub struct PixelSpace;

pub type WorldPoint = TypedPoint2D<f64, WorldSpace>;
pub type WorldSize = TypedSize2D<f64, WorldSpace>;
pub type PixelPoint = TypedPoint2D<f64, PixelSpace>;

const MM_PER_INCH: f64 = 25.4;
static FONT: &'static [u8] = include_bytes!("html/fonts/iosevka-regular.ttf");

struct Display {
    size: PixelPoint,
    dpi: f64,
    eye_distance_meters: f64,
}

impl Display {
    fn pixel_size(&self) -> ScaleFactor<f64, PixelSpace, WorldSpace> {
        ScaleFactor::new(MM_PER_INCH / self.dpi / self.eye_distance_meters * 0.5)
    }
    fn to_world(&self, point: PixelPoint) -> WorldPoint {
        let s = self.pixel_size();
        (point - self.size * 0.5) * s
    }
    fn to_screen(&self, point: WorldPoint) -> PixelPoint {
        let s = self.pixel_size().inv();
        point * s + self.size * 0.5
    }
    fn setup_canvas(&self, canvas: &mut Canvas) {}
}

pub struct Vm {
    blueprints: Vec<Rc<RefCell<Blueprint>>>,
    active_blueprint: Weak<RefCell<Blueprint>>,

    rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
    websocket_clients: HashMap<i64, websocket::sender::Writer<std::net::TcpStream>>,
    client_counter: i64,
    font: Rc<rusttype::Font<'static>>,

    // Per-client parameters:
    display: Display,
    center: WorldPoint,
    mouse: PixelPoint,
    last_update: Instant,
    mouse_handler: Option<Box<TouchReceiver>>,
}

struct VmCell(Rc<RefCell<Vm>>);

impl Vm {
    fn new() -> VmCell {
        let font_collection = rusttype::FontCollection::from_bytes(FONT);
        let font = Rc::new(font_collection.into_font().unwrap());

        let (tx, rx) = mpsc::channel();

        http::start_thread();

        let websocket_tx = tx.clone();
        thread::spawn(move || {
            let server = websocket::Server::bind("127.0.0.1:8081").unwrap();
            for stream in server
                    .filter_map(Result::ok)
                    .map(|x| x.accept())
                    .filter_map(Result::ok) {
                websocket_tx
                    .send(Event::NewWebsocketClient(stream))
                    .unwrap();
            }
        });

        VmCell(Rc::new(RefCell::new(Vm {
            blueprints: Vec::new(),
            active_blueprint: Weak::new(),
            rx: rx,
            tx: tx,
            center: WorldPoint::new(0., 0.),
            display: Display {
                size: PixelPoint::new(1024., 768.),
                dpi: 96.,
                eye_distance_meters: 0.5,
            },
            websocket_clients: HashMap::new(),
            font: font,
            client_counter: 0,
            mouse: PixelPoint::new(0., 0.),
            last_update: Instant::now(),
            mouse_handler: None,
        })))
    }

    fn update_clients(&mut self) {
        let mut c = JsonCanvas::new(self.font.clone());
        c.save();
        c.font(format!("{}px Iosevka", 6.).as_str());

        c.translate(self.display.size.x / 2., self.display.size.y / 2.);
        c.scale(self.display.pixel_size().inv().get());
        c.translate(self.center.x, self.center.y);

        self.draw(&mut c);
        c.restore();
        let json = c.serialize();
        let message = websocket::Message::text(json);
        for (id, writer) in &mut self.websocket_clients {
            writer.send_message(&message);
        }
        self.last_update = Instant::now();
    }

    fn draw(&self, c: &mut Canvas) {
        let blueprint_rc = self.active_blueprint.upgrade().unwrap();
        let blueprint = blueprint_rc.borrow();
        draw(&blueprint.links, c);
        draw(&blueprint.frames, c);
    }
}

impl VmCell {
    fn borrow(&self) -> Ref<Vm> {
        self.0.borrow()
    }
    fn borrow_mut(&self) -> RefMut<Vm> {
        self.0.borrow_mut()
    }
    fn run(&self) {
        loop {
            let message = self.borrow().rx.recv().unwrap();
            match message {
                Event::NewWebsocketClient(mut client) => {
                    let (mut websocket_reader, websocket_writer) = client.split().unwrap();
                    let client_number = self.borrow().client_counter;
                    println!("Client {} connected (websocket)", client_number);
                    self.borrow_mut().websocket_clients
                        .insert(client_number, websocket_writer);
                    self.borrow_mut().client_counter += 1;
                    let websocket_tx = self.borrow().tx.clone();
                    thread::spawn(move || {
                        for message in websocket_reader.incoming_messages() {
                            let mut message: websocket::Message = match message {
                                Ok(message) => message,
                                Err(_) => break,
                            };

                            use websocket::message::Type;

                            match message.opcode {
                                Type::Close => break,
                                Type::Text => {
                                    let payload = message.payload.to_mut();
                                    let json: Value = serde_json::from_slice(payload).unwrap();
                                    Event::from(json)
                                        .map(|event| { websocket_tx.send(event).unwrap(); });
                                }
                                _ => {}
                            };
                        }
                        websocket_tx
                            .send(Event::WebsocketDisconnected(client_number))
                            .unwrap();
                    });
                }
                Event::WebsocketDisconnected(i) => {
                    println!("Client {} disconnected", i);
                    self.borrow_mut().websocket_clients.remove(&i);
                }
                Event::MouseDown {
                    x: x,
                    y: y,
                    button: button,
                } => {
                    /* # Interaction modes
                     *
                     * Interaction modes describe how touch points (fingers on the screen / mouse pointer)
                     * affect the interface.
                     *
                     * ## Navigation Mode
                     *
                     * On desktop enabled by holding the middle mouse button or Left Shift.
                     * On mobile enabled by holding the navigation button.
                     *
                     * While in navigation mode, touch points are locked to their initial positions in
                     * the world space. Window viewport is adjusted to maintain this constraint.
                     *
                     * Moving cursor / finger moves the window in the opposing direction.
                     *
                     * Scrolling / pinching scales the window and keeps the effect.
                     * 
                     * Dragging the navigation button scales the window temporarily.
                     *
                     * ## Immediate Mode
                     *
                     * On desktop this mode is controlled by the left mouse button.
                     * On mobile enabled by holding the immediate button.
                     *
                     * Touching an element of the interface invokes default action. Usually movement.
                     *
                     * The action happens in the world space.
                     *
                     * ## Menu Mode
                     *
                     * On desktop this mode is controlled by the right mouse button.
                     * On mobile this is the default mode.
                     *
                     * Holding the touch point starts the menu in drag mode. Releasing the button quickly
                     * opens the menu in persistent mode.
                     *
                     * Opens a screen space menu with actions.
                     *
                     * Activating an action moves the interaction to the world space.
                     */
                    let mut vm = self.borrow_mut();
                    if vm.mouse_handler.is_some() {
                        continue;
                    }
                    let mut frames;
                    {
                        let blueprint = vm.active_blueprint.upgrade().unwrap();
                        let blueprint = blueprint.borrow();
                        frames = blueprint.frames.clone();
                    }
                    let pixel_point = PixelPoint::new(x, y);
                    let world_point = vm.display.to_world(pixel_point) - vm.center;
                    vm.mouse_handler = match button {
                        0 => start_touch(&frames, &world_point),
                        1 => Some(Box::new(NavTouchReceiver{ vm: Rc::downgrade(&self.0), pos: world_point })),
                        2 => {
                            //blueprint.start_touch_menu(x, y)
                            None
                        }
                        _ => None
                    }
                }
                Event::MouseUp {
                    x: x,
                    y: y,
                    button: button,
                } => {
                    self.borrow_mut().mouse_handler = None;
                }
                Event::MouseMove { x: x, y: y } => {
                    self.borrow_mut().mouse = PixelPoint::new(x, y);

                    let opt = self.borrow_mut().mouse_handler.take();
                    let opt = opt.and_then(|b| {
                        let world_point = self.borrow().display.to_world(self.borrow().mouse) - self.borrow().center;
                        b.continue_touch(world_point)
                     });
                    self.borrow_mut().mouse_handler = opt;
                    
                    if self.borrow().mouse_handler.is_some() {
                        self.borrow_mut().update_clients();
                    }
                }
                Event::DisplaySize {
                    width: w,
                    height: h,
                } => {
                    let mut vm = self.borrow_mut();
                    vm.display.size = PixelPoint::new(w, h);
                    println!("Display size is {} x {} px", w, h);
                    vm.update_clients();
                }
                Event::RenderingDone => {
                    /*
                let dur = last_update.elapsed();
                println!("Rendering done ({} ms)",
                         (dur.as_secs() as f64) * 1000. + (dur.subsec_nanos() as f64) / 1000000.);
*/
                }
                Event::RenderingReady => {
                    /*
                let dur = last_update.elapsed();
                println!("Rendering ready ({} ms)",
                         (dur.as_secs() as f64) * 1000. + (dur.subsec_nanos() as f64) / 1000000.);
                 */
                }
                Event::MouseWheel { x: x, y: y } => {
                    let mut vm = self.borrow_mut();
                    let start = vm.display.to_world(vm.mouse) - vm.center;
                    vm.display.eye_distance_meters *= (y/-200.).exp();
                    let end = vm.display.to_world(vm.mouse) - vm.center;
                    vm.center = vm.center - start + end;
                    vm.update_clients();
                }
                _ => {}
            }
        }
    }
}

struct Blueprint {
    vm: Weak<RefCell<Vm>>,
    name: String,
    frames: Vec<Rc<RefCell<Frame>>>,
    links: Vec<Rc<RefCell<Link>>>,
    machines: Vec<Rc<RefCell<Machine>>>,
    active_machine: Weak<RefCell<Machine>>,
}

trait TouchReceiver {
    fn continue_touch(&self, p: WorldPoint) -> Option<Box<TouchReceiver>>;
    fn end_touch(&self);
}

trait Visible {
    fn draw(&self, c: &mut Canvas);
    fn start_touch(&self, p: &WorldPoint) -> Option<Box<TouchReceiver>>;
}

impl Blueprint {
    fn new(vm_cell: &Rc<RefCell<Vm>>, name: String, activate: bool) -> Rc<RefCell<Blueprint>> {
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

    fn with_object<F: FnMut(&mut Object)>(&self, frame_rc: &Rc<RefCell<Frame>>, mut f: F) {
        let frame = frame_rc.borrow();
        let machine_rc = if frame.global {
            self.machines[0].clone()
        } else {
            self.active_machine.upgrade().unwrap()
        };
        let mut machine = machine_rc.borrow_mut();
        let object = machine
            .objects
            .iter_mut()
            .find(|o| Rc::ptr_eq(&o.frame, frame_rc))
            .unwrap();
        f(object);
    }
}

fn start_touch<V: Visible>(v: &Vec<V>, p: &WorldPoint) -> Option<Box<TouchReceiver>> {
    walk_visible(v, |elem| { elem.start_touch(p) })
}

fn draw<V: Visible>(v: &Vec<V>, c: &mut Canvas) {
    walk_visible(v, |elem| -> Option<()> {
        c.save();
        elem.draw(c);
        c.restore();
        None
    });
}

fn walk_visible<V: Visible, T, F: FnMut(&Visible)->Option<T>>(v: &Vec<V>, mut f: F) -> Option<T> {
    for visible in v.iter() {
        let result = f(visible as &Visible);
        if result.is_some() {
            return result;
        }
    }
    return None
}

struct FrameParam{
    frame: Rc<RefCell<Frame>>,
    param_index: usize,
}

impl FrameParam {
    fn center(&self) -> WorldPoint {
        let frame = self.frame.borrow();
        frame.pos + WorldPoint::new(
            0.0,
            frame.size.height * 0.5 + -PARAM_RADIUS + (PARAM_RADIUS * 2. + PARAM_SPACING) * (self.param_index as f64 + 1.),
        )
    }
}

impl Visible for FrameParam {
    fn draw(&self, c: &mut Canvas) {
        let center = self.center();
        let param = &self.frame.borrow().typ.parameters[self.param_index];
        c.beginPath();
        c.arc(center.x, center.y, PARAM_RADIUS, 0.0, std::f64::consts::PI * 2.);
        c.fillStyle("white");
        c.fill();
        c.fillStyle("black");
        c.fillText(param.name, center.x + PARAM_RADIUS + PARAM_SPACING, center.y);
    }
    fn start_touch(&self, p: &WorldPoint) -> Option<Box<TouchReceiver>> {
        let center = self.center();
        let q = *p - center;
        let dist = q.dot(q).sqrt();
        if dist < PARAM_RADIUS {
            let frame = self.frame.borrow();
            let blueprint_rc = frame.blueprint.upgrade().unwrap();
            let mut blueprint = blueprint_rc.borrow_mut();
            let link_rc = Rc::new(RefCell::new(Link {
                blueprint: frame.blueprint.clone(),
                a: LinkTerminator::Frame(self.frame.clone()),
                b: LinkTerminator::Point(*p),
                param_index: self.param_index,
                order: 0,
            }));
            blueprint.links.push(link_rc.clone());
            Some(Box::new(DragLink{
                side: LinkSide::B,
                link: link_rc,
                pos: *p,
            }))
        } else {
            None
        }
    }
}

impl Visible for Rc<RefCell<Frame>> {
    fn draw(&self, c: &mut Canvas) {
        let frame = self.borrow();
        if frame.typ.parameters.len() > 0 {
            c.strokeStyle("#888");
            c.beginPath();
            let last_frame_param = FrameParam{frame: self.clone(), param_index: frame.typ.parameters.len() - 1};
            let last_param_center = last_frame_param.center();
            c.moveTo(frame.pos.x, frame.pos.y);
            c.lineTo(last_param_center.x, last_param_center.y);
            c.stroke();
        }
        for param_index in 0..frame.typ.parameters.len() {
            let frame_param = FrameParam{frame: self.clone(), param_index: param_index,};
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
        blueprint.with_object(self, |o| {
            (frame.typ.draw)(o, c);
        });
    }
    fn start_touch(&self, p: &WorldPoint) -> Option<Box<TouchReceiver>> {
        // TODO: move this to touch::drag
        let mut q;
        let mut s;
        let mut param_count;
        {
            let frame = self.borrow();
            q = *p - frame.pos;
            s = frame.size * 0.5;
            param_count = frame.typ.parameters.len();
        }
        fn range_check(x: f64, range: f64) -> bool {
            x < range && x > -range
        }
        let hit = range_check(q.x, s.width) && range_check(q.y, s.height);
        
        if hit {
            // TODO: query object type
            //let args = Args(vec![]);
            //(frame.typ.run)(&args);
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
            Some(Box::new(DragFrame{
                horizontal: choose_drag_mode(q.x, s2.width),
                vertical: choose_drag_mode(q.y, s2.height),
                frame: self.clone(),
                pos: p.clone(),
            }))
        } else {
            for param_index in 0..param_count {
                let frame_param = FrameParam{ frame: self.clone(), param_index: param_index, };
                let touch_receiver = frame_param.start_touch(p);
                if touch_receiver.is_some() {
                    return touch_receiver;
                }
            }
            None
        }
    }
}

pub struct Frame {
    blueprint: Weak<RefCell<Blueprint>>,
    typ: &'static Type,
    pos: WorldPoint,
    size: WorldSize,
    global: bool,
}
    
const PARAM_RADIUS: f64 = 5.;
const PARAM_SPACING: f64 = 2.;

impl Frame {
    fn new(typ: &'static Type,
           blueprint: &Rc<RefCell<Blueprint>>,
           global: bool)
           -> Rc<RefCell<Frame>> {
        let f = Rc::new(RefCell::new(Frame {
            blueprint: Rc::downgrade(blueprint),
            typ: typ,
            pos: WorldPoint::zero(),
            size: WorldSize::new(10., 10.),
            global: global,
        }));
        blueprint.borrow_mut().frames.push(f.clone());
        for machine_cell in blueprint.borrow().machines.iter() {
            let mut machine = machine_cell.borrow_mut();
            let mut object = Object {
                machine: Rc::downgrade(machine_cell),
                frame: f.clone(),
                execute: false,
                running: false,
                data: Box::new(()),
            };
            (typ.init)(&mut object);
            machine.objects.push(object);
            if global {
                break;
            }
        }
        return f;
    }
}

enum LinkTerminator {
    Frame(Rc<RefCell<Frame>>),
    Point(WorldPoint),
}

impl LinkTerminator {
    fn get_pos(&self)->WorldPoint {
        match self {
            &LinkTerminator::Frame(ref frame) => {
                let frame = frame.borrow();
                frame.pos
            }
            &LinkTerminator::Point(point) => point,
        }
    }
}

pub struct Link {
    blueprint: Weak<RefCell<Blueprint>>,
    a: LinkTerminator,
    b: LinkTerminator,
    param_index: usize,
    order: i32,
}

impl Visible for Rc<RefCell<Link>> {
    fn draw(&self, c: &mut Canvas) {
        let link = self.borrow();
        let start = link.a.get_pos();
        let end = link.b.get_pos();
        
        //if link.a.is_some() {
        //    let a_rc = link.a.clone().unwrap();
        //    let frame_param = FrameParam{ frame: a_rc.clone(), param_index: link.param_index };
        //    let start = frame_param.center();
        c.strokeStyle("1px solid black");
        c.beginPath();
        c.moveTo(start.x, start.y);
        c.lineTo(end.x, end.y);
        c.stroke();
        
    }
    fn start_touch(&self, p: &WorldPoint) -> Option<Box<TouchReceiver>> {
        None
    }
}

struct Machine {
    blueprint: Weak<RefCell<Blueprint>>,
    objects: Vec<Object>,
}

impl Machine {
    fn new(blueprint: &Rc<RefCell<Blueprint>>, activate: bool) -> Rc<RefCell<Machine>> {
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
}

struct Object {
    machine: Weak<RefCell<Machine>>,
    frame: Rc<RefCell<Frame>>,
    execute: bool,
    running: bool,
    data: Box<Any>,
}


struct Arg(Weak<RefCell<Frame>>, Weak<RefCell<Machine>>);
struct ArgPack(Vec<Arg>);
struct Args(Vec<ArgPack>);

struct Parameter {
    name: &'static str,
    runnable: bool,
    output: bool,
}

struct Type {
    name: &'static str,
    parameters: &'static [Parameter],
    init: &'static (Fn(&mut Object) + Sync),
    run: &'static (Fn(&Args) + Sync),
    draw: &'static (Fn(&Object, &mut Canvas) + Sync),
}

static text_type: Type = Type {
    name: "Text",
    parameters: &[],
    init: &|o: &mut Object| { o.data = Box::new("Some string".to_string()); },
    run: &|args: &Args| {},
    draw: &|o: &Object, canvas: &mut Canvas| {
        let font_metrics = canvas.get_font_metrics(6.);

        canvas.fillStyle("black");
        canvas.fillText(o.data.downcast_ref::<String>().unwrap(),
                        2.,
                        2. + font_metrics.ascent as f64);
    },
};

static process_type: Type = Type {
    name: "Process",
    parameters: &[
        Parameter {
            name: "Command",
            runnable: false,
            output: false,
        },
    ],
    init: &|o: &mut Object| {},
    run: &|args: &Args| {
        println!("Executing Process");
        let mut child = std::process::Command::new("/bin/ls")
            .arg("/home/mrogalski")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("failed to execute ls");
        let output = child.wait_with_output().expect("failed to wait on ls");
        println!("Result: {}", String::from_utf8(output.stdout).unwrap());
    },
    draw: &|o: &Object, canvas: &mut Canvas| {},
};

fn main() {
    let vm = Vm::new();
    let blueprint = Blueprint::new(&vm.0, "Default".to_string(), true);
    Machine::new(&blueprint, true);
    let text_frame = Frame::new(&text_type, &blueprint, true);
    {
        let mut frame = text_frame.borrow_mut();
        frame.size.width = 50.;
        frame.pos.y = -20.;
    }
    let process_frame = Frame::new(&process_type, &blueprint, true);

    vm.run();
}

enum Event {
    NewWebsocketClient(websocket::Client<TcpStream>),
    WebsocketDisconnected(i64),
    RenderingReady, // sent when next frame is ready for commands
    RenderingDone, // sent after all rendering commands are flushed
    DisplaySize { width: f64, height: f64 },
    MouseMove { x: f64, y: f64 },
    MouseWheel { x: f64, y: f64 },
    MouseDown { x: f64, y: f64, button: i64 },
    MouseUp { x: f64, y: f64, button: i64 },
    KeyDown { code: String, key: String },
    KeyUp { code: String, key: String },
}

impl Event {
    fn from(json: Value) -> Option<Event> {
        let obj = json.as_object().unwrap();
        let typ = obj.get("type").unwrap().as_str().unwrap();


        match typ.as_ref() {
            "size" => {
                Some(Event::DisplaySize {
                         width: obj.get("width").unwrap().as_f64().unwrap(),
                         height: obj.get("height").unwrap().as_f64().unwrap(),
                     })
            }
            "mouse_move" => {
                Some(Event::MouseMove {
                         x: obj.get("x").unwrap().as_f64().unwrap(),
                         y: obj.get("y").unwrap().as_f64().unwrap(),
                     })
            }
            "mouse_down" => {
                Some(Event::MouseDown {
                         x: obj.get("x").unwrap().as_f64().unwrap(),
                         y: obj.get("y").unwrap().as_f64().unwrap(),
                         button: obj.get("button").unwrap().as_i64().unwrap(),
                     })
            }
            "mouse_up" => {
                Some(Event::MouseUp {
                         x: obj.get("x").unwrap().as_f64().unwrap(),
                         y: obj.get("y").unwrap().as_f64().unwrap(),
                         button: obj.get("button").unwrap().as_i64().unwrap(),
                     })
            }
            "render_done" => Some(Event::RenderingDone),
            "render_ready" => Some(Event::RenderingReady),
            "key_up" => {
                Some(Event::KeyUp {
                         key: String::from(obj.get("key").unwrap().as_str().unwrap()),
                         code: String::from(obj.get("code").unwrap().as_str().unwrap()),
                     })
            }
            "key_down" => {
                Some(Event::KeyDown {
                         key: String::from(obj.get("key").unwrap().as_str().unwrap()),
                         code: String::from(obj.get("code").unwrap().as_str().unwrap()),
                     })
            }
            "wheel" => {
                Some(Event::MouseWheel {
                         x: obj.get("x").unwrap().as_f64().unwrap(),
                         y: obj.get("y").unwrap().as_f64().unwrap(),
                     })
            }
            _ => {
                println!("Unknown Event: {:?}", json);
                None
            }
        }
    }
}
