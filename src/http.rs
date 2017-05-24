extern crate websocket;
extern crate hyper;
extern crate serde_json;

use std::thread;
use self::hyper::header::{ContentLength, ContentType};
use self::hyper::mime::{Mime, TopLevel, SubLevel};
use self::hyper::server::{Request, Response};
use self::hyper::status::StatusCode;
use self::hyper::uri::RequestUri;

const HEADER: &'static str = include_str!("html/index.html");
const SCRIPT: &'static str = include_str!("html/script.js");

pub fn start_thread() -> thread::JoinHandle<()> {
    thread::spawn(|| {
        let addr = format!("0.0.0.0:{}", port());
        let server = hyper::Server::http(addr).unwrap();
        let index = format!("{}<script>{}</script>", HEADER, SCRIPT);
        server
            .handle(move |req: Request, mut res: Response| {
                let get_response = match req.uri {
                    RequestUri::AbsolutePath(path) => {
                        match path.as_ref() {
                            "/" => index.as_bytes(),
                            "/favicon.ico" => {
                                res.headers_mut()
                                    .set(ContentType(Mime(TopLevel::Image,
                                                          SubLevel::Ext("x-icon".to_string()),
                                                          vec![])));
                                include_bytes!("html/favicon.ico")
                            }
                            _ => {
                                *res.status_mut() = StatusCode::NotFound;
                                b"404"
                            }
                        }
                    }
                    _ => {
                        *res.status_mut() = StatusCode::MethodNotAllowed;
                        b"502"
                    }
                };
                res.headers_mut()
                    .set(ContentLength(get_response.len() as u64));
                res.send(get_response).unwrap();
            })
            .unwrap();
    })
}

pub fn port() -> i32 {
    8080
}
