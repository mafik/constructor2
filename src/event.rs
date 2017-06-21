extern crate websocket;
extern crate serde_json;

use std::net::TcpStream;
use std::sync::mpsc;
use std::sync;
use std::cell::RefCell;
use ObjectCell;
use std::collections::linked_list;
use std::any::Any;

type Closure = Box<FnMut() + Send>;

pub enum Event {
    Quit(mpsc::Sender<i32>),
    NewWebsocketClient(websocket::Client<TcpStream>),
    Closure(Closure),
    RunUpdate(u64, Box<Any + Send>),
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
    pub fn from(json: serde_json::Value) -> Option<Event> {
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
