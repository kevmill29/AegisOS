use process_monitor::GameSession;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, watch};
use tracing::{debug, info, warn};

/// State-transition events the front-end subscribes to. Serialized as one JSON
/// object per line ("NDJSON") over a localhost TCP socket — TCP rather than a Unix
/// socket because the dev topology (daemon in WSL2, Electron on Windows) can only
/// share localhost TCP across the VM boundary, and on the real single-host target
/// the two are equivalent for this traffic.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Event {
    /// Sent to every client immediately on connect, so a frontend that starts
    /// after the daemon (or reconnects mid-game) doesn't have to guess the
    /// current state from silence.
    Hello { sleeping: bool },
    SleepModeInitiated {
        app_id: Option<String>,
        game_name: Option<String>,
    },
    SleepModeEnded,
}

impl Event {
    pub fn from_session(session: &GameSession) -> Self {
        Event::SleepModeInitiated {
            app_id: session.app_id.clone(),
            game_name: session.game_name.clone(),
        }
    }

    fn is_sleep_start(&self) -> bool {
        matches!(self, Event::SleepModeInitiated { .. })
    }
}

/// Fans events out to stdout (for journalctl greppability) and to every connected
/// TCP client. Slow/dead clients are dropped rather than allowed to backpressure
/// the daemon — the frontend is a consumer of this process's state, never a thing
/// it waits on.
pub struct EventBus {
    tx: broadcast::Sender<String>,
    sleeping: watch::Sender<bool>,
}

impl EventBus {
    /// Binds the listener and spawns the accept loop. Fails fast if the port is
    /// taken (most likely: a second daemon instance) — better than silently
    /// running without a frontend transport.
    pub async fn start(addr: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        info!(%addr, "event bus listening");

        let (tx, _) = broadcast::channel::<String>(64);
        let (sleeping_tx, sleeping_rx) = watch::channel(false);

        let accept_tx = tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        debug!(%peer, "frontend connected");
                        tokio::spawn(serve_client(
                            stream,
                            accept_tx.subscribe(),
                            *sleeping_rx.borrow(),
                        ));
                    }
                    Err(e) => warn!(error = %e, "accept failed"),
                }
            }
        });

        Ok(Self {
            tx,
            sleeping: sleeping_tx,
        })
    }

    pub fn emit(&self, event: &Event) {
        let json = match serde_json::to_string(event) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!(error = %e, "failed to serialize event");
                return;
            }
        };
        println!("AEGIS_EVENT: {json}");
        self.sleeping.send_replace(event.is_sleep_start());
        // Err here just means no clients are connected right now; that's fine.
        let _ = self.tx.send(json);
    }
}

async fn serve_client(
    mut stream: TcpStream,
    mut rx: broadcast::Receiver<String>,
    sleeping_now: bool,
) {
    let hello = serde_json::to_string(&Event::Hello {
        sleeping: sleeping_now,
    })
    .expect("Hello serializes");
    if stream.write_all(format!("{hello}\n").as_bytes()).await.is_err() {
        return;
    }

    loop {
        match rx.recv().await {
            Ok(json) => {
                if stream.write_all(format!("{json}\n").as_bytes()).await.is_err() {
                    debug!("frontend disconnected");
                    return;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                // Client fell behind the channel buffer. Events here are rare,
                // coarse state transitions, so lagging means something is very
                // wrong with the client; drop it and let it reconnect fresh.
                warn!(missed = n, "client lagged, dropping connection");
                return;
            }
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}
