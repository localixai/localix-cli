//! The tool client's connection to the daemon, made again when it is lost: the daemon stops (a
//! new build replaces it, `llx serve stop`) while the agent that started `llx mcp` goes on, and
//! its next call must find a daemon, not "the server is gone".

use std::io;
use std::sync::Mutex;
use std::time::Duration;

use client::{Begun, CallError, Client};
use protocol::{Command, Place};

/// How long the daemon has to take the handshake.
const ASK: Duration = Duration::from_secs(10);

pub(super) struct Link<F> {
    connect: F,
    place: Place,
    build: String,
    /// The connection and how many there have been, so that of several calls that find it gone
    /// only the first makes a new one.
    current: Mutex<(u64, Client)>,
}

impl<F: Fn() -> io::Result<Client>> Link<F> {
    /// Connect and say where this interface is; why not.
    pub(super) fn open(connect: F, place: Place, build: String) -> io::Result<Self> {
        let client = Self::handshake(&connect, &place, &build)?;
        Ok(Self {
            connect,
            place,
            build,
            current: Mutex::new((0, client)),
        })
    }

    fn handshake(connect: &F, place: &Place, build: &str) -> io::Result<Client> {
        let client = connect()?;
        let hello = Command::Initialize {
            version: protocol::VERSION,
            place: Some(place.clone()),
            build: build.to_string(),
        };
        client.call(hello, ASK).map_err(io::Error::other)?;
        Ok(client)
    }

    /// The connection now, and its number.
    pub(super) fn client(&self) -> (u64, Client) {
        let current = super::lock(&self.current);
        (current.0, current.1.clone())
    }

    /// A new connection, unless another call made one since `seen`.
    fn renew(&self, seen: u64) -> Result<(), CallError> {
        let mut current = super::lock(&self.current);
        if current.0 == seen {
            let made = Self::handshake(&self.connect, &self.place, &self.build)
                .map_err(|error| CallError::Failed(format!("the daemon is gone ({error})")))?;
            *current = (seen + 1, made);
        }
        Ok(())
    }

    /// `command` sent now; once more on a new connection when the daemon was gone.
    pub(super) fn begin(&self, command: Command) -> Result<Begun, CallError> {
        let (seen, client) = self.client();
        match client.begin(command.clone()) {
            Err(CallError::Gone) => {
                self.renew(seen)?;
                self.client().1.begin(command)
            }
            sent => sent,
        }
    }

    /// `command` answered within `wait`; once more on a new connection when the daemon was gone.
    pub(super) fn call(
        &self,
        command: Command,
        wait: Duration,
    ) -> Result<protocol::Done, CallError> {
        let (seen, client) = self.client();
        match client.call(command.clone(), wait) {
            Err(CallError::Gone) => {
                self.renew(seen)?;
                self.client().1.call(command, wait)
            }
            done => done,
        }
    }

    /// A call that died with the daemon: the next one finds a new one.
    pub(super) fn lost(&self, seen: u64) {
        let _ = self.renew(seen);
    }
}
