use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use evdev::{Device, KeyCode};
use futures::{future::join_all, stream::StreamExt};
use log::{info, warn};
use tokio::{
    signal,
    time::{Duration as TDuration, sleep},
};
use tokio_udev::{AsyncMonitorSocket, EventType, MonitorBuilder};

use keyheatmap::*;

async fn watch_devices(map: SharedMap) -> anyhow::Result<()> {
    let monitor = MonitorBuilder::new()?.match_subsystem("input")?.listen()?;

    let mut monitor = AsyncMonitorSocket::new(monitor)?;

    while let Some(event) = monitor.next().await {
        let event: tokio_udev::Event = event?;

        match event.event_type() {
            EventType::Add => {
                if let Some(path) = event.devnode() {
                    if !path.to_string_lossy().starts_with("/dev/input/event") {
                        continue;
                    }
                    info!("added: {}", path.display());
                    let map = map.clone();
                    sleep(TDuration::from_millis(100)).await;
                    let device = Device::open(path).unwrap();
                    tokio::spawn(setup_device_listen(device, map));
                }
            }
            _ => {}
        }
    }

    Ok(())
}

async fn setup_device_listen(device: Device, map: SharedMap) -> anyhow::Result<()> {
    let mut keys_pressed = HashMap::new();
    let name = device.name().unwrap_or("unknown").to_string();
    let path = device.physical_path().unwrap_or("unknown").to_string();

    if path.ends_with("ALSA") {
        info!("skipping audio dev: {} {}", name, path);
        return Ok(());
    }

    info!("listening dev: {} {}", name, path);
    let mut events = device.into_event_stream()?;
    while let Ok(ev) = events.next_event().await {
        if ev.event_type() == evdev::EventType::RELATIVE {
            use evdev::{KeyCode, RelativeAxisCode};
            let key = match RelativeAxisCode(ev.code()) {
                RelativeAxisCode::REL_WHEEL => {
                    if ev.value() > 0 {
                        KeyCode::KEY_SCROLLUP
                    } else if ev.value() < 0 {
                        KeyCode::KEY_SCROLLDOWN
                    } else {
                        continue;
                    }
                }
                RelativeAxisCode::REL_HWHEEL => {
                    if ev.value() > 0 {
                        KeyCode::KEY_KPRIGHTPAREN
                    } else if ev.value() < 0 {
                        KeyCode::KEY_KPLEFTPAREN
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };
            let mut map = map.lock().unwrap();
            let entry = map.entry(key).or_insert_with(KeyLog::default);
            entry.time_ms += 100;
            entry.count += 1;
        } else if ev.event_type() != evdev::EventType::KEY {
            continue;
        }
        match ev.value() {
            0 => {
                let code = ev.code();
                let key = KeyCode::new(code);
                info!("released:  {}", code);
                let Some(time_start) = keys_pressed.remove(&code) else {
                    continue;
                };
                let time_elapsed = ev
                    .timestamp()
                    .duration_since(time_start)
                    .unwrap_or(Duration::new(0, 0));
                let mut map = map.lock().unwrap();
                let entry = map.entry(key).or_insert_with(KeyLog::default);
                entry.time_ms += time_elapsed.as_millis();
                entry.count += 1;
            }
            1 => {
                let code = ev.code();
                info!("pressed:   {}", code);
                keys_pressed.insert(code, ev.timestamp());
            }
            2 => {
                let code = ev.code();
                //let key = KeyCode::new(code);
                //let mut map = map.lock().unwrap();
                //map.entry(KeyCode::new(ev.code())).and_modify(|v| {v.count += 1}).or_insert(KeyLog::default());
                info!("repeat:    {}", code);
            }
            _ => (),
        }
    }
    info!("closing dev: {}", path);

    Ok(())
}

async fn save_hashmap_loop(map: SharedMap) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    interval.tick().await;

    loop {
        interval.tick().await;
        save_hashmap(&map).await;
    }
}

async fn listen_kill(map: SharedMap) {
    let Ok(mut sigterm) = signal::unix::signal(signal::unix::SignalKind::terminate()) else {
        warn!("failed to install SIGTERM handler, graceful shutdown impossible");
        return;
    };

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Ctrl+C received");
        }
        _ = sigterm.recv() => {
            info!("SIGTERM received");
        }
    }

    save_hashmap(&map).await;

    std::process::exit(0)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = env_logger::Env::new().filter_or("RUST_LOG", "warn");
    env_logger::Builder::from_env(env).init();

    let map: SharedMap = Arc::new(Mutex::new(load_hashmap().await));

    let futures = evdev::enumerate().map(|t| {
        let device = t.1;
        let map = map.clone();

        tokio::spawn(setup_device_listen(device, map))
    });

    let _ = tokio::join!(
        join_all(futures),
        save_hashmap_loop(map.clone()),
        watch_devices(map.clone()),
        listen_kill(map)
    );
    Ok(())
}
