extern crate ws;
extern crate hyper;
extern crate config;


use std::fs;
use ws::{Sender, Handler, Handshake, Result};
use ws::util::Token;
use std::thread;
use std::sync::{Arc, Mutex};


use hyper::{Body, Response, Server};
use hyper::rt::Future;
use hyper::service::service_fn_ok;

use std::collections::HashMap;



const PONG: Token = Token(1);
const DATA: Token = Token(2);


struct WSServer {
	out: Sender,
	shared_message: Arc<Mutex<SharedMessage>>,
	shared_message_last_serial: u32,
}

impl WSServer {
	fn new(out: Sender, shared_message: Arc<Mutex<SharedMessage>>) -> Self {
		let tmp = shared_message.lock().unwrap();
		let current_serial = tmp.serial;
		drop(tmp);

		WSServer {
			out: out,
			shared_message: shared_message,
			shared_message_last_serial: current_serial,
		}
	}
}

impl Handler for WSServer {
	fn on_open(&mut self, _: Handshake) -> Result<()> {
		self.out.timeout(1000, PONG).unwrap();
		self.out.timeout(100,  DATA).unwrap();
		self.out.send("VERSION 0.1").unwrap();
		Ok(())
	}

	fn on_timeout(&mut self, event: Token) -> Result<()> {
		if event == PONG {
			self.out.send("PONG").unwrap();
			self.out.timeout(1000, PONG).unwrap();
			Ok(())

		} else if event == DATA {
			let msg = self.shared_message.lock().unwrap();
			if msg.serial != self.shared_message_last_serial {
				self.out.send(msg.msg.clone()).unwrap();
				self.shared_message_last_serial = msg.serial;
			}
			self.out.timeout(200, DATA).unwrap();
			Ok(())

		} else {
			Ok(())
		}
	}
}


#[derive(Debug)]
struct SharedMessage {
	serial: u32,
	msg: String,
}

fn main () 
{
	//
	// Read config and content files
	//
	let mut settings = config::Config::default();
	println!("Reading configuration");
	if let Err(e) = settings.merge(config::File::with_name("Settings.toml")) {
		println!("Cannot load configuration file \"Settings.toml\": {}", e);
		return;
	}

	let html = match settings.get_str("global.main") {
		Ok(f) => {
			println!("Reading index page contents from file {}", f.clone());
			match fs::read_to_string(f.clone()) {
				Ok(v) => v,
				Err(e) => {
					println!("Error reading file {}: {}", f.clone(), e);
					return;
				},
			}
		},
		Err(_e) => {
			println!("Config option \"main\" in section \"global\" is missing");
			return;
		}
	};

	// other options
	let ws_listen: std::net::SocketAddr = match settings.get_str("global.ws_listen") {
		Ok(v) => match v.clone().parse() {
			Ok(a) => a,
			Err(e) => {
				println!("Cannot use socket address \"{}\": {}", v.clone(), e);
				return;
			},
		},
		Err(_e) => {
			println!("Config option \"ws_listen\" in section \"global\" is missing");
			return;
		},
	};

	let http_listen: std::net::SocketAddr = match settings.get_str("global.http_listen") {
		Ok(v) => match v.clone().parse() {
			Ok(a) => a,
			Err(e) => {
				println!("Cannot use socket address \"{}\": {}", v.clone(), e);
				return;
			},
		},
		Err(_e) => {
			println!("Config option \"http_listen\" in section \"global\" is missing");
			return;
		},
	};

	// content files
	let mut contents: HashMap<String, String> = HashMap::new();

	match settings.get_table("proxy") {
		Err(_e) => {
			println!("Config section \"proxy\" is missing");
			return;
		},
		Ok(map) => {
			for (k, v) in map {
				match v.into_str() {
					Ok(f) => {
						println!("Reading content for action \"{}\" from file \"{}\"", k.clone(), f.clone());
						match fs::read_to_string(f.clone()) {
							Ok(content) => {
								println!("Loaded content for action \"{}\"", k.clone());
								contents.insert(format!("/{}", k.clone()), content);
							},
							Err(e) => {
								println!("Cannot read content from file \"{}\": {}", f.clone(), e);
								return;
							},
						}
					},
					Err(e) => {
						println!("Cannot get file name for action \"{}\": {}", k.clone(), e);
						return;
					},
				};
			}
		},
	}

	//
	// Prepare shared message struct to be used for communication between
	// the regular webserver and the websocket server
	//
	let shared_message = Arc::new(Mutex::new(SharedMessage { serial: 0, msg: "".to_string() }));
	let tmp = shared_message.clone();

	//
	// Start Websocket server in separate thread
	//
	println!("Websocket server listening on \"{}\"", ws_listen.clone());
	let _ws_server = thread::spawn(move || {
		ws::listen(ws_listen, |out| {
			WSServer::new(out, tmp.clone())
		}).unwrap();
	});

	//
	// Start regular webserver and stay in its loop
	//
	let new_svc = move || {
		let cloned_message = shared_message.clone();
		let my_html = html.clone();
		let routing = contents.clone();

		service_fn_ok(move |req| {
			println!("Incoming request: {:?}", req);

			for (k, v) in routing.clone() {
				if req.uri() == k.as_str() {
					println!("Request matched \"{}\" action", k.clone());
					let mut tmp = cloned_message.lock().unwrap();
					if tmp.serial == std::u32::MAX { tmp.serial = 0 }
					tmp.serial += 1;
					tmp.msg = v.clone();
				}
			}

			Response::new(Body::from(my_html.clone()))
		})
	};

	println!("HTTP server listening on \"{}\"", http_listen.clone());
	let server = Server::bind(&http_listen)
		.serve(new_svc)
		.map_err(|e| eprintln!("server error: {}", e));

	hyper::rt::run(server);
}
