use crate::tray::ipc::socket_path;
use crate::tray::protocol::{TrayEvent, TrayMessage, TrayRecord};

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

#[derive(Debug, Default)]
pub struct TrayHostModel {
    trays: BTreeMap<String, TrayRecord>,
}

impl TrayHostModel {
    pub fn apply_snapshot(&mut self, trays: Vec<TrayRecord>) {
        self.trays.clear();

        for tray in trays {
            self.trays.insert(tray.tray_id.clone(), tray);
        }
    }

    pub fn apply_event(&mut self, event: TrayEvent) {
        match event {
            TrayEvent::Registered { tray } | TrayEvent::Updated { tray } => {
                self.trays.insert(tray.tray_id.clone(), tray);
            }

            TrayEvent::Unregistered { tray_id } => {
                self.trays.remove(&tray_id);
            }
        }
    }

    pub fn trays(&self) -> Vec<&TrayRecord> {
        self.trays.values().collect()
    }

    pub fn visible_trays(&self) -> Vec<&TrayRecord> {
        self.trays.values().filter(|tray| tray.visible).collect()
    }
}

fn send_subscribe(stream: &mut UnixStream) -> Result<(), String> {
    let mut payload = serde_json::to_vec(&TrayMessage::Subscribe)
        .map_err(|error| format!("could not serialize tray host subscription: {error}"))?;

    payload.push(b'\n');

    stream
        .write_all(&payload)
        .map_err(|error| format!("could not write tray host subscription: {error}"))?;

    stream
        .flush()
        .map_err(|error| format!("could not flush tray host subscription: {error}"))
}

fn print_model(model: &TrayHostModel) {
    let total = model.trays().len();

    let visible = model.visible_trays().len();

    println!(
        "N.E.E.B.L.E.S. Tray Host: {} registered, {} visible",
        total, visible
    );
}

pub fn run() -> Result<(), String> {
    let path = socket_path();

    let mut stream = UnixStream::connect(&path).map_err(|error| {
        format!(
            "could not connect N.E.E.B.L.E.S. Tray Host to {}: {error}",
            path.display()
        )
    })?;

    send_subscribe(&mut stream)?;

    let mut reader = BufReader::new(stream);

    let mut model = TrayHostModel::default();

    println!("N.E.E.B.L.E.S. Tray Host connected to {}", path.display());

    loop {
        let mut line = String::new();

        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("could not read tray host event: {error}"))?;

        if read == 0 {
            return Err(
                "N.E.E.B.L.E.S. tray socket closed while Tray Host was connected".to_string(),
            );
        }

        if line.trim().is_empty() {
            continue;
        }

        let message: TrayMessage = serde_json::from_str(line.trim())
            .map_err(|error| format!("invalid tray host message: {error}"))?;

        match message {
            TrayMessage::Snapshot { trays } => {
                model.apply_snapshot(trays);

                print_model(&model);
            }

            TrayMessage::Event { event } => {
                model.apply_event(event);

                print_model(&model);
            }

            TrayMessage::Error { message } => {
                return Err(format!("Tray Manager rejected Tray Host: {message}"));
            }

            other => {
                return Err(format!(
                    "unexpected message received by Tray Host: {:?}",
                    other
                ));
            }
        }
    }
}
