extern crate websocket;
extern crate hyper;
extern crate serde_json;

use std::thread;
use std;
use vm;
use self::hyper::header::{ContentLength, ContentType};
use self::hyper::mime::{Mime, TopLevel, SubLevel};
use self::hyper::server::{Server as HyperServer, Request, Response, Handler};
use self::hyper::status::StatusCode;
use self::hyper::uri::RequestUri;
use self::websocket::server::upgrade::from_hyper::HyperRequest;
use self::websocket::server::upgrade::IntoWs;
use self::websocket::message::Type;
use self::websocket::Message;
use self::serde_json::Value;

const HEADER: &'static str = include_str!("html/index.html");
const SCRIPT: &'static str = include_str!("html/script.js");

struct EventServer {
    tx : std::sync::Mutex<std::sync::mpsc::Sender<vm::Event>>,
}

impl vm::Event {
    fn withJson(json: Value) -> Option<vm::Event> {
        let obj = json.as_object().unwrap();
        let typ = obj.get("type").unwrap().as_str().unwrap();
        match typ.as_ref() {
            "size" => Some(vm::Event::WindowSize{
                width: obj.get("width").unwrap().as_f64().unwrap(),
                height: obj.get("height").unwrap().as_f64().unwrap(),
            }),
            "mouse_move" => Some(vm::Event::MouseMove{
                x: obj.get("x").unwrap().as_f64().unwrap(),
                y: obj.get("y").unwrap().as_f64().unwrap(),
            }),
            "mouse_down" => Some(vm::Event::MouseDown{
                x: obj.get("x").unwrap().as_f64().unwrap(),
                y: obj.get("y").unwrap().as_f64().unwrap(),
                button: obj.get("button").unwrap().as_i64().unwrap(),
            }),
            "mouse_up" => Some(vm::Event::MouseUp{
                x: obj.get("x").unwrap().as_f64().unwrap(),
                y: obj.get("y").unwrap().as_f64().unwrap(),
                button: obj.get("button").unwrap().as_i64().unwrap(),
            }),
            "render_done" => Some(vm::Event::RenderingDone),
            "render_ready" => Some(vm::Event::RenderingReady),
            "key_up" => Some(vm::Event::KeyUp{
                key: String::from(obj.get("key").unwrap().as_str().unwrap()),
                code: String::from(obj.get("code").unwrap().as_str().unwrap()),
            }),
            "key_down" => Some(vm::Event::KeyDown{
                key: String::from(obj.get("key").unwrap().as_str().unwrap()),
                code: String::from(obj.get("code").unwrap().as_str().unwrap()),
            }),
            "wheel" => Some(vm::Event::MouseWheel{
                x: obj.get("x").unwrap().as_f64().unwrap(),
                y: obj.get("y").unwrap().as_f64().unwrap(),
            }),
            _ => {
                println!("Unknown vm::Event: {:?}", json);
                None
            },
        }
    }
}

/*
    KeyDown { code: String, key: String },
    KeyUp { code: String, key: String },
    Wheel { y: f32 },
*/

use std::sync::atomic::AtomicPtr;

impl Handler for EventServer {
    fn handle(&self, req: Request, mut res: Response) {
        match HyperRequest(req).into_ws() {
            Ok(upgrade) => {
                // `accept` sends a successful handshake, no need to worry about res
                let mut client = match upgrade.accept() {
                    Ok(c) => c,
                    Err(_) => panic!(),
                };
                let (tx, rx) = std::sync::mpsc::channel();
                let window = vm::Window {
                    id : 0,
                    commands_tx : tx.clone(),
                };
                self.tx.lock().unwrap().send(vm::Event::Start(window)).unwrap();
                for message in client.incoming_messages() {
                    let mut message: Message = match message {
                        Ok(message) => message,
                        Err(_) => break,
                    };

                    let event = match message.opcode {
                        Type::Close => break,
                        Type::Text => {
                            let payload = message.payload.to_mut();
                            let json : Value = serde_json::from_slice(payload).unwrap();
                            vm::Event::withJson(json)
                        },
                        _ => None,
                    };
                    
                    match event {
                        Some(event) => {
                            self.tx.lock().unwrap().send(event).unwrap();
                        },
                        None => {}
                    }
                }
                self.tx.lock().unwrap().send(vm::Event::Quit).unwrap();
                //client.send_message(&Message::text("{}")).unwrap();
            },
            Err((req, _)) => {
                let index = format!("{}<script>{}</script>", HEADER, SCRIPT);
                let get_response = match req.uri {
                    RequestUri::AbsolutePath(path) => {
                        match path.as_ref() {
                            "/" => {
                                index.as_bytes()
                            },
                            "/favicon.ico" => {
                                res.headers_mut().set(ContentType(Mime(
                                    TopLevel::Image,
                                    SubLevel::Ext("x-icon".to_string()),
                                    vec![])));
                                include_bytes!("html/favicon.ico")
                            },
                            _ => {
                                *res.status_mut() = StatusCode::NotFound;
                                b"404"
                            },
                        }
                    },
                    _ => {
                        *res.status_mut() = StatusCode::MethodNotAllowed;
                        b"502"
                    },
                };
                res.headers_mut().set(ContentLength(get_response.len() as u64));
                res.send(get_response).unwrap();
            },
        };
    }
}

pub fn start_thread(events : std::sync::mpsc::Sender<vm::Event>) -> thread::JoinHandle<()> {
    let handle = thread::spawn(move || {
        let addr = format!("0.0.0.0:{}", port());
        HyperServer::http(addr).unwrap().handle(EventServer { tx: std::sync::Mutex::new(events) }).unwrap();
    });
    handle
}

pub fn port() -> i32 {
    8000
}
