use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalEnvelope {
    #[serde(rename = "type")]
    pub envelope_type: String,
    pub endpoint: String,
    pub activate: bool,
    pub package: Map<String, Value>,
    pub message: Map<String, Value>,
}

pub fn error_message(code: impl Into<String>, message: impl Into<String>) -> Map<String, Value> {
    Map::from_iter([
        ("code".to_string(), Value::String(code.into())),
        ("message".to_string(), Value::String(message.into())),
    ])
}

pub fn incompatibility_message(
    component: impl Into<String>,
    current: impl Into<String>,
    required: impl Into<String>,
    message: impl Into<String>,
) -> Map<String, Value> {
    Map::from_iter([
        ("component".to_string(), Value::String(component.into())),
        ("current".to_string(), Value::String(current.into())),
        ("required".to_string(), Value::String(required.into())),
        ("message".to_string(), Value::String(message.into())),
    ])
}

pub fn stage0_message(state: impl Into<String>, message: impl Into<String>) -> Map<String, Value> {
    Map::from_iter([
        ("state".to_string(), Value::String(state.into())),
        ("message".to_string(), Value::String(message.into())),
    ])
}

pub fn build_external_envelope<PackageBuilder, MessageBuilder>(
    envelope_type: impl Into<String>,
    endpoint: impl Into<String>,
    activate: bool,
    package_builder: PackageBuilder,
    message_builder: MessageBuilder,
) -> Option<ExternalEnvelope>
where
    PackageBuilder: FnOnce() -> Map<String, Value>,
    MessageBuilder: FnOnce() -> Map<String, Value>,
{
    if !activate {
        return None;
    }

    Some(ExternalEnvelope {
        envelope_type: envelope_type.into(),
        endpoint: endpoint.into(),
        activate,
        package: package_builder(),
        message: message_builder(),
    })
}

const SYSTEMD_LISTEN_FD: libc::c_int = 3;
const MAX_EXTERNAL_PACKET_BYTES: usize = 256 * 1024;
const DEFAULT_EXTERNAL_SOCKET_PATH: &str = "/run/neebles/external.sock";

pub fn socket_path() -> std::path::PathBuf {
    std::env::var("NEEBLES_EXTERNAL_SOCKET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(DEFAULT_EXTERNAL_SOCKET_PATH))
}

pub fn send(envelope: &ExternalEnvelope) -> Result<(), String> {
    let payload = serde_json::to_vec(envelope)
        .map_err(|error| format!("could not serialize ExternalEnvelope: {error}"))?;

    if payload.len() > MAX_EXTERNAL_PACKET_BYTES {
        return Err(format!(
            "ExternalEnvelope is too large: {} bytes, maximum is {}",
            payload.len(),
            MAX_EXTERNAL_PACKET_BYTES
        ));
    }

    send_packet(&socket_path(), &payload)
}

fn send_packet(path: &std::path::Path, payload: &[u8]) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;

    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };

    if fd < 0 {
        return Err(format!(
            "could not create external SOCK_SEQPACKET client: {}",
            std::io::Error::last_os_error()
        ));
    }

    let result = (|| {
        let raw_path = path.as_os_str().as_bytes();

        if raw_path.len() >= 108 {
            return Err(format!(
                "external socket path is too long: {}",
                path.display()
            ));
        }

        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;

        for (index, byte) in raw_path.iter().enumerate() {
            address.sun_path[index] = *byte as libc::c_char;
        }

        let address_len =
            (std::mem::size_of::<libc::sa_family_t>() + raw_path.len() + 1) as libc::socklen_t;

        let connected = unsafe {
            libc::connect(
                fd,
                &address as *const _ as *const libc::sockaddr,
                address_len,
            )
        };

        if connected != 0 {
            return Err(format!(
                "could not connect to external socket {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }

        send_packet_fd(fd, payload)
    })();

    unsafe {
        libc::close(fd);
    }

    result
}

fn send_packet_fd(fd: libc::c_int, payload: &[u8]) -> Result<(), String> {
    let sent = unsafe {
        libc::send(
            fd,
            payload.as_ptr() as *const libc::c_void,
            payload.len(),
            libc::MSG_NOSIGNAL,
        )
    };

    if sent < 0 {
        return Err(format!(
            "could not send ExternalEnvelope: {}",
            std::io::Error::last_os_error()
        ));
    }

    if sent as usize != payload.len() {
        return Err(format!(
            "external socket sent {} of {} bytes",
            sent,
            payload.len()
        ));
    }

    Ok(())
}

pub fn serve() -> Result<(), String> {
    let listener = systemd_listener_fd()?;

    loop {
        let client = unsafe {
            libc::accept4(
                listener,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                libc::SOCK_CLOEXEC,
            )
        };

        if client < 0 {
            let error = std::io::Error::last_os_error();

            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }

            return Err(format!(
                "could not accept external.sock connection: {error}"
            ));
        }

        std::thread::spawn(move || {
            if let Err(error) = handle_external_client(client) {
                eprintln!("N.E.E.B.L.E.S.: external socket client error: {error}");
            }

            unsafe {
                libc::close(client);
            }
        });
    }
}

fn systemd_listener_fd() -> Result<libc::c_int, String> {
    let listen_pid = std::env::var("LISTEN_PID")
        .map_err(|_| "external receiver was not socket-activated by systemd".to_string())?
        .parse::<u32>()
        .map_err(|error| format!("invalid LISTEN_PID: {error}"))?;

    if listen_pid != std::process::id() {
        return Err(format!(
            "LISTEN_PID {listen_pid} does not match external receiver pid {}",
            std::process::id()
        ));
    }

    let listen_fds = std::env::var("LISTEN_FDS")
        .map_err(|_| "external receiver received no systemd socket".to_string())?
        .parse::<i32>()
        .map_err(|error| format!("invalid LISTEN_FDS: {error}"))?;

    if listen_fds != 1 {
        return Err(format!(
            "external receiver expected exactly one systemd socket, received {listen_fds}"
        ));
    }

    let mut socket_type: libc::c_int = 0;
    let mut socket_type_len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;

    let result = unsafe {
        libc::getsockopt(
            SYSTEMD_LISTEN_FD,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            &mut socket_type as *mut _ as *mut libc::c_void,
            &mut socket_type_len,
        )
    };

    if result != 0 {
        return Err(format!(
            "could not inspect systemd external socket: {}",
            std::io::Error::last_os_error()
        ));
    }

    if socket_type != libc::SOCK_SEQPACKET {
        return Err(format!(
            "external receiver expected SOCK_SEQPACKET, received socket type {socket_type}"
        ));
    }

    let mut socket_domain: libc::c_int = 0;
    let mut socket_domain_len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;

    let result = unsafe {
        libc::getsockopt(
            SYSTEMD_LISTEN_FD,
            libc::SOL_SOCKET,
            libc::SO_DOMAIN,
            &mut socket_domain as *mut _ as *mut libc::c_void,
            &mut socket_domain_len,
        )
    };

    if result != 0 {
        return Err(format!(
            "could not inspect systemd external socket domain: {}",
            std::io::Error::last_os_error()
        ));
    }

    if socket_domain != libc::AF_UNIX {
        return Err(format!(
            "external receiver expected AF_UNIX, received socket domain {socket_domain}"
        ));
    }

    let mut accepting: libc::c_int = 0;
    let mut accepting_len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;

    let result = unsafe {
        libc::getsockopt(
            SYSTEMD_LISTEN_FD,
            libc::SOL_SOCKET,
            libc::SO_ACCEPTCONN,
            &mut accepting as *mut _ as *mut libc::c_void,
            &mut accepting_len,
        )
    };

    if result != 0 {
        return Err(format!(
            "could not inspect systemd external socket listener state: {}",
            std::io::Error::last_os_error()
        ));
    }

    if accepting != 1 {
        return Err("external receiver expected a listening systemd socket".to_string());
    }

    Ok(SYSTEMD_LISTEN_FD)
}

fn handle_external_client(client: libc::c_int) -> Result<(), String> {
    authorize_external_peer(client)?;
    handle_external_client_with_limit(client, MAX_EXTERNAL_PACKET_BYTES)
}

fn authorize_external_peer(client: libc::c_int) -> Result<(), String> {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut credentials_len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

    let result = unsafe {
        libc::getsockopt(
            client,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut credentials as *mut _ as *mut libc::c_void,
            &mut credentials_len,
        )
    };

    if result != 0 {
        return Err(format!(
            "could not inspect external peer credentials: {}",
            std::io::Error::last_os_error()
        ));
    }

    let (desktop_uid, _) = crate::runtime_identity::desktop_identity()?;

    authorize_external_uid(credentials.uid, desktop_uid)
}

fn authorize_external_uid(peer_uid: libc::uid_t, desktop_uid: libc::uid_t) -> Result<(), String> {
    if peer_uid == 0 || peer_uid == desktop_uid {
        return Ok(());
    }

    Err(format!(
        "external socket rejected unauthorized peer uid {peer_uid}"
    ))
}

fn handle_external_client_with_limit(
    client: libc::c_int,
    max_packet_bytes: usize,
) -> Result<(), String> {
    loop {
        let packet_size = peek_packet_size(client)?;

        if packet_size == 0 {
            return Ok(());
        }

        if packet_size > max_packet_bytes {
            discard_packet(client)?;

            return Err(format!(
                "ExternalEnvelope packet is too large: {packet_size} bytes, maximum is {max_packet_bytes}"
            ));
        }

        let mut buffer = vec![0_u8; packet_size];

        let received = loop {
            let received = unsafe {
                libc::recv(
                    client,
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    buffer.len(),
                    0,
                )
            };

            if received >= 0 {
                break received;
            }

            let error = std::io::Error::last_os_error();

            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }

            return Err(format!("could not receive external envelope: {error}"));
        };

        if received == 0 {
            return Ok(());
        }

        if received as usize != packet_size {
            return Err(format!(
                "external socket received {received} of {packet_size} expected bytes"
            ));
        }

        let envelope: ExternalEnvelope = serde_json::from_slice(&buffer)
            .map_err(|error| format!("invalid ExternalEnvelope: {error}"))?;

        consume_external_envelope(envelope)?;
    }
}

fn peek_packet_size(client: libc::c_int) -> Result<usize, String> {
    let mut probe = [0_u8; 1];

    loop {
        let received = unsafe {
            libc::recv(
                client,
                probe.as_mut_ptr() as *mut libc::c_void,
                probe.len(),
                libc::MSG_PEEK | libc::MSG_TRUNC,
            )
        };

        if received >= 0 {
            return Ok(received as usize);
        }

        let error = std::io::Error::last_os_error();

        if error.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }

        return Err(format!(
            "could not inspect external envelope packet size: {error}"
        ));
    }
}

fn discard_packet(client: libc::c_int) -> Result<(), String> {
    let mut probe = [0_u8; 1];

    loop {
        let received = unsafe {
            libc::recv(
                client,
                probe.as_mut_ptr() as *mut libc::c_void,
                probe.len(),
                libc::MSG_TRUNC,
            )
        };

        if received >= 0 {
            return Ok(());
        }

        let error = std::io::Error::last_os_error();

        if error.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }

        return Err(format!(
            "could not discard oversized external envelope packet: {error}"
        ));
    }
}

fn consume_external_envelope(envelope: ExternalEnvelope) -> Result<(), String> {
    /*
     * activate=false is a hard guard on both sides of the boundary.
     *
     * Producers should never send a disabled envelope, but the receiver
     * independently refuses to process one if a malformed or nonconforming
     * local client sends it anyway.
     */
    if !envelope.activate {
        return Ok(());
    }

    /*
     * Transport and remote dispatch are intentionally not implemented here.
     *
     * external.sock only establishes the local outbound boundary.
     * Stage0 must never wait for internet or remote delivery.
     */
    let _ = envelope;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;

    fn object(value: Value) -> Map<String, Value> {
        value
            .as_object()
            .expect("test value must be a JSON object")
            .clone()
    }

    #[test]
    fn receiver_ignores_disabled_external_envelope() {
        let envelope = ExternalEnvelope {
            envelope_type: "error".to_string(),
            endpoint: String::new(),
            activate: false,
            package: Map::new(),
            message: error_message("test", "disabled envelope"),
        };

        consume_external_envelope(envelope).expect("disabled envelope must be ignored safely");
    }

    #[test]
    fn external_peer_policy_allows_root() {
        authorize_external_uid(0, 1000)
            .expect("root must be allowed to produce external envelopes");
    }

    #[test]
    fn external_peer_policy_allows_desktop_uid() {
        authorize_external_uid(1000, 1000)
            .expect("desktop uid must be allowed to produce external envelopes");
    }

    #[test]
    fn external_peer_policy_rejects_unrelated_uid() {
        let error = authorize_external_uid(2000, 1000).expect_err("unrelated uid must be rejected");

        assert!(error.contains("unauthorized peer uid 2000"));
    }

    #[test]
    fn receiver_rejects_packet_larger_than_configured_limit() {
        const TEST_LIMIT: usize = 4 * 1024;

        let mut sockets = [-1; 2];

        let result = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                sockets.as_mut_ptr(),
            )
        };

        assert_eq!(result, 0);

        let payload = vec![b'x'; TEST_LIMIT + 1];

        send_packet_fd(sockets[0], &payload).expect("test packet should reach receiver");

        let error = handle_external_client_with_limit(sockets[1], TEST_LIMIT)
            .expect_err("receiver must reject oversized packet");

        unsafe {
            libc::close(sockets[0]);
            libc::close(sockets[1]);
        }

        assert!(error.contains("packet is too large"));
        assert!(error.contains(&(TEST_LIMIT + 1).to_string()));
        assert!(error.contains(&TEST_LIMIT.to_string()));
    }

    #[test]
    fn seqpacket_preserves_one_envelope_as_one_packet() {
        let mut sockets = [-1; 2];

        let result = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                sockets.as_mut_ptr(),
            )
        };

        assert_eq!(result, 0);

        let envelope = build_external_envelope("stage0", "", true, Map::new, || {
            stage0_message("ready", "Stage0 completed")
        })
        .expect("external envelope should be built");

        let payload = serde_json::to_vec(&envelope).expect("external envelope should serialize");

        send_packet_fd(sockets[0], &payload).expect("external envelope should send as one packet");

        let mut buffer = vec![0_u8; MAX_EXTERNAL_PACKET_BYTES];

        let received = unsafe {
            libc::recv(
                sockets[1],
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
                0,
            )
        };

        unsafe {
            libc::close(sockets[0]);
            libc::close(sockets[1]);
        }

        assert!(received > 0);
        assert_eq!(received as usize, payload.len());
        assert_eq!(&buffer[..received as usize], payload.as_slice());

        let decoded: ExternalEnvelope = serde_json::from_slice(&buffer[..received as usize])
            .expect("received packet should decode");

        assert_eq!(decoded.envelope_type, "stage0");
        assert_eq!(decoded.message["state"], "ready");
        assert_eq!(decoded.message["message"], "Stage0 completed");
    }

    #[test]
    fn seqpacket_preserves_multiple_independent_envelopes() {
        let mut sockets = [-1; 2];

        let result = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                sockets.as_mut_ptr(),
            )
        };

        assert_eq!(result, 0);

        let envelopes = vec![
            build_external_envelope("stage0", "", true, Map::new, || {
                stage0_message("ready", "Stage0 completed")
            })
            .expect("stage0 envelope should be built"),
            build_external_envelope("error", "", true, Map::new, || {
                error_message("installer_failed", "Installer recipe failed")
            })
            .expect("error envelope should be built"),
            build_external_envelope("incompatibility", "", true, Map::new, || {
                incompatibility_message(
                    "libqt6quick6",
                    "6.4",
                    ">=6.5,<7.0",
                    "Installed version is outside the supported range",
                )
            })
            .expect("incompatibility envelope should be built"),
        ];

        for envelope in &envelopes {
            let payload = serde_json::to_vec(envelope).expect("external envelope should serialize");

            send_packet_fd(sockets[0], &payload)
                .expect("external envelope should send as one packet");
        }

        let mut received_types = Vec::new();

        for _ in 0..envelopes.len() {
            let mut buffer = vec![0_u8; MAX_EXTERNAL_PACKET_BYTES];

            let received = unsafe {
                libc::recv(
                    sockets[1],
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    buffer.len(),
                    0,
                )
            };

            assert!(received > 0);

            let decoded: ExternalEnvelope = serde_json::from_slice(&buffer[..received as usize])
                .expect("received packet should decode");

            received_types.push(decoded.envelope_type);
        }

        unsafe {
            libc::close(sockets[0]);
            libc::close(sockets[1]);
        }

        assert_eq!(received_types, vec!["stage0", "error", "incompatibility"]);
    }

    #[test]
    fn disabled_external_returns_before_building_payload() {
        let package_called = Cell::new(false);
        let message_called = Cell::new(false);

        let envelope = build_external_envelope(
            "stage0",
            "https://example.invalid/report",
            false,
            || {
                package_called.set(true);
                object(json!({"package": true}))
            },
            || {
                message_called.set(true);
                object(json!({"message": true}))
            },
        );

        assert!(envelope.is_none());
        assert!(!package_called.get());
        assert!(!message_called.get());
    }

    #[test]
    fn builds_error_message() {
        assert_eq!(
            error_message("installer_failed", "Installer recipe failed"),
            object(json!({
                "code": "installer_failed",
                "message": "Installer recipe failed"
            }))
        );
    }

    #[test]
    fn builds_incompatibility_message() {
        assert_eq!(
            incompatibility_message(
                "libqt6quick6",
                "6.4",
                ">=6.5,<7.0",
                "Installed version is outside the supported range",
            ),
            object(json!({
                "component": "libqt6quick6",
                "current": "6.4",
                "required": ">=6.5,<7.0",
                "message": "Installed version is outside the supported range"
            }))
        );
    }

    #[test]
    fn builds_stage0_message() {
        assert_eq!(
            stage0_message("failed", "Stage0 could not restore local integrity"),
            object(json!({
                "state": "failed",
                "message": "Stage0 could not restore local integrity"
            }))
        );
    }

    #[test]
    fn enabled_external_builds_generic_envelope() {
        let envelope = build_external_envelope("stage0", "", true, Map::new, || {
            stage0_message("failed", "example")
        })
        .expect("external envelope should be built");

        assert_eq!(envelope.envelope_type, "stage0");
        assert_eq!(envelope.endpoint, "");
        assert!(envelope.activate);
        assert!(envelope.package.is_empty());
        assert_eq!(envelope.message, stage0_message("failed", "example"));

        let serialized =
            serde_json::to_value(envelope).expect("external envelope should serialize");

        assert_eq!(serialized["type"], "stage0");
        assert_eq!(serialized["endpoint"], "");
        assert_eq!(serialized["activate"], true);
        assert_eq!(serialized["package"], json!({}));
        assert_eq!(
            serialized["message"],
            json!({"state": "failed", "message": "example"})
        );
    }
}
