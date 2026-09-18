use crate::tray::protocol::{socket_path, TrayEvent, TrayMessage, TrayRecord};

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use zbus::{
    blocking::{Connection, Proxy},
    interface,
};

const SNI_PATH: &str = "/StatusNotifierItem";
const WATCHER_SERVICE: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const WATCHER_INTERFACE: &str = "org.kde.StatusNotifierWatcher";

#[derive(Debug, Clone)]
enum StatusNotifierAction {
    RootToggle,
    ModuleOpen { tray_id: String },
}

struct StatusNotifierItem {
    id: String,
    title: String,
    icon: String,
    action: StatusNotifierAction,
}

impl StatusNotifierItem {
    fn root(icon: String) -> Self {
        Self {
            id: "neebles".to_string(),
            title: "N.E.E.B.L.E.S.".to_string(),
            icon,
            action: StatusNotifierAction::RootToggle,
        }
    }

    fn module(tray: &TrayRecord) -> Self {
        Self {
            id: tray.tray_id.clone(),
            title: tray.owner_module.clone(),
            icon: tray.icon.clone(),
            action: StatusNotifierAction::ModuleOpen {
                tray_id: tray.tray_id.clone(),
            },
        }
    }

    fn activate_item(&self) {
        let message = match &self.action {
            StatusNotifierAction::RootToggle => TrayMessage::HostToggle,

            StatusNotifierAction::ModuleOpen { tray_id } => TrayMessage::Open {
                tray_id: tray_id.clone(),
            },
        };

        let _ = send_manager_command(&message);
    }
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        &self.id
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        &self.title
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        &self.icon
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    fn activate(&self, _x: i32, _y: i32) {
        self.activate_item();
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {
        self.activate_item();
    }

    fn context_menu(&self, _x: i32, _y: i32) {
        self.activate_item();
    }

    fn scroll(&self, _delta: i32, _orientation: &str) {}
}

struct HostedItem {
    _connection: Connection,
}

fn service_name(tray_id: &str) -> String {
    let encoded = tray_id
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    format!("org.neebles.StatusNotifierItem.t{}", encoded)
}

fn send_manager_command(message: &TrayMessage) -> Result<TrayMessage, String> {
    crate::tray::client::request(message)
}

fn register_with_watcher(connection: &Connection, service: &str) -> Result<(), String> {
    let proxy = Proxy::new(connection, WATCHER_SERVICE, WATCHER_PATH, WATCHER_INTERFACE)
        .map_err(|error| format!("could not connect to StatusNotifierWatcher: {error}"))?;

    proxy
        .call_method("RegisterStatusNotifierItem", &(service,))
        .map_err(|error| {
            format!(
                "could not register StatusNotifierItem '{}': {error}",
                service
            )
        })?;

    Ok(())
}

fn create_root_item() -> Result<HostedItem, String> {
    const ROOT_ID: &str = "neebles";
    const ROOT_ICON: &str = "neebles-boss-tray-icon";

    let service = service_name(ROOT_ID);

    let connection = Connection::session()
        .map_err(|error| format!("could not connect root tray to session D-Bus: {error}"))?;

    connection.request_name(service.as_str()).map_err(|error| {
        format!(
            "could not acquire D-Bus service '{}' for root tray: {error}",
            service
        )
    })?;

    connection
        .object_server()
        .at(SNI_PATH, StatusNotifierItem::root(ROOT_ICON.to_string()))
        .map_err(|error| format!("could not export root StatusNotifierItem: {error}"))?;

    register_with_watcher(&connection, &service)?;

    println!(
        "N.E.E.B.L.E.S. Tray Host registered root tray as {}",
        service
    );

    Ok(HostedItem {
        _connection: connection,
    })
}

fn create_item(tray: &TrayRecord) -> Result<HostedItem, String> {
    let service = service_name(&tray.tray_id);

    let connection = Connection::session().map_err(|error| {
        format!(
            "could not connect tray '{}' to session D-Bus: {error}",
            tray.tray_id
        )
    })?;

    connection.request_name(service.as_str()).map_err(|error| {
        format!(
            "could not acquire D-Bus service '{}' for tray '{}': {error}",
            service, tray.tray_id
        )
    })?;

    connection
        .object_server()
        .at(SNI_PATH, StatusNotifierItem::module(tray))
        .map_err(|error| {
            format!(
                "could not export StatusNotifierItem for tray '{}': {error}",
                tray.tray_id
            )
        })?;

    register_with_watcher(&connection, &service)?;

    println!(
        "N.E.E.B.L.E.S. Tray Host registered '{}' as {}",
        tray.tray_id, service
    );

    Ok(HostedItem {
        _connection: connection,
    })
}

fn reconcile_item(
    items: &mut BTreeMap<String, HostedItem>,
    tray: TrayRecord,
) -> Result<(), String> {
    if !tray.visible {
        items.remove(&tray.tray_id);
        return Ok(());
    }

    /*
     * Recreate on update.
     *
     * Tray metadata is tiny and updates are uncommon.
     * Recreating avoids keeping a second mutable state layer
     * inside the visual host.
     */
    items.remove(&tray.tray_id);

    let item = create_item(&tray)?;

    items.insert(tray.tray_id.clone(), item);

    Ok(())
}

fn reconcile_root_item(root_item: &mut Option<HostedItem>, visible: bool) -> Result<(), String> {
    if visible {
        if root_item.is_none() {
            *root_item = Some(create_root_item()?);
        }

        return Ok(());
    }

    root_item.take();

    Ok(())
}

fn process_message(
    message: TrayMessage,
    root_item: &mut Option<HostedItem>,
    items: &mut BTreeMap<String, HostedItem>,
) -> Result<(), String> {
    match message {
        TrayMessage::Snapshot {
            trays,
            root_visible,
        } => {
            reconcile_root_item(root_item, root_visible)?;

            items.clear();

            for tray in trays {
                reconcile_item(items, tray)?;
            }

            Ok(())
        }

        TrayMessage::RootVisibility { visible } => reconcile_root_item(root_item, visible),

        TrayMessage::Event { event } => match event {
            TrayEvent::Registered { tray } | TrayEvent::Updated { tray } => {
                reconcile_item(items, tray)
            }

            TrayEvent::Unregistered { tray_id } => {
                items.remove(&tray_id);
                Ok(())
            }
        },

        TrayMessage::Error { message } => Err(message),

        _ => Ok(()),
    }
}

pub fn run() -> Result<(), String> {
    let path = socket_path();

    let mut stream = UnixStream::connect(&path)
        .map_err(|error| format!("could not connect Tray Host to {}: {error}", path.display()))?;

    let mut payload = serde_json::to_vec(&TrayMessage::Subscribe)
        .map_err(|error| format!("could not serialize Tray Host subscription: {error}"))?;

    payload.push(b'\n');

    stream
        .write_all(&payload)
        .map_err(|error| format!("could not send Tray Host subscription: {error}"))?;

    stream
        .flush()
        .map_err(|error| format!("could not flush Tray Host subscription: {error}"))?;

    let mut reader = BufReader::new(stream);

    /*
     * Root SNI lifetime is visual state, not process lifetime.
     *
     * The subscription snapshot decides whether the root item
     * must exist. Module items remain independent.
     */
    let mut root_item: Option<HostedItem> = None;

    let mut items = BTreeMap::<String, HostedItem>::new();

    println!("N.E.E.B.L.E.S. Tray Host subscribed to {}", path.display());

    loop {
        let mut line = String::new();

        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("Tray Host lost manager connection: {error}"))?;

        if read == 0 {
            return Err("Tray Manager closed the Tray Host subscription".to_string());
        }

        if line.trim().is_empty() {
            continue;
        }

        let message: TrayMessage = serde_json::from_str(line.trim())
            .map_err(|error| format!("invalid Tray Host message: {error}"))?;

        process_message(message, &mut root_item, &mut items)?;
    }
}
