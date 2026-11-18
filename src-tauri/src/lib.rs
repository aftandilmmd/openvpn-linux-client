pub mod ovpn3;

use std::{path::PathBuf, sync::Mutex};

use ovpn3::{Client, ConnectOutcome, InputField, InputValue, Profile, SessionInfo};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

pub const REPO_URL: &str = "https://github.com/aftandilmmd/openvpn-linux-client";

/// The system bus connection, or why it could not be opened.
struct Backend(Result<Client, String>);

impl Backend {
    fn get(&self) -> Result<&Client, String> {
        self.0.as_ref().map_err(|e| e.clone())
    }
}

type Res<T> = Result<T, String>;

#[tauri::command]
async fn backend_version(b: State<'_, Backend>) -> Res<String> {
    b.get()?.version().await
}

#[tauri::command]
async fn list_profiles(b: State<'_, Backend>) -> Res<Vec<Profile>> {
    b.get()?.profiles().await
}

#[tauri::command]
async fn list_sessions(b: State<'_, Backend>) -> Res<Vec<SessionInfo>> {
    b.get()?.sessions().await
}

/// Shows a file picker and imports the chosen profile. `None` if cancelled.
#[tauri::command]
async fn pick_and_import(app: AppHandle, b: State<'_, Backend>) -> Res<Option<String>> {
    let Some(picked) = app
        .dialog()
        .file()
        .set_title("Import OpenVPN profile")
        .add_filter("OpenVPN profile", &["ovpn", "conf"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    b.get()?.import_file(&path, None).await.map(Some)
}

#[tauri::command]
async fn import_profile(path: String, name: Option<String>, b: State<'_, Backend>) -> Res<String> {
    b.get()?.import_file(&PathBuf::from(path), name.as_deref()).await
}

#[tauri::command]
async fn remove_profile(path: String, b: State<'_, Backend>) -> Res<()> {
    b.get()?.remove_profile(&path).await
}

#[tauri::command]
async fn connect(config_path: String, b: State<'_, Backend>) -> Res<ConnectOutcome> {
    b.get()?.start(&config_path).await
}

#[tauri::command]
async fn submit_input(session: String, values: Vec<InputValue>, b: State<'_, Backend>) -> Res<ConnectOutcome> {
    b.get()?.provide(&session, values).await
}

#[tauri::command]
async fn pending_input(session: String, b: State<'_, Backend>) -> Res<Vec<InputField>> {
    b.get()?.pending_input(&session).await
}

#[tauri::command]
async fn disconnect(session: String, b: State<'_, Backend>) -> Res<()> {
    b.get()?.disconnect(&session).await
}

#[tauri::command]
async fn pause(session: String, b: State<'_, Backend>) -> Res<()> {
    b.get()?.pause(&session).await
}

#[tauri::command]
async fn resume(session: String, b: State<'_, Backend>) -> Res<()> {
    b.get()?.resume(&session).await
}

#[tauri::command]
async fn restart(session: String, b: State<'_, Backend>) -> Res<()> {
    b.get()?.restart(&session).await
}

/// Opens web-based authentication pages and the project page in the browser.
#[tauri::command]
fn open_url(url: String, app: AppHandle) -> Res<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn repo_url() -> &'static str {
    REPO_URL
}

#[derive(Default)]
struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize)]
struct UpdateInfo {
    version: String,
    current: String,
    notes: Option<String>,
}

#[derive(Serialize, Clone)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// Looks for a newer signed release on GitHub.
#[tauri::command]
async fn check_update(app: AppHandle, pending: State<'_, PendingUpdate>) -> Res<Option<UpdateInfo>> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone(),
    });
    *pending.0.lock().unwrap() = update;
    Ok(info)
}

/// Downloads, verifies and installs the update found by `check_update`, then
/// restarts the app. VPN sessions run in the openvpn3 services and stay up.
#[tauri::command]
async fn install_update(app: AppHandle, pending: State<'_, PendingUpdate>) -> Res<()> {
    let update = pending.0.lock().unwrap().take().ok_or("No update available")?;
    let mut downloaded = 0u64;
    let handle = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = handle.emit("update-progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(PendingUpdate::default())
        .setup(|app| {
            let client = tauri::async_runtime::block_on(Client::system()).map_err(|e| e.to_string());
            if let Ok(c) = &client {
                let c = c.clone();
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let result = c
                        .watch_signals(|ev| {
                            let _ = handle.emit("ovpn-signal", ev);
                        })
                        .await;
                    if let Err(e) = result {
                        eprintln!("signal listener stopped: {e}");
                    }
                });
            }
            app.manage(Backend(client));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            backend_version,
            list_profiles,
            list_sessions,
            pick_and_import,
            import_profile,
            remove_profile,
            connect,
            submit_input,
            pending_input,
            disconnect,
            pause,
            resume,
            restart,
            open_url,
            repo_url,
            check_update,
            install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tunnelo");
}
