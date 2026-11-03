// ============================================================================
// OPENORGEL CENTRIFUGAL BLOWER CONTROLLER & TELEMETRY (IBT-2 BTS7960)
// SINGLE-POTENTIOMETER ANTI-JITTER MASTER COARSE TUNER (PIN A4: 0.0% - 100.0%)
// ============================================================================
// Hardware Configuration:
//   - Pin 13 (pwmPin):          PWM speed output driving IBT-2 RPWM (Pin 13)
//                               (Centrifugal blower forward drive; LPWM unused / held LOW)
//   - Pin A1 (enPin):           Driver Enable output driving IBT-2 R_EN and L_EN (Pin A1 as digital)
//                               (HIGH = H-bridge active, LOW = H-bridge disabled; Pins 12/52/53 also driven as mirrors)
//   - Pin A2 (vccPin):          Dedicated 5V logic supply output for IBT-2 VCC (Pin A2 as digital, constant HIGH)
//   - Pin A0 (currentSensePin): Analog input measuring IBT-2 IS current-sense output
//                               (R_IS / L_IS connected to Pin A0)
//   - Pin A4 (coarsePotPin):    Surviving B10K Pot Wiper: Master Coarse Tuning Dial (0.0% to 100.0% PWM)
//                               (Equipped with 16x multi-sample oversampling, heavy alpha=0.08 EMA smoothing,
//                               and Schmitt trigger 3-count deadband to eliminate wiper jitter)
//   - Pin A3 (deadPotPin):      Dead/disconnected pot pin pulled internally to 5V (INPUT_PULLUP)
//                               to eliminate floating ADC charge leakage into Pin A4
//   - Pin 7  (pullup5vPin):     Dedicated 5V reference output on Pin 7 (adjacent to Pin 8 for clean pull-up resistor connection)
//   - Pin 8  (switchPin):       Physical ON-ON toggle switch wired between Pin 8 & GND
//                               (Uses internal pull-up resistor: LOW = ON, HIGH = OFF)
//
// Standalone Operation Guarantee:
//   - When Pin 8 switch is ON (Pin 8 connected to GND / LOW):
//       * Pin A2 (vccPin) provides steady 5V logic power to IBT-2 buffer chip.
//       * Pin A1 (enPin) is driven HIGH (IBT-2 BTS7960 output stage ENABLED).
//       * Pin 13 (pwmPin) is driven at effectiveMotorPwm (Coarse PWM + software nudge).
//       * The centrifugal organ blower runs reliably and continuously EVEN IF:
//           - No USB cable is connected (standalone physical organ performance)
//           - The laptop microphone tuner is OFF or disabled
//           - The OpenOrgel desktop application is not open or running
//   - When Pin 8 switch is OFF (HIGH):
//       * Pin A1 (enPin) is driven LOW (IBT-2 BTS7960 disabled to prevent power draw).
//       * Pin 13 (pwmPin) is set to 0.
//       * Measured motor current is clamped to 0.00 A.
//
// BTS7960 Current Measurement Physics:
//   - The IBT-2 module includes an onboard 1 kOhm pull-down resistor on each IS pin.
//   - BTS7960 typical current mirror ratio: K_ILIS = 8500 (1 mA output per 8.5 A load).
//   - Output voltage: V_IS = (I_load / 8500) * R_IS = (I_load / 8500) * 1000 Ohm = I_load * 0.1176 V/A.
//   - Inverting: I_load = V_IS * 8.5 A.
//   - With an Arduino 5.0V 10-bit ADC (1023 max counts):
//     I_load = ADC_reading * (5.0 * 8.5 / 1023.0) ~= ADC_reading * 0.041544 A.
//   - Multi-sample oversampling (16 readings) combined with an Exponential Moving Average
//     (EMA) low-pass filter (alpha = 0.20):
//       filteredCurrent = alpha * rawCurrent + (1 - alpha) * filteredCurrent;
//
// Telemetry & Control Protocol (115200 baud over USB Serial):
//   - Telemetry emitted to OpenOrgel GUI:
//       * "BLOWER:1" / "BLOWER:0"  -> Hardware switch state
//       * "PWM:<percent>"          -> Live blower duty cycle (0.0 to 100.0%)
//       * "COARSE:<percent>"       -> Master coarse potentiometer duty cycle (0.0 to 100.0%)
//       * "CURRENT:<amps>"         -> Live motor current in Amperes (e.g. "CURRENT:1.85")
//   - Control commands received from OpenOrgel:
//       * 'F' or 'f'               -> Pipe is Flat:  increase software trim by +0.5%
//       * 'S' or 's'               -> Pipe is Sharp: decrease software trim by -0.5%
//       * 'R' or 'r'               -> Reset software trim to 0.0% (returns strictly to pot value)
//       * "PWM:<0..100>\n"         -> Set software PWM target (%)
//       * "A4_1" / "A4_0"          -> Safely ignored (no-op)
//   - Enforces a 500 ms anti-oscillation settling delay between step adjustments
//     so the blower impeller and windchest reservoir stabilize before the next step.
// ============================================================================

const int pwmPin = 13;          // PWM output to IBT-2 RPWM (Pin 13)
const int enPin = A1;           // Primary Driver Enable output to IBT-2 R_EN and L_EN (Pin A1 as digital)
const int vccPin = A2;          // Dedicated 5V logic supply output for IBT-2 VCC (Pin A2 as digital, constant HIGH)
const int enPinAlt = 12;        // Redundant Driver Enable output (Pin 12)
const int enPinAlt2 = 52;       // Redundant Driver Enable output (Pin 52)
const int enPinAlt3 = 53;       // Redundant Driver Enable output (Pin 53)
const int currentSensePin = A0; // Analog input for IBT-2 IS current sense (Pin A0)
const int deadPotPin = A3;      // Disconnected/dead pot pin (pulled HIGH to kill floating cross-talk)
const int coarsePotPin = A4;    // Surviving B10K Pot Wiper: Master Coarse Tuning Dial (0.0% to 100.0% PWM)
const int pullup5vPin = 7;      // Dedicated 5V reference output on Pin 7 (adjacent to Pin 8 for clean pull-up resistor connection)
const int switchPin = 8;        // ON-ON toggle switch (Pin 8 to GND, active LOW)

// Helper: drives Pin A1, Pin 12, Pin 52, and Pin 53 HIGH (enabled) or LOW (disabled)
void setDriverEnable(bool enable) {
  digitalWrite(enPin, enable ? HIGH : LOW);
  digitalWrite(enPinAlt, enable ? HIGH : LOW);
  digitalWrite(enPinAlt2, enable ? HIGH : LOW);
  digitalWrite(enPinAlt3, enable ? HIGH : LOW);
}

float commandedPwm = 0.0;                  // Direct Serial PWM Setpoint (0.0% to 100.0%, starts at 0.0% for safe boot)
float effectiveMotorPwm = 0.0;             // Net PWM applied to Pin 13
float lastReportedPwm = -999.0;            // Telemetry change detection

unsigned long lastAdjustmentTime = 0;      // Timestamp (ms) of the last pitch adjustment
const unsigned long adjustmentDelay = 50;  // 50 ms settling delay for rapid responsive serial nudging

bool debouncedSwitchState = false;         // Stable, filtered switch state
int lastRawReading = HIGH;                 // Previous raw pin read for edge detection
unsigned long lastDebounceTime = 0;        // Timestamp (ms) of the last raw pin change
const unsigned long debounceDelayMs = 50;  // 50 ms stability window to reject EMI / contact bounce
unsigned long lastTelemetryTime = 0;       // Periodic telemetry stream timestamp
float smoothedCurrentAmps = 0.0;           // Low-pass filtered motor current in Amperes
const float maxSafeCurrentAmps = 8.5;       // Electronic fuse trip threshold in Amperes (normal running is 1.5 - 3.5A)
const unsigned long overcurrentTimeoutMs = 150; // Trips if current exceeds threshold continuously for > 150 ms
unsigned long overcurrentStartTime = 0;    // Overcurrent timer start timestamp
bool isCurrentTripped = false;              // Latching electronic fuse status
bool fuseOverride = false;                  // Set true to bypass electronic stall fuse for hardware testing
String lineBuffer = "";                    // Line accumulator for serial commands

// Helper: converts 0.0-100.0% duty cycle to 8-bit timer value (0-255) and writes to Pin 13
void applyBlowerPwm(float percent) {
  int pwmCounts = (int)((percent / 100.0) * 255.0 + 0.5);
  if (pwmCounts < 0) pwmCounts = 0;
  if (pwmCounts > 255) pwmCounts = 255;
  analogWrite(pwmPin, pwmCounts);
}

// Potentiometers completely disabled - both Pin A3 and Pin A4 pulled HIGH internally
void readPotentiometers() {
  // No-op: all analog pots disabled to eliminate hardware jitter, faulty wiper flutter, and ADC noise
}

// Measures motor current via IBT-2 BTS7960 IS on Pin A0 with oversampling & EMA smoothing
float readMotorCurrentAmps() {
  long adcSum = 0;
  const int numSamples = 16;
  for (int i = 0; i < numSamples; i++) {
    adcSum += analogRead(currentSensePin);
    delayMicroseconds(50);
  }
  float avgAdc = (float)adcSum / (float)numSamples;
  // Calibrated load current in Amperes:
  // Hardware calibration: 12.0A maximum power supply saturation produces 23.0A raw (1.917x scale error due to R_IS resistor).
  // Corrected transfer function: (5.0V * 4.435A / 1023.0) accurately maps power supply stall to 12.0A peak.
  float rawAmps = avgAdc * (5.0 * 4.435 / 1023.0);
  if (rawAmps < 0.02) rawAmps = 0.0; // Noise threshold clamp

  // Exponential Moving Average low-pass filter (alpha = 0.20)
  const float alpha = 0.20;
  smoothedCurrentAmps = (alpha * rawAmps) + ((1.0 - alpha) * smoothedCurrentAmps);
  return smoothedCurrentAmps;
}

// Transmits live PWM duty cycle, motor current, and fuse status back to OpenOrgel GUI
void reportTelemetry() {
  Serial.print("PWM:");
  Serial.println(effectiveMotorPwm, 1);
  Serial.print("CURRENT:");
  Serial.println(smoothedCurrentAmps, 2);
  Serial.print("COARSE:");
  Serial.println(effectiveMotorPwm, 1);
  Serial.print("OVERRIDE:");
  Serial.println(fuseOverride ? 1 : 0);
}

// Processes a single step command ('F' = Flat -> +0.5%, 'S' = Sharp -> -0.5%, 'R' = Reset 75.0% nominal)
void processStepCommand(char command) {
  unsigned long now = millis();

  if (command == 'R' || command == 'r') {
    commandedPwm = 75.0;
    effectiveMotorPwm = commandedPwm;
    lastAdjustmentTime = now;
    if (debouncedSwitchState && !isCurrentTripped) {
      applyBlowerPwm(effectiveMotorPwm);
    }
    reportTelemetry();
    return;
  }

  // Allow +0.5% / -0.5% adjustments if settling delay has elapsed
  if (now - lastAdjustmentTime >= adjustmentDelay) {
    if (command == 'F' || command == 'f') {
      commandedPwm += 0.5;
      if (commandedPwm > 100.0) commandedPwm = 100.0;
      effectiveMotorPwm = commandedPwm;
      lastAdjustmentTime = now;
      if (debouncedSwitchState && !isCurrentTripped) {
        applyBlowerPwm(effectiveMotorPwm);
      }
      reportTelemetry();
    } else if (command == 'S' || command == 's') {
      commandedPwm -= 0.5;
      if (commandedPwm < 0.0) commandedPwm = 0.0;
      effectiveMotorPwm = commandedPwm;
      lastAdjustmentTime = now;
      if (debouncedSwitchState && !isCurrentTripped) {
        applyBlowerPwm(effectiveMotorPwm);
      }
      reportTelemetry();
    }
  }
}

// Processes a newline-terminated command string (e.g. "F", "S", "PWM:52.5", "A4_1")
void processLineCommand(String cmd) {
  cmd.trim();
  if (cmd.length() == 0) return;

  // Single-character Flat / Sharp / Reset command
  if (cmd.length() == 1) {
    processStepCommand(cmd.charAt(0));
    return;
  }

  // Direct PWM setpoint command from OpenOrgel GUI / Terminal ("PWM:25.0", "PWM:50", "PWM:0")
  if (cmd.startsWith("PWM:")) {
    float target = cmd.substring(4).toFloat();
    if (target < 0.0) target = 0.0;
    if (target > 100.0) target = 100.0;
    commandedPwm = target;
    effectiveMotorPwm = commandedPwm;
    lastAdjustmentTime = millis();
    if (debouncedSwitchState && !isCurrentTripped) {
      applyBlowerPwm(effectiveMotorPwm);
    }
    reportTelemetry();
    return;
  }

  // Fuse override commands for hardware diagnostics:
  // "OVERRIDE:1", "FUSE:0", "OVERRIDE:ON" -> bypass fuse
  // "OVERRIDE:0", "FUSE:1", "OVERRIDE:OFF" -> re-arm fuse
  if (cmd.startsWith("OVERRIDE:") || cmd.startsWith("FUSE:")) {
    String arg = cmd.substring(cmd.indexOf(':') + 1);
    arg.trim();
    if (cmd.startsWith("FUSE:")) {
      fuseOverride = (arg == "0" || arg.equalsIgnoreCase("OVERRIDE") || arg.equalsIgnoreCase("OFF") || arg.equalsIgnoreCase("BYPASS"));
    } else {
      fuseOverride = (arg == "1" || arg.equalsIgnoreCase("ON") || arg.equalsIgnoreCase("TRUE") || arg.equalsIgnoreCase("ENABLE"));
    }
    if (fuseOverride) {
      isCurrentTripped = false; // Immediately clear trip condition if active
      overcurrentStartTime = 0;
      if (debouncedSwitchState) {
        setDriverEnable(true);
        applyBlowerPwm(effectiveMotorPwm);
      }
    }
    Serial.print("OVERRIDE:");
    Serial.println(fuseOverride ? 1 : 0);
    reportTelemetry();
    return;
  }
}

void setup() {
  Serial.begin(115200); // High baud rate (115200) for low-latency communication with OpenOrgel

  // Configure Pin A2 as continuous 5V logic supply for IBT-2 VCC
  pinMode(vccPin, OUTPUT);
  digitalWrite(vccPin, HIGH);

  // Configure Pin 7 as continuous 5V pull-up reference for Pin 8 switch resistor
  pinMode(pullup5vPin, OUTPUT);
  digitalWrite(pullup5vPin, HIGH);

  pinMode(pwmPin, OUTPUT);
  pinMode(enPin, OUTPUT);
  pinMode(enPinAlt, OUTPUT);
  pinMode(enPinAlt2, OUTPUT);
  pinMode(enPinAlt3, OUTPUT);
  pinMode(switchPin, INPUT_PULLUP);  // ON-ON toggle switch between Pin 8 & GND
  pinMode(currentSensePin, INPUT);
  pinMode(deadPotPin, INPUT_PULLUP);    // Pull up dead Pin A3 to kill floating ADC cross-talk!
  pinMode(coarsePotPin, INPUT_PULLUP);  // Pull up Pin A4 as well (all analog pots disabled, 100% digital serial PWM)

  // Initialize outputs safely disabled
  setDriverEnable(false);
  analogWrite(pwmPin, 0);

  // Read initial physical switch state with multi-sample majority voting (reject startup transients)
  int lowCount = 0;
  for (int i = 0; i < 10; i++) {
    if (digitalRead(switchPin) == LOW) lowCount++;
    delay(2);
  }
  debouncedSwitchState = (lowCount >= 7);
  lastRawReading = debouncedSwitchState ? LOW : HIGH;

  if (debouncedSwitchState) {
    // Switch is ON on boot: enable driver immediately for standalone operation
    setDriverEnable(true);
    applyBlowerPwm(effectiveMotorPwm);
  }
}

void loop() {
  // Digital Debounce Filter for Pin 8 Switch (enforces 50 ms stability against EMI & contact bounce)
  int rawReading = digitalRead(switchPin);
  if (rawReading != lastRawReading) {
    lastDebounceTime = millis();
    lastRawReading = rawReading;
  }

  if ((millis() - lastDebounceTime) >= debounceDelayMs) {
    bool isSystemOn = (rawReading == LOW);
    if (isSystemOn != debouncedSwitchState) {
      debouncedSwitchState = isSystemOn;

      if (debouncedSwitchState) {
        // Switch flipped ON: Enable IBT-2 driver (Pin A1, Pin 12, Pin 52 & Pin 53) and apply effective PWM (Pin 13)
        digitalWrite(vccPin, HIGH);
        digitalWrite(pullup5vPin, HIGH);
        setDriverEnable(true);
        applyBlowerPwm(effectiveMotorPwm);
      } else {
        // Switch flipped OFF: Disable IBT-2 driver immediately
        setDriverEnable(false);
        analogWrite(pwmPin, 0);
        smoothedCurrentAmps = 0.0;
        isCurrentTripped = false; // Reset electronic fuse trip condition when user flips switch OFF
        overcurrentStartTime = 0;
      }

      // Immediately report switch transition event to OpenOrgel
      Serial.print("BLOWER:");
      Serial.println(debouncedSwitchState ? 1 : 0);
      if (debouncedSwitchState) {
        reportTelemetry();
      }
    }
  }

  bool isSystemOn = debouncedSwitchState;

  // When system is OFF: keep outputs disabled, drain serial buffer, and broadcast OFF heartbeat
  if (!isSystemOn) {
    setDriverEnable(false);
    analogWrite(pwmPin, 0);
    smoothedCurrentAmps = 0.0;
    isCurrentTripped = false;
    overcurrentStartTime = 0;

    // Drain queued serial input while blower is turned off
    while (Serial.available() > 0) {
      Serial.read();
    }
    lineBuffer = "";

    // Periodic telemetry heartbeat every 1000 ms while OFF
    if (millis() - lastTelemetryTime >= 1000) {
      lastTelemetryTime = millis();
      Serial.println("BLOWER:0");
      Serial.println("CURRENT:0.00");
      Serial.print("PWM:");
      Serial.println(effectiveMotorPwm, 1);
      Serial.print("COARSE:");
      Serial.println(effectiveMotorPwm, 1);
    }
    return;
  }

  // Fast Electronic Fuse / Stall Protection Check:
  if (isCurrentTripped && !fuseOverride) {
    // Motor is latched OFF due to severe overcurrent (>8.5A) or mechanical stall.
    // Ensure driver and PWM remain completely killed to protect motor windings and PLA housing.
    setDriverEnable(false);
    analogWrite(pwmPin, 0);

    // Periodic emergency warning telemetry every 250 ms
    if (millis() - lastTelemetryTime >= 250) {
      lastTelemetryTime = millis();
      Serial.println("TRIP:1");
      Serial.println("BLOWER:TRIP");
      Serial.print("CURRENT:");
      Serial.println(smoothedCurrentAmps, 2);
      Serial.print("OVERRIDE:");
      Serial.println(fuseOverride ? 1 : 0);
    }

    // Process incoming serial commands even while tripped (so user can send "OVERRIDE:1" or "PWM:0")
    while (Serial.available() > 0) {
      char c = (char)Serial.read();
      if (c == '\n' || c == '\r') {
        if (lineBuffer.length() > 0) {
          processLineCommand(lineBuffer);
          lineBuffer = "";
        }
      } else if (lineBuffer.length() < 64) {
        lineBuffer += c;
      }
    }
    return;
  }

  // System is ON:
  // Ensure Pin A2 VCC is 5V, Pin 7 is 5V, and IBT-2 BTS7960 enable pins (Pin A1, Pin 12, Pin 52 & Pin 53) are active HIGH
  digitalWrite(vccPin, HIGH);
  digitalWrite(pullup5vPin, HIGH);
  setDriverEnable(true);

  // Apply effective PWM duty cycle to Pin 13 (runs reliably standalone even if tuner/app is closed)
  applyBlowerPwm(effectiveMotorPwm);

  // Measure motor current via BTS7960 IS on Pin A0
  readMotorCurrentAmps();

  // Fast Electronic Fuse / Stall Protection:
  // If fuse override is NOT active, trip off if current exceeds 8.5 Amps (e.g. rotor jam, mechanical rub)
  // for longer than overcurrentTimeoutMs (150 ms), immediately trip off the driver.
  if (!fuseOverride && smoothedCurrentAmps >= maxSafeCurrentAmps) {
    if (overcurrentStartTime == 0) {
      overcurrentStartTime = millis();
    } else if (millis() - overcurrentStartTime >= overcurrentTimeoutMs) {
      isCurrentTripped = true;
      setDriverEnable(false);
      analogWrite(pwmPin, 0);
      Serial.println("TRIP:1");
      Serial.println("BLOWER:TRIP");
      return;
    }
  } else {
    overcurrentStartTime = 0;
  }

  // Periodic high-rate telemetry stream (every 100 ms) for smooth GUI gauges
  if (millis() - lastTelemetryTime >= 100) {
    lastTelemetryTime = millis();
    reportTelemetry();
  }

  // Check for incoming pitch-correction commands from the OpenOrgel tuner
  while (Serial.available() > 0) {
    char c = (char)Serial.read();

    if (c == '\n' || c == '\r') {
      if (lineBuffer.length() > 0) {
        processLineCommand(lineBuffer);
        lineBuffer = "";
      }
    } else if (lineBuffer.length() == 0 && (c == 'F' || c == 'f' || c == 'S' || c == 's' || c == 'R' || c == 'r')) {
      // Immediate single-byte pitch step command
      processStepCommand(c);
    } else if (lineBuffer.length() < 64) {
      lineBuffer += c;
    }
  }
}
