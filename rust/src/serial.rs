//! ARDUINO / TEENSY LINK - the hardware serial bridge to the physical organ console & blower controller.
//!
//! Supported bidirectional wire protocol (`115200` baud):
//!   Inbound from Arduino -> OpenOrgel:
//!     - `"<Stop Name>:1|0"`   toggle a stop by name
//!     - `"ALL:1|0"`           toggle every stop on / off
//!     - `"VOL:0..100"`        master volume percentage
//!     - `"<index>:1|0"`       legacy numeric stop index fallback
//!     - `"BLOWER:1|0"`        Pin 8 ON-ON blower toggle switch state (`1` = ON, `0` = OFF)
//!     - `"PWM:<0.0..100.0>"`  live Pin 13 MOSFET blower PWM duty cycle percentage reported by Arduino
//!   Outbound from OpenOrgel -> Arduino:
//!     - `"F\n"`               pipe pitch is Flat  -> increase blower PWM by `+0.5%` on Pin 13
//!     - `"S\n"`               pipe pitch is Sharp -> decrease blower PWM by `-0.5%` on Pin 13
//!     - `"R\n"`               reset blower PWM to `75.0%` nominal pressure
//!     - `"PWM:<val>\n"`       explicit PWM duty cycle percentage (`0.0..100.0`)
//!     - `"A4_1\n"` / `"A4_0\n"` servo flag for the A4 pipe

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use serialport::{SerialPort, SerialPortType};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Events received from the physical Arduino console or blower controller over USB Serial.
#[derive(Debug, Clone)]
pub enum SerialEvent {
    /// Physical stop drawknob / toggle switch changed state.
    Stop { ident: String, state: bool },
    /// Master volume potentiometer / swell pedal value (`0.0` to `1.0`).
    Volume(f32),
    /// Organ dynamics / swell slider multiplier (`0.10` to `1.50`).
    Dynamics(f32),
    /// 180-degree manifold throttle valve servo angle (`0.0` to `180.0` degrees).
    ServoAngle(f32),
    /// Pin 8 ON-ON blower toggle switch state (`true` = ON / LOW, `false` = OFF / HIGH).
    BlowerSwitch(bool),
    /// Live Pin 13 blower MOSFET PWM duty cycle (`0.0%` to `100.0%`) reported by the Arduino.
    BlowerPwm(f32),
    /// Microscope Coarse Tuning Dial PWM (`0.0%` to `100.0%`) reported on Pin A3.
    CoarsePwm(f32),
    /// Microscope Fine Vernier Trim PWM (`-10.0%` to `+10.0%`) reported on Pin A4.
    FineTrim(f32),
    /// Live centrifugal blower motor current in Amperes reported by IBT-2 BTS7960 IS pin on Pin A0.
    MotorCurrent(f32),
    /// Serial connection established on the named COM port.
    Connected(String),
    /// Serial connection lost.
    Disconnected,
}

/// Thread-safe handle managing the background USB Serial reader/writer thread.
pub struct ArduinoLink {
    pub rx: Receiver<SerialEvent>,
    writer: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    connected: Arc<AtomicBool>,
}

/// Scans available USB serial ports for known Arduino / Teensy / CH340 USB-Serial VIDs and names.
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

/// Parses a single newline-delimited message from the Arduino and dispatches a `SerialEvent`.
fn parse_line(line: &str, tx: &Sender<SerialEvent>) {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() != 2 {
        return;
    }
    let ident = parts[0].trim();
    let val_str = parts[1].trim();

    if ident == "VOL" {
        if let Ok(v) = val_str.parse::<i32>() {
            let _ = tx.send(SerialEvent::Volume(v as f32 / 100.0));
        }
        return;
    }
    if ident == "BLOWER" {
        let on = val_str == "1" || val_str.eq_ignore_ascii_case("ON");
        let _ = tx.send(SerialEvent::BlowerSwitch(on));
        return;
    }
    if ident == "PWM" {
        if let Ok(pwm) = val_str.parse::<f32>() {
            let _ = tx.send(SerialEvent::BlowerPwm(pwm.clamp(0.0, 100.0)));
        }
        return;
    }
    if ident == "COARSE" {
        if let Ok(c) = val_str.parse::<f32>() {
            let _ = tx.send(SerialEvent::CoarsePwm(c.clamp(0.0, 100.0)));
        }
        return;
    }
    if ident == "FINE" || ident == "TRIM" {
        if let Ok(f) = val_str.parse::<f32>() {
            let _ = tx.send(SerialEvent::FineTrim(f.clamp(-15.0, 15.0)));
        }
        return;
    }
    if ident == "CURRENT" || ident == "AMPS" || ident == "CUR" {
        if let Ok(amps) = val_str.parse::<f32>() {
            let _ = tx.send(SerialEvent::MotorCurrent(amps.max(0.0)));
        }
        return;
    }
    if ident == "DYN" || ident == "DYNAMICS" {
        if let Ok(raw) = val_str.parse::<f32>() {
            let dyn_val = if raw > 2.0 {
                // If sent as 0..100%, map to 0.10..1.50
                0.10 + (raw.clamp(0.0, 100.0) / 100.0) * 1.40
            } else {
                raw.clamp(0.10, 1.50)
            };
            let _ = tx.send(SerialEvent::Dynamics(dyn_val));
        }
        return;
    }
    if ident == "SERVO" || ident == "VALVE" || ident == "MANIFOLD" {
        if let Ok(angle) = val_str.parse::<f32>() {
            let _ = tx.send(SerialEvent::ServoAngle(angle.clamp(0.0, 180.0)));
        }
        return;
    }
    match val_str.parse::<i32>() {
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
    /// Spawns the background auto-reconnecting USB Serial thread at `115200` baud.
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

    /// Returns true if an Arduino/Teensy console is currently connected over USB Serial.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Sends a raw string or command line over USB Serial to the connected Arduino.
    pub fn send_line(&self, line: &str) {
        if let Some(port) = self.writer.lock().as_mut() {
            if let Err(e) = port.write_all(line.as_bytes()) {
                eprintln!("Failed to write to Arduino: {e}");
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_parse_motor_current_and_telemetry() {
        let (tx, rx) = unbounded();

        // 1. Current telemetry: "CURRENT:1.85"
        parse_line("CURRENT:1.85", &tx);
        match rx.try_recv().expect("Expected MotorCurrent event") {
            SerialEvent::MotorCurrent(amps) => {
                assert!((amps - 1.85).abs() < 1e-4, "Expected 1.85A, got {amps}");
            }
            other => panic!("Unexpected event for CURRENT: {other:?}"),
        }

        // 2. Fallback aliases: "AMPS:3.42" and "CUR:0.50"
        parse_line("AMPS:3.42", &tx);
        match rx.try_recv().expect("Expected MotorCurrent event") {
            SerialEvent::MotorCurrent(amps) => {
                assert!((amps - 3.42).abs() < 1e-4, "Expected 3.42A, got {amps}");
            }
            other => panic!("Unexpected event for AMPS: {other:?}"),
        }

        // 3. Negative current clamping to 0.0
        parse_line("CURRENT:-0.15", &tx);
        match rx.try_recv().expect("Expected MotorCurrent event") {
            SerialEvent::MotorCurrent(amps) => {
                assert_eq!(amps, 0.0, "Expected negative current to clamp to 0.0");
            }
            other => panic!("Unexpected event for clamped current: {other:?}"),
        }

        // 4. Blower switch ON and OFF
        parse_line("BLOWER:1", &tx);
        match rx.try_recv().expect("Expected BlowerSwitch event") {
            SerialEvent::BlowerSwitch(on) => assert!(on),
            other => panic!("Unexpected event for BLOWER:1: {other:?}"),
        }
        parse_line("BLOWER:0", &tx);
        match rx.try_recv().expect("Expected BlowerSwitch event") {
            SerialEvent::BlowerSwitch(on) => assert!(!on),
            other => panic!("Unexpected event for BLOWER:0: {other:?}"),
        }

        // 5. Blower PWM & Dynamics Potentiometer
        parse_line("PWM:52.5", &tx);
        match rx.try_recv().expect("Expected BlowerPwm event") {
            SerialEvent::BlowerPwm(pwm) => {
                assert!((pwm - 52.5).abs() < 1e-4, "Expected 52.5%, got {pwm}");
            }
            other => panic!("Unexpected event for PWM:52.5: {other:?}"),
        }
        parse_line("DYN:1.20", &tx);
        match rx.try_recv().expect("Expected Dynamics event") {
            SerialEvent::Dynamics(d) => {
                assert!((d - 1.20).abs() < 1e-4, "Expected 1.20, got {d}");
            }
            other => panic!("Unexpected event for DYN:1.20: {other:?}"),
        }

        // 6. Ignored / malformed lines produce no events
        parse_line("A4_1", &tx);
        assert!(rx.try_recv().is_err());
        parse_line("GARBAGE", &tx);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_parse_manifold_servo_telemetry() {
        let (tx, rx) = unbounded();

        // 1. Direct servo telemetry: "SERVO:90"
        parse_line("SERVO:90", &tx);
        match rx.try_recv().expect("Expected ServoAngle event") {
            SerialEvent::ServoAngle(deg) => {
                assert!((deg - 90.0).abs() < 1e-4, "Expected 90.0 deg, got {deg}");
            }
            other => panic!("Unexpected event for SERVO:90: {other:?}"),
        }

        // 2. Alias "VALVE:135.5"
        parse_line("VALVE:135.5", &tx);
        match rx.try_recv().expect("Expected ServoAngle event") {
            SerialEvent::ServoAngle(deg) => {
                assert!((deg - 135.5).abs() < 1e-4, "Expected 135.5 deg, got {deg}");
            }
            other => panic!("Unexpected event for VALVE:135.5: {other:?}"),
        }

        // 3. Alias "MANIFOLD:0"
        parse_line("MANIFOLD:0", &tx);
        match rx.try_recv().expect("Expected ServoAngle event") {
            SerialEvent::ServoAngle(deg) => {
                assert_eq!(deg, 0.0);
            }
            other => panic!("Unexpected event for MANIFOLD:0: {other:?}"),
        }

        // 4. Clamping to 0.0 .. 180.0
        parse_line("SERVO:215.0", &tx);
        match rx.try_recv().expect("Expected ServoAngle event") {
            SerialEvent::ServoAngle(deg) => {
                assert_eq!(deg, 180.0, "Expected over-range angle to clamp to 180.0");
            }
            other => panic!("Unexpected event for clamped SERVO: {other:?}"),
        }

        parse_line("SERVO:-25.0", &tx);
        match rx.try_recv().expect("Expected ServoAngle event") {
            SerialEvent::ServoAngle(deg) => {
                assert_eq!(deg, 0.0, "Expected negative angle to clamp to 0.0");
            }
            other => panic!("Unexpected event for clamped SERVO: {other:?}"),
        }
    }

    #[test]
    fn test_parse_microscope_coarse_fine_telemetry() {
        let (tx, rx) = unbounded();

        // 1. Coarse dial reading: "COARSE:78.5"
        parse_line("COARSE:78.5", &tx);
        match rx.try_recv().expect("Expected CoarsePwm event") {
            SerialEvent::CoarsePwm(c) => {
                assert!((c - 78.5).abs() < 1e-4, "Expected 78.5%, got {c}");
            }
            other => panic!("Unexpected event for COARSE:78.5: {other:?}"),
        }

        // 2. Fine vernier trim reading: "FINE:-2.35"
        parse_line("FINE:-2.35", &tx);
        match rx.try_recv().expect("Expected FineTrim event") {
            SerialEvent::FineTrim(f) => {
                assert!((f - (-2.35)).abs() < 1e-4, "Expected -2.35%, got {f}");
            }
            other => panic!("Unexpected event for FINE:-2.35: {other:?}"),
        }

        // 3. Fine alias: "TRIM:4.10"
        parse_line("TRIM:4.10", &tx);
        match rx.try_recv().expect("Expected FineTrim event") {
            SerialEvent::FineTrim(f) => {
                assert!((f - 4.10).abs() < 1e-4, "Expected 4.10%, got {f}");
            }
            other => panic!("Unexpected event for TRIM:4.10: {other:?}"),
        }

        // 4. Clamping
        parse_line("COARSE:125.0", &tx);
        match rx.try_recv().expect("Expected CoarsePwm event") {
            SerialEvent::CoarsePwm(c) => {
                assert_eq!(c, 100.0);
            }
            other => panic!("Unexpected event for clamped COARSE: {other:?}"),
        }

        parse_line("FINE:20.0", &tx);
        match rx.try_recv().expect("Expected FineTrim event") {
            SerialEvent::FineTrim(f) => {
                assert_eq!(f, 15.0);
            }
            other => panic!("Unexpected event for clamped FINE: {other:?}"),
        }
    }
}

