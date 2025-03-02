use crossbeam_channel::{Receiver, Sender};
use osm_parser::OsmData;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;

pub struct Worker {
	pub http_client: reqwest::Client,
	pub sender: Sender<Response>,
	pub receiver: Receiver<Request>,
}

pub struct WorkerHandle {
	pub thread: JoinHandle<()>,
	pub sender: Sender<Request>,
	pub receiver: Receiver<Response>,
}

pub enum Request {
	GetMap(super::osm::Bbox),
}

pub enum Response {
	Map(Result<Box<OsmData>, Box<dyn std::error::Error + Sync + Send>>),
}

impl Worker {
	pub fn run(&self) {
		for request in self.receiver.iter() {
			match request {
				Request::GetMap(bbox) => {
					dbg!(bbox);
					sleep(Duration::from_secs(1));
					self.sender.send(Response::Map(Ok(Box::from(OsmData::default())))).unwrap();
				}
			}
		}
	}
}
