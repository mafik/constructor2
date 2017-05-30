/*
TODOs:
-

Milestones:
1. Object that runs a command on click
2. Command is fetched from another (text) object
3. Command is fetched from several other (text) objects
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

use std::time::{Duration, Instant};
use std::thread;
use std::sync::mpsc;
use std::collections::HashMap;
use std::net::TcpStream;
use serde_json::Value;
use std::any::Any;
use std::rc::{Rc, Weak};
use std::cell::{Cell, RefCell, Ref};
use std::ops::Deref;

use canvas::*;
use json_canvas::*;
use euclid::*;

struct WorldSpace; // milimeters @ half meter
struct PixelSpace;

type WorldPoint = TypedPoint2D<f64, WorldSpace>;
type WorldSize = TypedSize2D<f64, WorldSpace>;
type PixelPoint = TypedPoint2D<f64, PixelSpace>;


const MM_PER_INCH: f64 = 25.4;
static FONT: &'static [u8] = include_bytes!("html/fonts/iosevka-regular.ttf");

struct Window {
    size: PixelPoint,
    dpi: f64,
    eye_distance_meters: f64,
}

impl Window {
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

struct Vm<'a> {
    blueprints: Vec<Rc<RefCell<Blueprint>>>,
    active_blueprint: Weak<RefCell<Blueprint>>,

    rx: mpsc::Receiver,
    tx: mpsc::Sender,
    center: WorldPoint,
    window: Window,
    websocket_clients: HashMap<i64, websocket::sender::Writer<_>>,
    font: Rc<rusttype::Font<'a>>,
    client_counter: i64,
    mouse: PixelPoint,
    nav_mode: bool,
    last_update: Instant,
}

impl<'a> Vm<'a> {
    fn new() -> Rc<RefCell<Vm<'a>>> {
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

        Rc::new(RefCell::new(Vm {
                                 blueprints: Vec::new(),
                                 active_blueprint: Weak::new(),
                                 rx: rx,
                                 tx: tx,
                                 center: WorldPoint::new(0., 0.),
                                 window: Window {
                                     size: PixelPoint::new(1024., 768.),
                                     dpi: 96.,
                                     eye_distance_meters: 0.5,
                                 },
                                 websocket_clients: HashMap::new(),
                                 font: font,
                                 client_counter: 0,
                                 mouse: PixelPoint::new(0., 0.),
                                 nav_mode: false,
                                 last_update: Instant::now(),
                             }))
    }

    fn update_clients(&mut self) {
        let mut c = JsonCanvas::new(self.font.clone());
        c.save();
        c.font(format!("{}px Iosevka", 6.).as_str());

        c.translate(self.window.size.x / 2., self.window.size.y / 2.);
        c.scale(self.window.pixel_size().inv().get());
        c.translate(self.center.x, self.center.y);

        self.draw(&mut c);
        c.restore();
        let json = c.serialize();
        let message = websocket::Message::text(json);
        for (id, writer) in self.websocket_clients {
            writer.send_message(&message);
        }
    }

    fn draw(&self, c: &mut Canvas) {
        let rc = self.active_blueprint.upgrade().unwrap();
        rc.borrow().draw(c);
    }

    fn run(&mut self) {

        for message in self.rx.iter() {
            match message {
                Event::NewWebsocketClient(mut client) => {
                    let (mut websocket_reader, websocket_writer) = client.split().unwrap();
                    let client_number = client_counter;
                    println!("Client {} connected (websocket)", client_number);
                    websocket_clients.insert(client_number, websocket_writer);
                    client_counter += 1;
                    let websocket_tx = tx.clone();
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
                    websocket_clients.remove(&i);
                }
                Event::MouseDown {
                    x: x,
                    y: y,
                    button: button,
                } => {
                    if button == 1 {
                        nav_mode = true;
                    }
                    println!("Mouse button {} pressed at {} x {}", button, x, y);
                }
                Event::MouseUp {
                    x: x,
                    y: y,
                    button: button,
                } => {
                    if button == 1 {
                        nav_mode = false;
                    }
                }
                Event::MouseMove { x: x, y: y } => {
                    let old_mouse = mouse.clone();
                    mouse = PixelPoint::new(x, y);
                    let delta = mouse - old_mouse;
                    let world_delta = delta * window.pixel_size();
                    if nav_mode {
                        center = center + world_delta;
                        println!("Sent update");
                        update_clients(&window, &mut websocket_clients, &center);
                        last_update = Instant::now();
                    }
                }
                Event::WindowSize {
                    width: w,
                    height: h,
                } => {
                    window.size = PixelPoint::new(w, h);
                    println!("Window size is {} x {} px", w, h);
                    update_clients(&window, &mut websocket_clients, &center);
                    last_update = Instant::now();
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

    fn draw(&self, c: &mut Canvas) {
        let global_machine_borrow = self.machines[0].borrow();
        let global_machine: &Machine = global_machine_borrow.deref();
        let local_machine_rc = self.active_machine.upgrade().unwrap();
        let local_machine_borrow = local_machine_rc.borrow();
        let local_machine: &Machine = local_machine_borrow.deref();

        for frame_cell in self.frames.iter() {
            let frame = frame_cell.borrow();
            c.save();
            c.translate(frame.pos.x, frame.pos.y);
            let machine = if frame.global {
                global_machine
            } else {
                local_machine
            };
            let o = machine
                .objects
                .iter()
                .find(|o| Rc::ptr_eq(&o.frame, frame_cell))
                .unwrap();
            frame.draw(c, o);
            c.restore();
        }
    }
}

struct Frame {
    blueprint: Weak<RefCell<Blueprint>>,
    typ: &'static Type,
    pos: WorldPoint,
    size: WorldSize,
    global: bool,
}

impl Frame {
    fn new(typ: &'static Type,
           blueprint_cell: &Rc<RefCell<Blueprint>>,
           global: bool)
           -> Rc<RefCell<Frame>> {
        let f = Rc::new(RefCell::new(Frame {
                                         blueprint: Rc::downgrade(blueprint_cell),
                                         typ: typ,
                                         pos: WorldPoint::zero(),
                                         size: WorldSize::new(10., 10.),
                                         global: global,
                                     }));
        let mut blueprint = blueprint_cell.borrow_mut();
        blueprint.frames.push(f.clone());
        for machine_cell in blueprint.machines.iter() {
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

    fn draw(&self, c: &mut Canvas, o: &Object) {
        c.translate(-self.size.width / 2., -self.size.height / 2.);
        c.fillStyle("white");
        c.fillRect(0., 0., self.size.width, self.size.height);
        c.fillStyle("black");
        c.fillText(self.typ.name, 0., 0.);
        (self.typ.draw)(o, c);
    }
}


struct Link {
    blueprint: Weak<RefCell<Blueprint>>,
    a: Rc<Frame>,
    b: Rc<Frame>,
    param_i: i32,
    order: i32,
}

struct Machine {
    blueprint: Weak<RefCell<Blueprint>>,
    objects: Vec<Object>,
}

impl Machine {
    fn new(blueprint_cell: &Rc<RefCell<Blueprint>>, activate: bool) -> Rc<RefCell<Machine>> {
        let mut blueprint = blueprint_cell.borrow_mut();
        let m = Rc::new(RefCell::new(Machine {
                                         blueprint: Rc::downgrade(blueprint_cell),
                                         objects: Vec::new(),
                                     }));
        // TODO: init all objects (from blueprint frames)
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
    parameters: &[],
    init: &|o: &mut Object| {},
    run: &|args: &Args| {},
    draw: &|o: &Object, canvas: &mut Canvas| {},
};

fn main() {
    let vm = Vm::new();
    let blueprint = Blueprint::new(&vm, "Default".to_string(), true);
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
    WindowSize { width: f64, height: f64 },
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
                Some(Event::WindowSize {
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
