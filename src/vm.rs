extern crate websocket;
extern crate rusttype;
extern crate serde_json;
extern crate ref_eq;
extern crate serde;

use std::rc::{Rc, Weak};
use std::cell::RefCell;
use std::collections::{HashSet, HashMap, VecDeque};
use std::sync::mpsc;
use std::net::TcpStream;
use std::time;
use std::thread;
use std::process;

use blueprint::*;
use json_canvas::*;
use canvas::*;
use process_type;
use empty_type;
use text_type;
use menu::*;
use RunArgs;
use event::*;
use Display;
use WorldPoint;
use DisplayPoint;
use PixelPoint;
use Object;
use TouchReceiver;
use LinkTerminator;
use FrameParam;
use Visible;
use ObjectCell;
use euclid::ScaleFactor;
use DisplayMillimetreSpace;
use WorldMillimetreSpace;
use Type;
use AddFrameAction;
use http;
use touch::*;

static FONT: &'static [u8] = include_bytes!("html/fonts/iosevka-regular.ttf");

fn walk_visible<V: Visible, T, F: FnMut(&Visible)->Option<T>>(v: &Vec<V>, mut f: F) -> Option<T> {
    for visible in v.iter() {
        let result = f(visible as &Visible);
        if result.is_some() {
            return result;
        }
    }
    return None
}

pub struct Vm {
    pub blueprints: Vec<Rc<RefCell<Blueprint>>>,
    pub active_blueprint: Weak<RefCell<Blueprint>>,

    pub tasks: VecDeque<Weak<RefCell<Object>>>,
    pub types: Vec<&'static Type>,

    is_running: bool,

    rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
    websocket_clients: HashMap<i64, websocket::sender::Writer<TcpStream>>,
    client_counter: i64,
    font: Rc<rusttype::Font<'static>>,

    // Per-client parameters:
    display: Display,
    center: Rc<RefCell<WorldPoint>>,
    mouse: PixelPoint,
    last_update: time::Instant,
    mouse_handler: Option<Box<TouchReceiver>>,
    menus: Vec<Weak<VisibleMenu>>,
    zoom: ScaleFactor<f64, DisplayMillimetreSpace, WorldMillimetreSpace>,
}

use self::serde::ser::{Serialize, Serializer, SerializeSeq, SerializeStruct};

use SerializableVec;

struct Tasks<'a> (&'a Vm);

impl <'a> Serialize for Tasks<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
        use std::ops::Deref;
        let mut task_seq = serializer.serialize_seq(None)?;
        for task in self.0.tasks.iter() {
            if let Some(task) = task.upgrade() {
                let task = task.borrow();
                let frame = task.frame.borrow();
                let blueprint = frame.blueprint.upgrade().unwrap();
                let blueprint_index = self.0.blueprint_index(&blueprint);
                let blueprint = blueprint.borrow();
                let frame_index = blueprint.frame_index(&task.frame);
                let machine = task.machine.upgrade().unwrap();
                let machine_index = blueprint.machine_index(&machine);
                let tuple = (blueprint_index, frame_index, machine_index);
                task_seq.serialize_element(&tuple)?;
            }
        }
        task_seq.end()
    }
}

impl Serialize for Vm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
        let mut serializer = serializer.serialize_struct("Vm", 3)?;
        serializer.serialize_field("blueprints", &SerializableVec(&self.blueprints))?;
        let active_blueprint = self.active_blueprint.upgrade().unwrap();
        let active_blueprint = self.blueprint_index(&active_blueprint);
        serializer.serialize_field("active_blueprint", &active_blueprint);
        serializer.serialize_field("tasks", &Tasks(self));
        serializer.end()
    }
}

use std::fmt;
use std::error::Error;

struct VmVisitor;

impl Vm {
    fn blueprint_index(&self, blueprint: &Rc<RefCell<Blueprint>>) -> u32 {
        for (i, other) in self.blueprints.iter().enumerate() {
            if Rc::ptr_eq(blueprint, other) { return i as u32; }
        }
        panic!("Bad blueprint reference");
    }
    fn mouse_object(&self) -> Option<Weak<RefCell<Object>>> {
        let world_point = self.mouse_world();
        let blueprint_rc = self.active_blueprint.upgrade().unwrap();
        let blueprint = blueprint_rc.borrow();
        let frame_rc = blueprint.query_frame(world_point);
        if frame_rc.is_none() { return None; }
        let frame_rc = frame_rc.unwrap();
        return Some(Rc::downgrade(&blueprint.get_object(&frame_rc)));
    }
    fn mouse_display(&self) -> DisplayPoint {
        self.display.to_millimetre(self.mouse)
    }
    fn mouse_world(&self) -> WorldPoint {
        self.display.to_millimetre(self.mouse) * self.zoom - *self.center.borrow()
    }
    pub fn new() -> Rc<RefCell<Vm>> {
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
                    types: vec![&process_type, &text_type, &empty_type],
                    tasks: VecDeque::new(),
                    is_running: true,
                    rx: rx,
                    tx: tx,
                    center: Rc::new(RefCell::new(WorldPoint::new(0., 0.))),
                    display: Display {
                        size: PixelPoint::new(1024., 768.),
                        dpi: 96.,
                        eye_distance_meters: 0.5,
                    },
                    websocket_clients: HashMap::new(),
                    font: font,
                    client_counter: 0,
                    mouse: PixelPoint::new(0., 0.),
                    last_update: time::Instant::now(),
                    mouse_handler: None,
                    menus: Vec::new(),
                    zoom: ScaleFactor::new(1.0),
        }))
    }

    pub fn activate(&mut self, blueprint: &Rc<RefCell<Blueprint>>) {
        self.active_blueprint = Rc::downgrade(blueprint);
    }

    pub fn load_json(this: &Rc<RefCell<Vm>>) -> Result<(), Box<Error>> {
        use std::fs::File;
        use std::io::Read;
        let file = File::open("vm.json")?;
        let value: serde_json::Value = serde_json::from_reader(file)?;
        let blueprints = value.get("blueprints").ok_or("No blueprints")?;
        let blueprints = blueprints.as_array().ok_or("Blueprints is not an array")?;
        for blueprint in blueprints.iter() {
            let mybp = Blueprint::new(this);
            Blueprint::load_json(&mybp, blueprint);
        }
        let active_blueprint = value.get("active_blueprint").unwrap().as_i64().unwrap();
        let weak_bp = Rc::downgrade(&this.borrow().blueprints[active_blueprint as usize]);
        this.borrow_mut().active_blueprint = weak_bp;
        let tasks = value.get("tasks").ok_or("No tasks")?;
        let tasks = tasks.as_array().ok_or("Tasks is not an array")?;
        for task in tasks.iter() {
            // TODO
        }
        let mut contents = String::new();
        let mut file = File::open("vm.json")?;
        file.read_to_string(&mut contents)?;
        println!("File contents:");
        println!("{}", contents);
        use std::ops::Deref;
        let buffer = serde_json::to_string(this.borrow().deref()).ok().unwrap();
        println!("Loaded contents:");
        println!("{}", buffer);

        Ok(())
    }

    fn update_clients(&mut self) {
        let mut c = JsonCanvas::new(self.font.clone());
        self.draw(&mut c);
        let json = c.serialize();
        let message = websocket::Message::text(json);
        for (id, writer) in &mut self.websocket_clients {
            writer.send_message(&message);
        }
        self.last_update = time::Instant::now();
    }

    fn draw(&mut self, c: &mut Canvas) {
        let blueprint_rc = self.active_blueprint.upgrade().unwrap();
        let blueprint = blueprint_rc.borrow();

        fn draw<V: Visible>(v: &Vec<V>, c: &mut Canvas) {
            walk_visible(v, |elem| -> Option<()> {
                c.save();
                elem.draw(c);
                c.restore();
                None
            });
        }
        
        c.save();
        c.font(format!("{}px Iosevka", 6.).as_str());
        c.translate(self.display.size.x / 2., self.display.size.y / 2.);
        c.scale(self.display.pixel_size().inv().get());

        c.save();
        c.scale(self.zoom.inv().get());
        {
            let center = self.center.borrow();
            c.translate(center.x, center.y);
        }
        draw(&blueprint.frames, c);
        draw(&blueprint.links, c);
        c.restore();

        use PARAM_RADIUS;
        let pixel_scale = self.display.pixel_size().get();
        let half_width = self.display.size.x * 0.5 * pixel_scale;
        let half_height = self.display.size.y * 0.5 * pixel_scale;
        let mut top = -half_height + PARAM_RADIUS * 2.;
        let left = -half_width + PARAM_RADIUS * 2.;
        let active_machine = blueprint.active_machine.upgrade().unwrap();
        for machine in blueprint.machines.iter() {
            let fill_style = if Rc::ptr_eq(machine, &active_machine) {
                "#3e64a3"
            } else {
                "#959ba5"
            };
            c.fillStyle(fill_style);
            c.fillCircle(left, top, PARAM_RADIUS);
            top += PARAM_RADIUS * 3.;
        }

        let menus_rc = self.menus.iter().filter_map(|x| x.upgrade()).collect();
        draw(&menus_rc, c);
        self.menus = menus_rc.iter().map(Rc::downgrade).collect();
        c.restore();
    }

    fn make_menu(&mut self) -> Menu {
        let d = self.mouse_display();
        let w = self.mouse_world();
        let move_view = Entry {
                    name: "Move view".to_string(),
                    color: None,
                    shortcuts: vec!["MMB".to_string()],
                    action: Box::new(MovePointAction::new(Rc::downgrade(&self.center), true)),
        };
        let type_entries = self.types.iter().map(|typ| {
            Entry {
                name: format!("New {}", typ.name),
                color: None,
                shortcuts: Vec::new(),
                action: Box::new(AddFrameAction::new(typ)),
            }
        });
        
        {
            let blueprint = self.active_blueprint.upgrade().unwrap();
            let blueprint = blueprint.borrow();
            let frames = blueprint.frames.clone();
            
            if let Some(mut frame_menu) = walk_visible(&frames, |frame| { frame.make_menu(d, w) }) {
                frame_menu.entries.push(move_view);
                frame_menu.entries.extend(type_entries);
                return frame_menu;
            }
        }

        let mut menu_entries = vec![move_view];
        menu_entries.extend(type_entries);
        
        Menu {
            entries: menu_entries,
            color: "#f49e42".to_string(),
        }
        /*
        let mut frames;
        {
            let blueprint = self.active_blueprint.upgrade().unwrap();
            let blueprint = blueprint.borrow();
            frames = blueprint.frames.clone();
        }
        self.mouse_handler = match button {
            0 => start_touch(&frames, &world_point),
            1 => Some(Box::new(NavTouchReceiver{ nav: Rc::downgrade(&self.center), last_pos: world_point })),
            2 => open_menu(world_point),
            _ => None
        }
         */
    }

    fn open_menu(&mut self, menu: Menu, point: DisplayPoint) -> Option<Box<TouchReceiver>> {
        let visible_menu_rc = VisibleMenu::new(menu, point);
        self.menus.push(Rc::downgrade(&visible_menu_rc));
        Some(Box::new(visible_menu_rc))
    }

    fn activate_shortcut(&mut self, menu: Menu, code: String) -> Option<Box<TouchReceiver>> {
        let d = self.mouse_display();
        let w = self.mouse_world();
        menu.activate_shortcut(self, code, d, w)
    }

    fn process_event(&mut self, event: Event) {
        match event {
            Event::Quit(mut over) => {
                println!("VM: received Quit");
                over.send(0).unwrap();
                println!("VM: sent response");
            },
                Event::NewWebsocketClient(mut client) => {
                    let (mut websocket_reader, websocket_writer) = client.split().unwrap();
                    let client_number = self.client_counter;
                    self.client_counter += 1;
                    println!("Client {} connected (websocket)", client_number);
                    self.websocket_clients.insert(client_number, websocket_writer);
                    let websocket_tx = self.tx.clone();
                    thread::spawn(move || {
                        for message in websocket_reader.incoming_messages() {
                            let mut message: websocket::Message = match message {
                                Ok(message) => message,
                                Err(_) => break,
                            };

                            use self::websocket::message::Type;

                            match message.opcode {
                                Type::Close => break,
                                Type::Text => {
                                    let payload = message.payload.to_mut();
                                    let json: serde_json::Value = serde_json::from_slice(payload).unwrap();
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
                    self.websocket_clients.remove(&i);
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
                    
                    if self.mouse_handler.is_some() {
                        return;
                    }
                    let display_point = self.mouse_display();
                    let world_point = self.mouse_world();
                    let menu = self.make_menu();
                    self.mouse_handler = match button {
                        0 => menu.activate_shortcut(self, "LMB".to_string(), display_point, world_point),
                        1 => menu.activate_shortcut(self, "MMB".to_string(), display_point, world_point),
                        2 => self.open_menu(menu, display_point),
                        _ => None,
                    };
                    self.update_clients();
                }
                Event::MouseUp {
                    x: x,
                    y: y,
                    button: button,
                } => {
                    match self.mouse_handler.take() {
                        Some(touch_receiver) => touch_receiver.end_touch(self),
                        None => (),
                    }
                    self.update_clients();
                }
                Event::MouseMove { x: x, y: y } => {
                    self.mouse = PixelPoint::new(x, y);
                    let display = self.mouse_display();
                    let world = self.mouse_world();

                    let taken = self.mouse_handler.take();
                    let update = taken.is_some();
                    let taken = taken.and_then(|b| b.continue_touch(self, display, world));
                    self.mouse_handler = taken;
                    
                    if update {
                        self.update_clients();
                    }
                }
                Event::KeyDown { code: code, key: key } => {
                    println!("Pressed key {}, code {}", key, code);
                    if code == "Print" {
                        use std::fs::File;
                        use std::io::Write;
                        let mut file = File::create("vm.json").ok().unwrap();
                        let buffer = serde_json::to_string(self).ok().unwrap();
                        file.write_all(buffer.as_ref()).ok().unwrap();
                        println!("VM state saved");
                        return;
                    }
                    if code == "Insert" {
                        use Machine;
                        let machine = Machine::new(&self.active_blueprint.upgrade().unwrap());
                    }
                    if code == "Delete" {
                        let bp = self.active_blueprint.upgrade().unwrap();
                        let mut bp = bp.borrow_mut();
                        let mc = bp.active_machine.upgrade().unwrap();
                        let mut idx = bp.machines.iter().enumerate().find(|tup| Rc::ptr_eq(tup.1, &mc)).unwrap().0;
                        if idx > 0 {
                            bp.machines.remove(idx);
                            if idx >= bp.machines.len() {
                                idx -= 1;
                            }
                            bp.active_machine = Rc::downgrade(&bp.machines[idx]);
                        }
                    }
                    if code == "PageDown" || code == "PageUp" {
                        let bp = self.active_blueprint.upgrade().unwrap();
                        let mut bp = bp.borrow_mut();
                        let mc = bp.active_machine.upgrade().unwrap();
                        let idx = bp.machines.iter().enumerate().find(|tup| Rc::ptr_eq(tup.1, &mc)).unwrap().0;
                        let delta = if code == "PageDown" { 1 } else { bp.machines.len()-1 };
                        let idx = (idx + delta) % bp.machines.len();
                        bp.active_machine = Rc::downgrade(&bp.machines[idx]);
                    }
                    if self.mouse_handler.is_some() {
                        return;
                    }
                    let menu = self.make_menu();
                    self.mouse_handler = match code.as_ref() {
                        "Delete" => self.activate_shortcut(menu, code.clone()),
                        "Space" => self.activate_shortcut(menu, code.clone()),
                        _ => None,
                    };
                    if let Some(weak) = self.mouse_object() {
                        let rc = weak.upgrade().unwrap();
                        {
                            let mut object = rc.borrow_mut();
                            if ref_eq::ref_eq(object.frame.borrow().typ, &text_type) {
                                if key.len() == 1 {
                                    let mut contents = object.data
                                        .downcast_mut::<String>().unwrap();
                                    contents.push_str(key.as_ref());
                                } else if key == "Backspace" {
                                    let mut contents = object.data
                                        .downcast_mut::<String>().unwrap();
                                    contents.pop();
                                }
                            }
                        }
                    }
                    self.update_clients();
                }
                Event::DisplaySize {
                    width: w,
                    height: h,
                } => {
                    self.display.size = PixelPoint::new(w, h);
                    println!("Display size is {} x {} px", w, h);
                    self.update_clients();
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
                    {
                        let start = self.mouse_world();
                        self.zoom = ScaleFactor::new(self.zoom.get() * (y/200.).exp());
                        let end = self.mouse_world();
                        let mut center = self.center.borrow_mut();
                        *center = *center - start + end;
                    }
                    self.update_clients();
                }
                _ => {}
            }
    }

    fn collect_args(&self, object: &ObjectCell) -> RunArgs {
        let object = object.borrow();
        let machine_rc = object.machine.upgrade().unwrap();
        let machine = machine_rc.borrow_mut();
        let frame = object.frame.borrow();
        let mut args = vec![];
        for param in frame.typ.parameters {
            args.push(vec![]);
        }
        let blueprint_rc = frame.blueprint.upgrade().unwrap();
        let blueprint = blueprint_rc.borrow();
        for link_rc in blueprint.links.iter() {
            let link = link_rc.borrow();
            if let &LinkTerminator::FrameParam(FrameParam {
                frame: ref frame_a,
                param_index: param_index,
            }) = &link.a {
                if !Rc::ptr_eq(frame_a, &object.frame) { continue; }
                if let &LinkTerminator::Frame(ref frame_b) = &link.b {
                    args[param_index].push(machine.get_object(frame_b));
                }
            }
        }
        return args;
    }

    fn process_task(&mut self, object: Weak<RefCell<Object>>) {
        if let Some(object_rc) = object.upgrade() {
            let args = self.collect_args(&object_rc);
            let object = object_rc.borrow();
            let typ = object.frame.borrow().typ;
            (typ.run)(args);
        }
    }

    pub fn run(&mut self) {
        while self.is_running {
            if let Ok(event) = self.rx.try_recv() {
                self.process_event(event);
            } else if let Some(task) = self.tasks.pop_front() {
                self.process_task(task);
            } else if let Ok(event) = self.rx.recv() {
                self.process_event(event);
            } else {
                println!("MAIN: Breaking main loop");
                break;
            }
        }
    }
}
