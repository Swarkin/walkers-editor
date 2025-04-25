use super::config::TargetServer;
use super::osm;
use crate::app::osm::OsmToken;
use crate::app::osmchange::Tag;
use crossbeam_channel::{Receiver, Sender};
use osm_parser::OsmData;
use std::num::NonZeroU32;
use std::thread::JoinHandle;

pub type AnyError = Box<dyn std::error::Error + Sync + Send>;

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
	GetMap(Box<osm::Bbox>), // box is used to keep enum size small
	SetTargetServer(TargetServer),
	FetchToken(String),
	CreateChangeset(Vec<Tag>),
	CloseChangeset(NonZeroU32),
}

#[derive(Debug)]
pub enum Response {
	Map(Result<Box<OsmData>, AnyError>),
	Token(Result<OsmToken, AnyError>, TargetServer),
	CreatedChangeset(Result<NonZeroU32, AnyError>),
	ClosedChangeset(Result<NonZeroU32, AnyError>),
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
				Request::FetchToken(auth_code) => {
					let target_server = self.osm_client.target_server;
					let token = self.osm_client.fetch_token(auth_code);
					if let Ok(token) = token.as_ref() {
						self.osm_client.auth_token.insert(target_server, token.to_owned());
					}

					self.sender.send(Response::Token(token, target_server)).unwrap();
				},
				Request::CreateChangeset(tags) => {
					let result = self.osm_client.create_changeset(tags);
					self.sender.send(Response::CreatedChangeset(result)).unwrap()
				}
				Request::CloseChangeset(id) => {
					let result = self.osm_client.close_changeset(id)
						.map(|_| id).map_err(|e| e.into());
					self.sender.send(Response::ClosedChangeset(result)).unwrap()
				}
			}
		}
	}
}
