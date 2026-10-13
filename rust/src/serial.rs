//! ARDUINO / TEENSY LINK - the bridge to the physical console.
//!
//! Wire protocol (unchanged, so `organconsole.ino` needs no edits):
//!   in : "<Stop Name>:1|0"   toggle a stop
//!        "ALL:1|0"           every stop on / off
//!        "VOL:0..100"        master volume
//!        "<index>:1|0"       legacy numeric stop index fallback
//!   out: "A4_1\n" / "A4_0\n" servo flag for the A4 pipe

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use serialport::{SerialPort, SerialPortType};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum SerialEvent {
    Stop { ident: String, state: bool },
    Volume(f32),
    Connected(String),
    Disconnected,
}

pub struct ArduinoLink {
    pub rx: Receiver<SerialEvent>,
    writer: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    connected: Arc<AtomicBool>,
}

fn find_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    for p in ports {
        if let SerialPortType::UsbPort(info) = &p.port_type {
            let text = format!(
                "{} {}",
                info.product.clone().unwrap_or_default(),
                info.manufacturer.clone().unwrap_or_default()
            )
            .to_lowercase();
            let vid_match = matches!(info.vid, 0x2341 | 0x2A03 | 0x1A86 | 0x16C0);
            let name_match = ["arduino", "ch340", "usb-serial", "teensy"]
                .iter()
                .any(|k| text.contains(k));
            if vid_match || name_match {
                return Some(p.port_name);
            }
        }
    }
    None
}

fn parse_line(line: &str, tx: &Sender<SerialEvent>) {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() != 2 {
        return;
    }
    let ident = parts[0];
    if ident == "VOL" {
        if let Ok(v) = parts[1].trim().parse::<i32>() {
            let _ = tx.send(SerialEvent::Volume(v as f32 / 100.0));
        }
        return;
    }
    match parts[1].trim().parse::<i32>() {
        Ok(v) => {
            let _ = tx.send(SerialEvent::Stop {
                ident: ident.to_string(),
                state: v != 0,
            });
        }
        Err(_) => eprintln!("Ignored invalid Arduino data (Not an integer state): {line}"),
    }
}

impl ArduinoLink {
    pub fn spawn(on_event: impl Fn() + Send + 'static) -> ArduinoLink {
        let (tx, rx) = unbounded();
        let writer: Arc<Mutex<Option<Box<dyn SerialPort>>>> = Arc::new(Mutex::new(None));
        let connected = Arc::new(AtomicBool::new(false));

        let w = writer.clone();
        let c = connected.clone();
        std::thread::spawn(move || loop {
            let Some(port_name) = find_port() else {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            };
            let port = match serialport::new(&port_name, 115_200)
                .timeout(Duration::from_millis(500))
                .open()
            {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Arduino open failed on {port_name}: {e}");
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
            };
            let mut reader = match port.try_clone() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Arduino clone failed: {e}");
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
            };
            *w.lock() = Some(port);
            c.store(true, Ordering::SeqCst);
            let _ = tx.send(SerialEvent::Connected(port_name.clone()));
            on_event();

            let mut acc: Vec<u8> = Vec::with_capacity(256);
            let mut chunk = [0u8; 256];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => {}
                    Ok(n) => {
                        acc.extend_from_slice(&chunk[..n]);
                        while let Some(pos) = acc.iter().position(|&b| b == b'\n') {
                            let raw: Vec<u8> = acc.drain(..=pos).collect();
                            let line = String::from_utf8_lossy(&raw).trim().to_string();
                            if !line.is_empty() {
                                parse_line(&line, &tx);
                                on_event();
                            }
                        }
                        if acc.len() > 4096 {
                            acc.clear(); // garbled stream, start over
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(e) => {
                        eprintln!("Arduino connection lost ({e}). Will try to reconnect...");
                        break;
                    }
                }
            }
            *w.lock() = None;
            c.store(false, Ordering::SeqCst);
            let _ = tx.send(SerialEvent::Disconnected);
            on_event();
            std::thread::sleep(Duration::from_secs(3));
        });

        ArduinoLink {
            rx,
            writer,
            connected,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn send_line(&self, line: &str) {
        if let Some(port) = self.writer.lock().as_mut() {
            if let Err(e) = port.write_all(line.as_bytes()) {
                eprintln!("Failed to write to Arduino: {e}");
            }
        }
    }
}
