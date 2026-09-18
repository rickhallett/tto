//! One JSON line in, one JSON line out, over the daemon's unix socket.
//! Note what is absent: there is no "cancel". Not hidden. Absent.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::state::State;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum Request {
    Ping,
    Status,
    Off { until: u64, categories: Vec<String> },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub version: String,
}

impl Response {
    pub fn ok(state: State) -> Self {
        Self {
            ok: true,
            error: None,
            state,
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            state: State::default(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

#[derive(Debug)]
pub enum ClientError {
    NotInstalled,
    Io(std::io::Error),
    Daemon(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NotInstalled => write!(
                f,
                "the helper isn't installed. Run `sudo tto install`, or open the menu bar app and choose Install"
            ),
            ClientError::Io(e) => write!(f, "can't talk to the helper: {e}"),
            ClientError::Daemon(e) => write!(f, "{e}"),
        }
    }
}

pub fn call(socket: &Path, req: &Request) -> Result<Response, ClientError> {
    let mut stream = match UnixStream::connect(socket) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ClientError::NotInstalled);
        }
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            return Err(ClientError::NotInstalled);
        }
        Err(e) => return Err(ClientError::Io(e)),
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(ClientError::Io)?;
    let mut line = serde_json::to_string(req).expect("serialisable");
    line.push('\n');
    stream.write_all(line.as_bytes()).map_err(ClientError::Io)?;
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(ClientError::Io)?;
    let resp: Response =
        serde_json::from_str(&reply).map_err(|e| ClientError::Daemon(format!("bad reply: {e}")))?;
    if resp.ok {
        Ok(resp)
    } else {
        Err(ClientError::Daemon(resp.error.unwrap_or_default()))
    }
}
