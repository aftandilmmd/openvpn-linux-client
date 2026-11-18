//! Client for the openvpn3-linux D-Bus services.
//!
//! API reference: https://github.com/OpenVPN/openvpn3-linux/tree/master/docs/dbus

use std::{collections::HashMap, path::Path, time::Duration};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use zbus::{
    fdo::PropertiesProxy,
    proxy,
    proxy::CacheProperties,
    zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value},
    Connection, MatchRule, MessageStream,
};

const SESS_SERVICE: &str = "net.openvpn.v3.sessions";

#[proxy(
    interface = "net.openvpn.v3.configuration",
    default_service = "net.openvpn.v3.configuration",
    default_path = "/net/openvpn/v3/configuration"
)]
trait ConfigManager {
    #[zbus(name = "Import")]
    fn import(
        &self,
        name: &str,
        config_str: &str,
        single_use: bool,
        persistent: bool,
    ) -> zbus::Result<OwnedObjectPath>;

    #[zbus(name = "FetchAvailableConfigs")]
    fn fetch_available_configs(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    #[zbus(property, name = "version")]
    fn version(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "net.openvpn.v3.configuration",
    default_service = "net.openvpn.v3.configuration"
)]
trait ConfigProfile {
    #[zbus(name = "Remove")]
    fn remove(&self) -> zbus::Result<()>;

    #[zbus(property, name = "name")]
    fn name(&self) -> zbus::Result<String>;

    #[zbus(property, name = "import_timestamp")]
    fn import_timestamp(&self) -> zbus::Result<u64>;

    #[zbus(property, name = "last_used_timestamp")]
    fn last_used_timestamp(&self) -> zbus::Result<u64>;
}

#[proxy(
    interface = "net.openvpn.v3.sessions",
    default_service = "net.openvpn.v3.sessions",
    default_path = "/net/openvpn/v3/sessions"
)]
trait SessionManager {
    #[zbus(name = "NewTunnel")]
    fn new_tunnel(&self, config_path: &ObjectPath<'_>) -> zbus::Result<OwnedObjectPath>;

    #[zbus(name = "FetchAvailableSessions")]
    fn fetch_available_sessions(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[proxy(
    interface = "net.openvpn.v3.sessions",
    default_service = "net.openvpn.v3.sessions"
)]
trait Session {
    #[zbus(name = "Ready")]
    fn ready(&self) -> zbus::Result<()>;
    #[zbus(name = "Connect")]
    fn connect(&self) -> zbus::Result<()>;
    #[zbus(name = "Disconnect")]
    fn disconnect(&self) -> zbus::Result<()>;
    #[zbus(name = "Pause")]
    fn pause(&self, reason: &str) -> zbus::Result<()>;
    #[zbus(name = "Resume")]
    fn resume(&self) -> zbus::Result<()>;
    #[zbus(name = "Restart")]
    fn restart(&self) -> zbus::Result<()>;
    #[zbus(name = "LogForward")]
    fn log_forward(&self, enable: bool) -> zbus::Result<()>;

    #[zbus(name = "UserInputQueueGetTypeGroup")]
    fn user_input_queue_get_type_group(&self) -> zbus::Result<Vec<(u32, u32)>>;
    #[zbus(name = "UserInputQueueCheck")]
    fn user_input_queue_check(&self, kind: u32, group: u32) -> zbus::Result<Vec<u32>>;
    #[zbus(name = "UserInputQueueFetch")]
    fn user_input_queue_fetch(
        &self,
        kind: u32,
        group: u32,
        id: u32,
    ) -> zbus::Result<(u32, u32, u32, String, String, bool)>;
    #[zbus(name = "UserInputProvide")]
    fn user_input_provide(&self, kind: u32, group: u32, id: u32, value: &str) -> zbus::Result<()>;

    #[zbus(property, name = "config_path")]
    fn config_path(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property, name = "config_name")]
    fn config_name(&self) -> zbus::Result<String>;
    #[zbus(property, name = "device_name")]
    fn device_name(&self) -> zbus::Result<String>;
    #[zbus(property, name = "session_created")]
    fn session_created(&self) -> zbus::Result<u64>;
}

#[derive(Serialize)]
pub struct Profile {
    pub path: String,
    pub name: String,
    pub imported: u64,
    pub last_used: u64,
}

#[derive(Serialize, Default)]
pub struct Status {
    pub major: u32,
    pub minor: u32,
    pub message: String,
}

#[derive(Serialize)]
pub struct Server {
    pub proto: String,
    pub ip: String,
    pub port: u32,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub path: String,
    pub config_path: String,
    pub config_name: String,
    pub status: Status,
    pub device: String,
    pub server: Option<Server>,
    pub created: u64,
    pub stats: HashMap<String, i64>,
}

#[derive(Serialize, Clone)]
pub struct InputField {
    pub kind: u32,
    pub group: u32,
    pub id: u32,
    pub name: String,
    pub description: String,
    pub hidden: bool,
}

#[derive(Deserialize)]
pub struct InputValue {
    pub kind: u32,
    pub group: u32,
    pub id: u32,
    pub value: String,
}

#[derive(Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ConnectOutcome {
    Started { session: String },
    NeedsInput { session: String, fields: Vec<InputField> },
}

#[derive(Serialize, Clone)]
pub struct SignalEvent {
    pub path: String,
    pub member: String,
    pub a: u32,
    pub b: u32,
    pub message: String,
}

/// Turns D-Bus errors into the human readable part only.
pub fn err(e: zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(_, Some(desc), _) => desc,
        other => other.to_string(),
    }
}

type R<T> = Result<T, String>;

#[derive(Clone)]
pub struct Client {
    conn: Connection,
}

impl Client {
    pub async fn system() -> zbus::Result<Self> {
        Ok(Self {
            conn: Connection::system().await?,
        })
    }

    pub async fn version(&self) -> R<String> {
        let mgr = ConfigManagerProxy::new(&self.conn).await.map_err(err)?;
        mgr.version().await.map_err(err)
    }

    async fn profile(&self, path: &str) -> R<ConfigProfileProxy<'static>> {
        let p = ObjectPath::try_from(path.to_string()).map_err(|e| e.to_string())?;
        ConfigProfileProxy::builder(&self.conn)
            .path(p)
            .map_err(err)?
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(err)
    }

    async fn session(&self, path: &str) -> R<SessionProxy<'static>> {
        let p = ObjectPath::try_from(path.to_string()).map_err(|e| e.to_string())?;
        SessionProxy::builder(&self.conn)
            .path(p)
            .map_err(err)?
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(err)
    }

    pub async fn profiles(&self) -> R<Vec<Profile>> {
        let mgr = ConfigManagerProxy::new(&self.conn).await.map_err(err)?;
        let mut out = Vec::new();
        for path in mgr.fetch_available_configs().await.map_err(err)? {
            let path = path.to_string();
            let Ok(p) = self.profile(&path).await else { continue };
            let Ok(name) = p.name().await else { continue };
            out.push(Profile {
                imported: p.import_timestamp().await.unwrap_or(0),
                last_used: p.last_used_timestamp().await.unwrap_or(0),
                path,
                name,
            });
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    /// Imports an .ovpn file as a persistent profile. Referenced key/cert
    /// files are embedded, since openvpn3 only accepts self-contained profiles.
    pub async fn import_file(&self, file: &Path, name: Option<&str>) -> R<String> {
        let text = std::fs::read_to_string(file)
            .map_err(|e| format!("Cannot read {}: {e}", file.display()))?;
        let dir = file.parent().unwrap_or(Path::new("."));
        let config = inline_files(&text, dir)?;
        let name = match name {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "VPN profile".into()),
        };
        let mgr = ConfigManagerProxy::new(&self.conn).await.map_err(err)?;
        let path = mgr.import(&name, &config, false, true).await.map_err(err)?;
        Ok(path.to_string())
    }

    pub async fn remove_profile(&self, path: &str) -> R<()> {
        self.profile(path).await?.remove().await.map_err(err)
    }

    pub async fn sessions(&self) -> R<Vec<SessionInfo>> {
        let mgr = SessionManagerProxy::new(&self.conn).await.map_err(err)?;
        let mut out = Vec::new();
        for path in mgr.fetch_available_sessions().await.map_err(err)? {
            let path = path.to_string();
            let Ok(s) = self.session(&path).await else { continue };
            let Ok(config_path) = s.config_path().await else { continue };
            let props = self.props(&path).await?;
            out.push(SessionInfo {
                config_path: config_path.to_string(),
                config_name: s.config_name().await.unwrap_or_default(),
                status: props.get("status").await.map(|v| parse_status(&v)).unwrap_or_default(),
                device: s.device_name().await.unwrap_or_default(),
                server: props.get("connected_to").await.ok().and_then(|v| parse_server(&v)),
                created: s.session_created().await.unwrap_or(0),
                stats: props
                    .get("statistics")
                    .await
                    .ok()
                    .and_then(|v| HashMap::<String, i64>::try_from(v).ok())
                    .unwrap_or_default(),
                path,
            });
        }
        Ok(out)
    }

    async fn props(&self, path: &str) -> R<SessionProps> {
        let p = ObjectPath::try_from(path.to_string()).map_err(|e| e.to_string())?;
        let proxy = PropertiesProxy::builder(&self.conn)
            .destination(SESS_SERVICE)
            .map_err(err)?
            .path(p)
            .map_err(err)?
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(err)?;
        Ok(SessionProps(proxy))
    }

    /// Creates a session for the profile and connects it, or reports the
    /// credentials the backend asks for first.
    pub async fn start(&self, config_path: &str) -> R<ConnectOutcome> {
        let cfg = ObjectPath::try_from(config_path).map_err(|e| e.to_string())?;
        let mgr = SessionManagerProxy::new(&self.conn).await.map_err(err)?;
        let session = mgr.new_tunnel(&cfg).await.map_err(err)?.to_string();
        let s = self.session(&session).await?;
        // Log forwarding is best effort; the connection works without it.
        let _ = s.log_forward(true).await;
        self.ready_then_connect(&s, &session).await
    }

    pub async fn provide(&self, session: &str, values: Vec<InputValue>) -> R<ConnectOutcome> {
        let s = self.session(session).await?;
        for v in values {
            s.user_input_provide(v.kind, v.group, v.id, &v.value)
                .await
                .map_err(err)?;
        }
        self.ready_then_connect(&s, session).await
    }

    pub async fn pending_input(&self, session: &str) -> R<Vec<InputField>> {
        let s = self.session(session).await?;
        input_queue(&s).await.map_err(err)
    }

    async fn ready_then_connect(&self, s: &SessionProxy<'_>, session: &str) -> R<ConnectOutcome> {
        // The backend process needs a moment to start; Ready() fails until then.
        let mut last = String::from("VPN backend did not become ready");
        for _ in 0..50 {
            match s.ready().await {
                Ok(()) => {
                    s.connect().await.map_err(err)?;
                    return Ok(ConnectOutcome::Started {
                        session: session.to_string(),
                    });
                }
                Err(e) => {
                    last = err(e);
                    let fields = input_queue(s).await.unwrap_or_default();
                    if !fields.is_empty() {
                        return Ok(ConnectOutcome::NeedsInput {
                            session: session.to_string(),
                            fields,
                        });
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err(last)
    }

    pub async fn disconnect(&self, session: &str) -> R<()> {
        self.session(session).await?.disconnect().await.map_err(err)
    }

    pub async fn pause(&self, session: &str) -> R<()> {
        self.session(session).await?.pause("Paused by user").await.map_err(err)
    }

    pub async fn resume(&self, session: &str) -> R<()> {
        self.session(session).await?.resume().await.map_err(err)
    }

    pub async fn restart(&self, session: &str) -> R<()> {
        self.session(session).await?.restart().await.map_err(err)
    }

    /// Forwards Log, StatusChange and AttentionRequired signals from all
    /// sessions until the bus connection closes.
    pub async fn watch_signals<F: Fn(SignalEvent)>(&self, on_event: F) -> zbus::Result<()> {
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(SESS_SERVICE)?
            .build();
        let mut stream = MessageStream::for_match_rule(rule, &self.conn, Some(256)).await?;
        while let Some(msg) = stream.next().await {
            let Ok(msg) = msg else { continue };
            let header = msg.header();
            let member = header.member().map(|m| m.to_string()).unwrap_or_default();
            if !matches!(member.as_str(), "Log" | "StatusChange" | "AttentionRequired") {
                continue;
            }
            let path = header.path().map(|p| p.to_string()).unwrap_or_default();
            if let Ok((a, b, message)) = msg.body().deserialize::<(u32, u32, String)>() {
                on_event(SignalEvent {
                    path,
                    member,
                    a,
                    b,
                    message,
                });
            }
        }
        Ok(())
    }
}

struct SessionProps(PropertiesProxy<'static>);

impl SessionProps {
    async fn get(&self, name: &str) -> zbus::Result<OwnedValue> {
        let iface = zbus::names::InterfaceName::from_static_str_unchecked(SESS_SERVICE);
        Ok(self.0.get(iface, name).await?)
    }
}

async fn input_queue(s: &SessionProxy<'_>) -> zbus::Result<Vec<InputField>> {
    let mut out = Vec::new();
    for (kind, group) in s.user_input_queue_get_type_group().await? {
        for id in s.user_input_queue_check(kind, group).await? {
            let (kind, group, id, name, description, hidden) =
                s.user_input_queue_fetch(kind, group, id).await?;
            out.push(InputField {
                kind,
                group,
                id,
                name,
                description,
                hidden,
            });
        }
    }
    Ok(out)
}

/// `status` is a `(uus)` struct: StatusMajor, StatusMinor, message.
fn parse_status(v: &Value) -> Status {
    if let Value::Structure(s) = v {
        if let [Value::U32(major), Value::U32(minor), Value::Str(msg)] = s.fields() {
            return Status {
                major: *major,
                minor: *minor,
                message: msg.to_string(),
            };
        }
    }
    Status::default()
}

/// `connected_to` is a `(ssu)` struct: proto, server ip, port.
fn parse_server(v: &Value) -> Option<Server> {
    if let Value::Structure(s) = v {
        if let [Value::Str(proto), Value::Str(ip), Value::U32(port)] = s.fields() {
            if !ip.is_empty() {
                return Some(Server {
                    proto: proto.to_string(),
                    ip: ip.to_string(),
                    port: *port,
                });
            }
        }
    }
    None
}

const FILE_DIRECTIVES: &[&str] = &[
    "ca",
    "cert",
    "key",
    "extra-certs",
    "tls-auth",
    "tls-crypt",
    "tls-crypt-v2",
    "dh",
];

/// Replaces `ca ca.crt`-style references with inline `<ca>...</ca>` blocks.
pub fn inline_files(text: &str, dir: &Path) -> R<String> {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(directive), Some(arg)) = (parts.next(), parts.next()) {
            if FILE_DIRECTIVES.contains(&directive) && arg != "[inline]" {
                let file = dir.join(arg.trim_matches('"'));
                let body = std::fs::read_to_string(&file).map_err(|e| {
                    format!("'{directive}' points to {} which cannot be read: {e}", file.display())
                })?;
                out.push_str(&format!("<{directive}>\n{}\n</{directive}>\n", body.trim_end()));
                if directive == "tls-auth" {
                    if let Some(key_dir) = parts.next() {
                        out.push_str(&format!("key-direction {key_dir}\n"));
                    }
                }
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inlines_referenced_files() {
        let dir = std::env::temp_dir().join("openvpn-client-inline-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ca.crt"), "CERTDATA\n").unwrap();
        std::fs::write(dir.join("ta.key"), "TAKEY\n").unwrap();
        let cfg = "client\nca ca.crt\ntls-auth ta.key 1\n<key>\nINLINE\n</key>\n";
        let out = inline_files(cfg, &dir).unwrap();
        assert_eq!(
            out,
            "client\n<ca>\nCERTDATA\n</ca>\n<tls-auth>\nTAKEY\n</tls-auth>\nkey-direction 1\n<key>\nINLINE\n</key>\n"
        );
    }

    #[test]
    fn missing_file_is_an_error() {
        let err = inline_files("ca nope.crt\n", Path::new("/nonexistent")).unwrap_err();
        assert!(err.contains("nope.crt"));
    }
}
