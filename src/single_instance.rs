use std::io::{ErrorKind, Write};
use std::net::{TcpListener, TcpStream};

/// TCP port that the primary instance listens on for activation requests.
///
/// Send a simple activation payload to whatever is listening on
/// `ACTIVATION_PORT`.
///
/// This is a best-effort operation; we ignore any errors because the most we
/// can do in this context is let the other instance potentially raise itself.
pub fn notify_existing_instance(listening_port: u16) {
    if let Ok(mut s) = TcpStream::connect(("127.0.0.1", listening_port)) {
        let _ = s.write_all(b"activate");
    }
}

/// Attempt to become the primary instance.
///
/// On success returns `Ok(Some(listener))` where `listener` is a bound
/// `TcpListener` that the caller should keep alive for the lifetime of the
/// application; it will be used later to receive activation messages.  If the
/// port could not be bound for non-fatal reasons, `Ok(None)` is returned and
/// activation support is simply disabled.
///
/// If another instance is detected via the port, this function sends the
/// activation message and returns `Err` so that the caller can exit quietly.
pub fn acquire(listening_port: u16) -> std::io::Result<Option<TcpListener>> {
    // try binding the activation port first.  failure with AddrInUse means a
    // running instance already has it.
    let listener_opt = match TcpListener::bind(("127.0.0.1", listening_port)) {
        Ok(l) => Some(l),
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            notify_existing_instance(listening_port);
            std::thread::sleep(std::time::Duration::from_millis(300)); // give the other instance a moment to wake up and raise itself
            return Err(std::io::Error::other("another instance is running"));
        }
        Err(_e) => {
            // non‑fatal failure, just continue without activation support
            None
        }
    };

    Ok(listener_opt)
}

/// Start a background thread that listens for activation messages on the
/// given `listener` and forwards a simple notification on a channel.
///
/// The returned `Receiver` is intended to be polled from the UI thread (for
/// example via a timer) so that `Frame` objects are not moved between threads.
///
/// This helper encapsulates the original loop that was previously located in
/// `main.rs`.
pub fn spawn_activation_listener(listener: TcpListener) -> std::sync::mpsc::Receiver<()> {
    let (act_tx, act_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut buf = [0u8; 16];
            use std::io::Read;
            if let Ok(n) = s.read(&mut buf)
                && buf[..n].starts_with(b"activate")
            {
                let _ = act_tx.send(());
            }
        }
    });
    act_rx
}
