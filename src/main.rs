extern crate hyper;
extern crate websocket;
extern crate serde_json;

mod http;

use std::thread;
use std::sync::mpsc::channel;
use std::collections::HashMap;
use std::net::TcpStream;
use serde_json::Value;

fn main() {
    let (tx, rx) = channel();

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

    let mut websocket_clients = HashMap::new();
    let mut client_counter: i64 = 0;
    for message in rx.iter() {
        match message {
            Event::NewWebsocketClient(client) => {
                let (mut websocket_reader, websocket_writer) = client.split().unwrap();
                let client_number = client_counter;
                println!("New websocket client ({})!", client_number);
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
            _ => {}
        }
    }
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
