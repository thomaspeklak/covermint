use std::{env, fs, os::unix::net::UnixDatagram, path::PathBuf, sync::mpsc, thread};

const CONTROL_SOCKET_BASENAME: &str = "covermint-control";

#[derive(Clone, Copy, Debug)]
pub(crate) enum ControlCommand {
    On,
    Off,
    Toggle,
}

impl ControlCommand {
    fn as_wire(self) -> &'static str {
        match self {
            Self::On => "lyrics:on",
            Self::Off => "lyrics:off",
            Self::Toggle => "lyrics:toggle",
        }
    }

    fn parse_wire(value: &str) -> Option<Self> {
        match value.trim() {
            "lyrics:on" => Some(Self::On),
            "lyrics:off" => Some(Self::Off),
            "lyrics:toggle" => Some(Self::Toggle),
            _ => None,
        }
    }
}

pub(crate) fn send_command(command: ControlCommand) -> Result<(), String> {
    let path = control_socket_path();
    let socket = UnixDatagram::unbound().map_err(|error| error.to_string())?;

    socket
        .send_to(command.as_wire().as_bytes(), &path)
        .map_err(|error| {
            format!(
                "failed to reach running covermint instance at {}: {error}",
                path.display()
            )
        })?;

    Ok(())
}

pub(crate) fn start_listener(command_tx: mpsc::Sender<ControlCommand>) {
    let path = control_socket_path();
    let thread_path = path.clone();

    let _ = thread::Builder::new()
        .name("covermint-control-listener".to_string())
        .spawn(move || {
            if thread_path.exists() {
                let _ = fs::remove_file(&thread_path);
            }

            let Some(parent) = thread_path.parent() else {
                eprintln!(
                    "covermint: invalid control socket path {}",
                    thread_path.display()
                );
                return;
            };

            if let Err(error) = fs::create_dir_all(parent) {
                eprintln!(
                    "covermint: failed to create control socket dir {}: {error}",
                    parent.display()
                );
                return;
            }

            let socket = match UnixDatagram::bind(&thread_path) {
                Ok(socket) => socket,
                Err(error) => {
                    eprintln!(
                        "covermint: failed to bind control socket {}: {error}",
                        thread_path.display()
                    );
                    return;
                }
            };

            let mut buffer = [0_u8; 128];
            loop {
                match socket.recv(&mut buffer) {
                    Ok(length) => {
                        if let Ok(message) = std::str::from_utf8(&buffer[..length])
                            && let Some(command) = ControlCommand::parse_wire(message)
                        {
                            let _ = command_tx.send(command);
                        }
                    }
                    Err(error) => {
                        eprintln!("covermint: control socket receive error: {error}");
                    }
                }
            }
        });
}

fn control_socket_path() -> PathBuf {
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    let scope = env::var("UID")
        .or_else(|_| env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());

    runtime_dir.join(format!("{CONTROL_SOCKET_BASENAME}-{scope}.sock"))
}
