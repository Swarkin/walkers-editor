use super::osm::{Bbox, OsmClient, OsmResult, OsmToken, TargetServer};
use super::osmchange::Tag;
use osm_parser::OsmData;
use std::num::NonZeroU32;

#[cfg(not(target_family = "wasm"))]
use {
	crossbeam_channel::{Receiver, Sender},
	std::thread::JoinHandle,
};

#[cfg(target_family = "wasm")]
use futures::{
	channel::mpsc::{UnboundedReceiver as Receiver, UnboundedSender as Sender},
	stream::StreamExt,
};

pub enum Request {
	GetMap(Box<Bbox>), // box is used to keep enum size small
	SetTargetServer(TargetServer),
	FetchToken(String),
	CreateChangeset(Vec<Tag>),
	#[allow(dead_code)]
	CloseChangeset(NonZeroU32),
}

#[derive(Debug)]
pub enum Response {
	Map(OsmResult<OsmData>),
	Token(OsmResult<OsmToken>, TargetServer),
	CreatedChangeset(OsmResult<NonZeroU32>),
	ClosedChangeset(OsmResult<NonZeroU32>),
}

pub struct Worker {
	pub osm_client: OsmClient,
	pub sender: Sender<Response>,
}

impl Worker {
	pub fn send_message(&self, msg: Response) {
		#[cfg(not(target_family = "wasm"))]
		self.sender.send(msg).unwrap();
		#[cfg(target_family = "wasm")]
		self.sender.unbounded_send(msg).unwrap();
	}
}

pub struct WorkerHandle {
	#[cfg(not(target_family = "wasm"))]
	#[allow(dead_code)]
	pub thread: JoinHandle<()>,
	pub sender: Sender<Request>,
	pub receiver: Receiver<Response>,
}

impl WorkerHandle {
	pub fn send_message(&self, msg: Request) {
		#[cfg(not(target_family = "wasm"))]
		self.sender.send(msg).unwrap();
		#[cfg(target_family = "wasm")]
		self.sender.unbounded_send(msg).unwrap();
	}

	/// Returns all received messages without blocking.
	#[cfg(not(target_family = "wasm"))]
	pub fn recv_messages(&self) -> Vec<Response> {
		self.receiver.try_iter().collect::<Vec<_>>()
	}

	#[cfg(target_family = "wasm")]
	pub fn recv_messages(&mut self) -> Vec<Response> {
		let mut messages = vec![];
		while let Ok(msg) = self.receiver.try_next() {
			if let Some(msg) = msg {
				messages.push(msg);
			} else { panic!("receiver was closed unexpectedly"); }
		}
		messages
	}
}

impl Worker {
	#[cfg(target_family = "wasm")]
	#[allow(clippy::future_not_send)]
	async fn handle_message(&mut self, request: Request) {
		match request {
			Request::GetMap(bbox) => {
				let data = self.osm_client.get_map(&bbox);
				#[cfg(target_family = "wasm")] let data = data.await;

				self.send_message(Response::Map(data));
			}
			Request::SetTargetServer(target) => {
				self.osm_client.target_server = target;
			}
			Request::FetchToken(auth_code) => {
				let token = self.osm_client.fetch_token(auth_code);
				#[cfg(target_family = "wasm")] let token = token.await;

				let target_server = self.osm_client.target_server;

				if let Ok(token) = token.as_ref() {
					self.osm_client.auth_token[target_server as usize] = Some(token.to_owned());
				}

				self.send_message(Response::Token(token, target_server));
			}
			Request::CreateChangeset(tags) => {
				let result = self.osm_client.create_changeset(tags);
				#[cfg(target_family = "wasm")] let result = result.await;

				self.send_message(Response::CreatedChangeset(result));
			}
			Request::CloseChangeset(id) => {
				let result = self.osm_client.close_changeset(id);
				#[cfg(target_family = "wasm")] let result = result.await;

				self.send_message(Response::ClosedChangeset(result));
			}
		}
	}

	#[cfg(not(target_family = "wasm"))]
	fn handle_message(&mut self, request: Request) {
		match request {
			Request::GetMap(bbox) => {
				let data = self.osm_client.get_map(&bbox);
				self.send_message(Response::Map(data));
			}
			Request::SetTargetServer(target) => {
				self.osm_client.target_server = target;
			}
			Request::FetchToken(auth_code) => {
				let token = self.osm_client.fetch_token(auth_code);
				let target_server = self.osm_client.target_server;

				if let Ok(token) = token.as_ref() {
					self.osm_client.auth_token[target_server as usize] = Some(token.to_owned());
				}

				self.send_message(Response::Token(token, target_server));
			}
			Request::CreateChangeset(tags) => {
				let result = self.osm_client.create_changeset(tags);
				self.send_message(Response::CreatedChangeset(result));
			}
			Request::CloseChangeset(id) => {
				let result = self.osm_client.close_changeset(id);
				self.send_message(Response::ClosedChangeset(result));
			}
		}
	}

	#[cfg(target_family = "wasm")]
	#[allow(clippy::future_not_send)]
	pub async fn run(&mut self, mut receiver: Receiver<Request>) {
		while let Some(msg) = receiver.next().await {
			self.handle_message(msg).await;
		}
	}

	#[cfg(not(target_family = "wasm"))]
	pub fn run(&mut self, receiver: Receiver<Request>) {
		for msg in receiver {
			self.handle_message(msg);
		}
	}
}
