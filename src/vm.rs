extern crate websocket;
extern crate rusttype;
extern crate serde_json;

use std::rc::{Rc, Weak};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::net::TcpStream;
use std::time::Instant;
use std::thread;

use blueprint::*;
use json_canvas::*;
use canvas::*;
use Task;
use TaskArg;
use RunArgs;
use event::*;
use Display;
use WorldPoint;
use PixelPoint;
use TouchReceiver;
use LinkTerminator;
use FrameParam;
use Visible;
use http;
use touch::*;

static FONT: &'static [u8] = include_bytes!("html/fonts/iosevka-regular.ttf");

fn start_touch<V: Visible>(v: &Vec<V>, p: &WorldPoint) -> Option<Box<TouchReceiver>> {
    walk_visible(v, |elem| { elem.start_touch(p) })
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

pub struct Vm {
    pub blueprints: Vec<Rc<RefCell<Blueprint>>>,
    pub active_blueprint: Weak<RefCell<Blueprint>>,

    tasks: VecDeque<Task>,

    rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
    websocket_clients: HashMap<i64, websocket::sender::Writer<TcpStream>>,
    client_counter: i64,
    font: Rc<rusttype::Font<'static>>,

    // Per-client parameters:
    display: Display,
    center: Rc<RefCell<WorldPoint>>,
    mouse: PixelPoint,
    last_update: Instant,
    mouse_handler: Option<Box<TouchReceiver>>,
}

impl Vm {
    fn query_task(&self) -> Option<Task> {
        let world_point = self.mouse_world();
        let blueprint_rc = self.active_blueprint.upgrade().unwrap();
        let blueprint = blueprint_rc.borrow();
        let frame_rc = blueprint.query_frame(world_point);
        if frame_rc.is_none() { return None; }
        let frame_rc = frame_rc.unwrap();
        let mut task = Task {
            machine: blueprint.active_machine.clone(),
            frame: Rc::downgrade(&frame_rc),
            args: vec![],
        };
        for param in frame_rc.borrow().typ.parameters {
            task.args.push(vec![]);
        }
        for link_rc in blueprint.links.iter() {
            let link = link_rc.borrow();
            if let &LinkTerminator::FrameParam(FrameParam {
                frame: ref frame_a,
                param_index: param_index,
            }) = &link.a {
                if !Rc::ptr_eq(frame_a, &frame_rc) { continue; }
                if let &LinkTerminator::Frame(ref frame_b) = &link.b {
                    task.args[param_index].push(Rc::downgrade(frame_b));
                }
            }
        }
        return Some(task);
    }
    fn mouse_world(&self) -> WorldPoint {
        self.display.to_world(self.mouse) - *self.center.borrow()
    }
    pub fn new() -> Vm {
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

        Vm {
            blueprints: Vec::new(),
            active_blueprint: Weak::new(),
            tasks: VecDeque::new(),
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
            last_update: Instant::now(),
            mouse_handler: None,
        }
    }

    fn update_clients(&mut self) {
        let mut c = JsonCanvas::new(self.font.clone());
        c.save();
        c.font(format!("{}px Iosevka", 6.).as_str());

        c.translate(self.display.size.x / 2., self.display.size.y / 2.);
        c.scale(self.display.pixel_size().inv().get());
        {
            let center = self.center.borrow();
            c.translate(center.x, center.y);
        }

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

        fn draw<V: Visible>(v: &Vec<V>, c: &mut Canvas) {
            walk_visible(v, |elem| -> Option<()> {
                c.save();
                elem.draw(c);
                c.restore();
                None
            });
        }

        draw(&blueprint.frames, c);
        draw(&blueprint.links, c);
    }

    fn process_event(&mut self, event: Event) {
        match event {
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
                    let mut frames;
                    {
                        let blueprint = self.active_blueprint.upgrade().unwrap();
                        let blueprint = blueprint.borrow();
                        frames = blueprint.frames.clone();
                    }
                    let pixel_point = PixelPoint::new(x, y);
                    let world_point = self.mouse_world();
                    self.mouse_handler = match button {
                        0 => start_touch(&frames, &world_point),
                        1 => Some(Box::new(NavTouchReceiver{ nav: Rc::downgrade(&self.center), last_pos: world_point })),
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
                    match self.mouse_handler.take() {
                        Some(touch_receiver) => touch_receiver.end_touch(),
                        None => (),
                    }
                    self.update_clients();
                }
                Event::MouseMove { x: x, y: y } => {
                    self.mouse = PixelPoint::new(x, y);

                    let taken = self.mouse_handler.take();
                    let taken = taken.and_then(|b| b.continue_touch(self.mouse_world()));
                    self.mouse_handler = taken;
                    
                    if self.mouse_handler.is_some() {
                        self.update_clients();
                    }
                }
                Event::KeyDown { code: code, key: key } => {
                    println!("Pressed key {}, code {}", key, code);
                    if code == "Enter" {
                        if let Some(task) = self.query_task() {
                            self.tasks.push_back(task);
                        }
                    }
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
                        self.display.eye_distance_meters *= (y/-200.).exp();
                        let end = self.mouse_world();
                        let mut center = self.center.borrow_mut();
                        *center = *center - start + end;
                    }
                    self.update_clients();
                }
                _ => {}
            }
    }

    fn process_task(&mut self, task: Task) {
        if let Some(machine_rc) = task.machine.upgrade() {
            let mut machine = machine_rc.borrow_mut();
            if let Some(frame_rc) = task.frame.upgrade() {
                let mut run_args: RunArgs = vec![];
                for task_arg in task.args.into_iter() {
                    let mut run_arg = vec![];
                    for weak_frame in task_arg.into_iter() {
                        let frame_rc = weak_frame.upgrade().unwrap();
                        let mut object = machine.get_object(&frame_rc);
                        run_arg.push(object);
                    }
                    run_args.push(run_arg);
                }
                let typ = frame_rc.borrow().typ;
                (typ.run)(run_args);
            }
        }
    }

    pub fn run(mut self) {
        loop {
            if let Ok(event) = self.rx.try_recv() {
                self.process_event(event);
            } else if let Some(task) = self.tasks.pop_front() {
                self.process_task(task);
            } else if let Ok(event) = self.rx.recv() {
                self.process_event(event);
            }
        }
    }
}