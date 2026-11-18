//! End-to-end check of the openvpn3 D-Bus client against a real server.
//!
//! Usage: probe <profile.ovpn> <username> <password>

use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use tunnelo_lib::ovpn3::{Client, ConnectOutcome, InputValue};

fn main() {
    tauri::async_runtime::block_on(async {
        if let Err(e) = run().await {
            eprintln!("FAIL: {e}");
            std::process::exit(1);
        }
        println!("PASS");
    });
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let [_, file, user, pass] = args.as_slice() else {
        return Err("usage: probe <profile.ovpn> <username> <password>".into());
    };

    let client = Client::system().await.map_err(|e| e.to_string())?;
    println!("openvpn3 version: {}", client.version().await?);

    let logs = Arc::new(AtomicUsize::new(0));
    let (c, l) = (client.clone(), logs.clone());
    tauri::async_runtime::spawn(async move {
        let _ = c
            .watch_signals(|ev| {
                if l.fetch_add(1, Ordering::Relaxed) < 8 {
                    println!("  signal {} ({},{}) {}", ev.member, ev.a, ev.b, ev.message.trim_end());
                }
            })
            .await;
    });

    let profile = client.import_file(Path::new(file), Some("probe")).await?;
    println!("imported: {profile}");
    let names: Vec<String> = client.profiles().await?.into_iter().map(|p| p.name).collect();
    println!("profiles: {names:?}");
    if !names.contains(&"probe".to_string()) {
        return Err("imported profile not listed".into());
    }

    let mut outcome = client.start(&profile).await?;
    let session = loop {
        match outcome {
            ConnectOutcome::Started { session } => break session,
            ConnectOutcome::NeedsInput { session, fields } => {
                let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
                println!("needs input: {names:?}");
                let values = fields
                    .iter()
                    .map(|f| InputValue {
                        kind: f.kind,
                        group: f.group,
                        id: f.id,
                        value: if f.name == "username" { user.clone() } else { pass.clone() },
                    })
                    .collect();
                outcome = client.provide(&session, values).await?;
            }
        }
    };
    println!("session: {session}");

    let mut connected = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let sessions = client.sessions().await?;
        let Some(s) = sessions.iter().find(|s| s.path == session) else { continue };
        if s.status.minor == 7 {
            println!(
                "connected: device={} server={:?} bytes_in={:?}",
                s.device,
                s.server.as_ref().map(|x| format!("{}:{}/{}", x.ip, x.port, x.proto)),
                s.stats.get("BYTES_IN")
            );
            connected = true;
            break;
        }
        println!("  status {} {}", s.status.minor, s.status.message);
    }

    client.disconnect(&session).await.ok();
    client.remove_profile(&profile).await?;
    println!("log/status signals received: {}", logs.load(Ordering::Relaxed));
    if !connected {
        return Err("did not reach CONNECTED".into());
    }
    Ok(())
}
