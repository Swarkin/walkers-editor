use super::config::TargetServer;
use super::osm;
use crossbeam_channel::{Receiver, Sender};
use osm_parser::OsmData;
use std::thread::JoinHandle;

pub struct Worker {
	pub osm_client: osm::OsmClient,
	pub sender: Sender<Response>,
	pub receiver: Receiver<Request>,
}

pub struct WorkerHandle {
	pub thread: JoinHandle<()>,
	pub sender: Sender<Request>,
	pub receiver: Receiver<Response>,
}

pub enum Request {
	GetMap(osm::Bbox),
	SetTargetServer(TargetServer),
	RequestToken(String),
}

#[derive(Debug)]
pub enum Response {
	Map(Result<Box<OsmData>, Box<dyn std::error::Error + Sync + Send>>),
	Token(String),
}

impl Worker {
	pub fn run(&mut self) {
		for request in self.receiver.iter() {
			match request {
				Request::GetMap(bbox) => {
					let data = self.osm_client.get_map(&bbox);
					self.sender.send(Response::Map(Ok(Box::from(data)))).unwrap();
				},
				Request::SetTargetServer(target) => {
					self.osm_client.target_server = target;
				},
				Request::RequestToken(auth_code) => {
					let target_server = self.osm_client.target_server;
					let token = self.osm_client.fetch_token(auth_code);
					self.sender.send(Response::Token(token.access_token.clone())).unwrap();
					self.osm_client.auth_token.insert(target_server, token);
				},
			}
		}
	}
}
