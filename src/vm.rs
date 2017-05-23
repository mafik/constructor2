use http;
use std;
use canvas::Canvas;

#[derive(Debug)]
pub struct Window {
    pub id: i64,
    pub commands_tx: std::sync::mpsc::Sender<Command>,
}

pub enum Command {
    
}

#[derive(Debug)]
pub enum Event {
    Start(Window),
    Quit,
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

pub struct Vm {
    events_tx : std::sync::mpsc::Sender<Event>,
    events_rx : std::sync::mpsc::Receiver<Event>,
    server_thread : std::thread::JoinHandle<()>,
}

impl Vm {
    pub fn new() -> Vm {
        let (tx, rx) = std::sync::mpsc::channel();
        Vm {
            events_tx : tx.clone(),
            events_rx : rx,
            server_thread : http::start_thread(tx.clone()),
        }
    }

    pub fn run_forever(mut self) {
        for event in self.events_rx {
            println!("event: {:?}", event);
        }
        self.server_thread.join();
    }
}
