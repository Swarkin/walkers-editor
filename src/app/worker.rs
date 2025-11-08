use super::osm::{Bbox, OrderedTags, OsmClient, OsmResult, OsmToken, TargetServer};
use osm_parser::OsmData;

#[cfg(not(target_family = "wasm"))]
use {
	crossbeam_channel::{Receiver, Sender},
	std::thread::JoinHandle,
};

use crate::app::osmchange::{ChangesetId, OsmChange, Tag};
#[cfg(target_family = "wasm")]
use futures::{
	channel::mpsc::{UnboundedReceiver as Receiver, UnboundedSender as Sender},
	stream::StreamExt,
};

pub enum Request { // box is used to keep enum size small
	GetMap(Box<Bbox>),
	SetTargetServer(TargetServer),
	FetchToken(String),
	UploadChanges { tags: Box<OrderedTags>, osmchange: Box<OsmChange> },
}

pub enum Response {
	Map(OsmResult<OsmData>),
	Token(OsmResult<OsmToken>, TargetServer),
	UploadChangesProgress(UploadChangesProgress),
}

pub enum UploadChangesProgress {
	ChangesetCreated(OsmResult<ChangesetId>),
	DiffUploaded(OsmResult<String>),
	ChangesetClosed(OsmResult<()>),
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
	#[cfg(not(target_family = "wasm"))]
	fn handle_message(&mut self, request: Request) {
		match request {
			Request::GetMap(bbox) => {
				let result = self.osm_client.get_map(&bbox);
				self.send_message(Response::Map(result));
			}
			Request::SetTargetServer(target) => {
				self.osm_client.target_server = target;
			}
			Request::FetchToken(auth_code) => {
				let result = self.osm_client.fetch_token(auth_code);
				let target_server = self.osm_client.target_server;

				if let Ok(token) = result.as_ref() {
					self.osm_client.auth_token[target_server as usize] = Some(token.to_owned());
				}

				self.send_message(Response::Token(result, target_server));
			}
			Request::UploadChanges { tags, mut osmchange } => {
				let tags = tags.into_iter().map(|(k, v)| Tag { k, v }).collect();

				let changeset_result = self.osm_client.create_changeset(tags);
				let changeset_id = changeset_result.as_ref().ok().copied();

				self.send_message(Response::UploadChangesProgress(
					UploadChangesProgress::ChangesetCreated(changeset_result),
				));

				if let Some(changeset_id) = changeset_id {
					osmchange.prepare_upload(changeset_id);
					let diff_result = self.osm_client.diff_upload(changeset_id, osmchange.to_string_pretty().unwrap());
					self.send_message(Response::UploadChangesProgress(
						UploadChangesProgress::DiffUploaded(diff_result),
					));

					let close_result = self.osm_client.close_changeset(changeset_id);
					self.send_message(Response::UploadChangesProgress(
						UploadChangesProgress::ChangesetClosed(close_result),
					));
				}
			}
		}
	}

	#[cfg(target_family = "wasm")]
	#[allow(clippy::future_not_send)]
	async fn handle_message(&mut self, request: Request) {
		match request {
			Request::GetMap(bbox) => {
				let result = self.osm_client.get_map(&bbox).await;
				self.send_message(Response::Map(result));
			}
			Request::SetTargetServer(target) => {
				self.osm_client.target_server = target;
			}
			Request::FetchToken(auth_code) => {
				let result = self.osm_client.fetch_token(auth_code).await;
				let target_server = self.osm_client.target_server;

				if let Ok(token) = result.as_ref() {
					self.osm_client.auth_token[target_server as usize] = Some(token.to_owned());
				}

				self.send_message(Response::Token(result, target_server));
			}
			Request::UploadChanges { tags, mut osmchange } => {
				let tags = tags.into_iter().map(|(k, v)| Tag { k, v }).collect();

				let changeset_result = self.osm_client.create_changeset(tags).await;
				let changeset_id = changeset_result.as_ref().ok().copied();

				self.send_message(Response::UploadChangesProgress(
					UploadChangesProgress::ChangesetCreated(changeset_result),
				));

				if let Some(changeset_id) = changeset_id {
					osmchange.prepare_upload(changeset_id);
					let diff_result = self.osm_client.diff_upload(changeset_id, osmchange.to_string_pretty().unwrap()).await;
					self.send_message(Response::UploadChangesProgress(
						UploadChangesProgress::DiffUploaded(diff_result),
					));

					let close_result = self.osm_client.close_changeset(changeset_id).await;
					self.send_message(Response::UploadChangesProgress(
						UploadChangesProgress::ChangesetClosed(close_result),
					));
				}
			}
		}
	}

	#[cfg(not(target_family = "wasm"))]
	pub fn run(&mut self, receiver: Receiver<Request>) {
		for msg in receiver {
			self.handle_message(msg);
		}
	}

	#[cfg(target_family = "wasm")]
	#[allow(clippy::future_not_send)]
	pub async fn run(&mut self, mut receiver: Receiver<Request>) {
		while let Some(msg) = receiver.next().await {
			self.handle_message(msg).await;
		}
	}
}
