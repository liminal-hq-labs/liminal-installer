// The agent server's life in the app: started, stopped and moved by the settings, with a token a person can reveal or replace.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use li_studio::{Studio, mcp::McpServer};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentInfo {
	pub url: String,
	pub port: u16,
}

pub struct Agent {
	studio: Arc<Studio>,
	server: Mutex<Option<(McpServer, u16)>>,
}

impl Agent {
	pub fn new(studio: Arc<Studio>) -> Self {
		Self {
			studio,
			server: Mutex::new(None),
		}
	}

	fn lock(&self) -> std::sync::MutexGuard<'_, Option<(McpServer, u16)>> {
		self.server.lock().unwrap_or_else(|e| e.into_inner())
	}

	/// Makes the server match the settings: stopped when disabled, otherwise listening on `port` (0 for any free
	/// one). A port that is taken is refused with the reason and the server that was running keeps running.
	pub fn apply(&self, enabled: bool, port: u32) -> Result<(), String> {
		let port = u16::try_from(port).map_err(|_| "the port is out of range".to_string())?;
		let mut slot = self.lock();
		if !enabled {
			*slot = None;
			return Ok(());
		}
		if slot.as_ref().is_some_and(|(_, asked)| *asked == port) {
			// Already serving as asked; a token already handed out stays valid.
			return Ok(());
		}
		// Probe first, so a taken port leaves the running server alone.
		let held_by_us = slot.as_ref().is_some_and(|(s, _)| s.addr.port() == port);
		if port != 0 && !held_by_us {
			TcpListener::bind(("127.0.0.1", port)).map_err(|e| {
				if e.kind() == std::io::ErrorKind::AddrInUse {
					format!("port {port} is in use")
				} else {
					format!("port {port} cannot be used: {e}")
				}
			})?;
		}
		*slot = None;
		let server = McpServer::start(self.studio.clone(), port)
			.map_err(|e| format!("the agent server did not start: {e}"))?;
		*slot = Some((server, port));
		Ok(())
	}

	pub fn info(&self) -> Option<AgentInfo> {
		self.lock().as_ref().map(|(s, _)| AgentInfo {
			url: format!("http://{}/mcp", s.addr),
			port: s.addr.port(),
		})
	}

	/// The token, only when the server runs.
	pub fn token(&self) -> Option<String> {
		self.lock().as_ref().map(|(s, _)| s.token())
	}

	pub fn regenerate_token(&self) -> Result<String, String> {
		match self.lock().as_ref() {
			Some((s, _)) => s.regenerate_token().map_err(|e| e.to_string()),
			None => Err("the agent server is off".into()),
		}
	}

	pub fn banner(&self) -> Option<String> {
		self.lock().as_ref().map(|(s, _)| s.banner())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use li_studio::MockBackend;

	fn agent() -> Agent {
		Agent::new(Arc::new(Studio::new(Box::new(MockBackend))))
	}

	#[test]
	fn it_starts_stops_and_hides_its_token_while_off() {
		let a = agent();
		assert!(a.info().is_none() && a.token().is_none());
		assert!(a.regenerate_token().is_err());
		a.apply(true, 0).unwrap();
		let info = a.info().unwrap();
		assert!(info.url.starts_with("http://127.0.0.1:"));
		assert_eq!(a.token().unwrap().len(), 48);
		a.apply(false, 0).unwrap();
		assert!(a.info().is_none());
	}

	#[test]
	fn applying_the_same_settings_keeps_the_token() {
		let a = agent();
		a.apply(true, 0).unwrap();
		let token = a.token().unwrap();
		a.apply(true, 0).unwrap();
		assert_eq!(a.token().unwrap(), token);
	}

	#[test]
	fn regenerating_gives_a_new_token() {
		let a = agent();
		a.apply(true, 0).unwrap();
		let old = a.token().unwrap();
		let fresh = a.regenerate_token().unwrap();
		assert_ne!(old, fresh);
		assert_eq!(a.token().unwrap(), fresh);
	}

	#[test]
	fn a_taken_port_is_refused_and_the_running_server_stays() {
		let a = agent();
		a.apply(true, 0).unwrap();
		let before = a.info().unwrap();
		let held = TcpListener::bind(("127.0.0.1", 0)).unwrap();
		let port = held.local_addr().unwrap().port();
		let refused = a.apply(true, u32::from(port)).unwrap_err();
		assert!(refused.contains("in use"), "{refused}");
		assert_eq!(a.info().unwrap(), before);
	}

	#[test]
	fn a_chosen_port_is_used() {
		let free = {
			let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
			l.local_addr().unwrap().port()
		};
		let a = agent();
		a.apply(true, u32::from(free)).unwrap();
		assert_eq!(a.info().unwrap().port, free);
	}
}
