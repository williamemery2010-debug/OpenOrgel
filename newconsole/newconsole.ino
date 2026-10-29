// ============================================================================
// OPENORGEL CENTRIFUGAL BLOWER CONTROLLER & TELEMETRY (IBT-2 BTS7960)
// ============================================================================
// Hardware Configuration:
//   - Pin 3  (pwmPin):          PWM speed output driving IBT-2 RPWM (Pin 3)
//                               (Centrifugal blower forward drive; LPWM held LOW/GND)
//   - Pin 53 (enPin):           Driver Enable output driving IBT-2 R_EN and L_EN
//                               (HIGH = H-bridge active, LOW = H-bridge disabled)
//   - Pin A0 (currentSensePin): Analog input measuring IBT-2 IS current-sense output
//                               (R_IS / L_IS connected to Pin A0)
//   - Pin 8  (switchPin):       Physical ON-ON toggle switch wired between Pin 8 & GND
//                               (Uses internal pull-up resistor: LOW = ON, HIGH = OFF)
//
// Standalone Operation Guarantee:
//   - When Pin 8 switch is ON (Pin 8 connected to GND / LOW):
//       * Pin 53 (enPin) is driven HIGH (IBT-2 BTS7960 output stage ENABLED).
//       * Pin 3 (pwmPin) is driven at currentPwmPercent (nominal 50.0% on boot).
//       * The centrifugal organ blower runs reliably and continuously EVEN IF:
//           - No USB cable is connected (standalone physical organ performance)
//           - The laptop microphone tuner is OFF or disabled
//           - The OpenOrgel desktop application is not open or running
//   - When Pin 8 switch is OFF (HIGH):
//       * Pin 53 (enPin) is driven LOW (IBT-2 BTS7960 disabled to prevent power draw).
//       * Pin 3 (pwmPin) is set to 0.
//       * currentPwmPercent resets to 50.0% nominal ready for the next startup.
//       * Measured motor current is clamped to 0.00 A.
//
// BTS7960 Current Measurement Physics:
//   - The IBT-2 module includes an onboard 1 kOhm pull-down resistor on each IS pin.
//   - BTS7960 typical current mirror ratio: K_ILIS = 8500 (1 mA output per 8.5 A load).
//   - Output voltage: V_IS = (I_load / 8500) * R_IS = (I_load / 8500) * 1000 Ohm = I_load * 0.1176 V/A.
//   - Inverting: I_load = V_IS * 8.5 A.
//   - With an Arduino 5.0V 10-bit ADC (1023 max counts):
//     I_load = ADC_reading * (5.0 * 8.5 / 1023.0) ~= ADC_reading * 0.041544 A.
//   - To eliminate high-frequency ripple from the PWM switching waveform, we perform
//     multi-sample oversampling (16 readings) combined with an Exponential Moving Average
//     (EMA) low-pass filter (alpha = 0.20):
//       filteredCurrent = alpha * rawCurrent + (1 - alpha) * filteredCurrent;
//
// Telemetry & Control Protocol (115200 baud over USB Serial):
//   - Telemetry emitted to OpenOrgel GUI:
//       * "BLOWER:1" / "BLOWER:0"  -> Hardware switch state
//       * "PWM:<percent>"          -> Live blower duty cycle (0.0 to 100.0%)
//       * "CURRENT:<amps>"         -> Live motor current in Amperes (e.g. "CURRENT:1.85")
//   - Control commands received from OpenOrgel:
//       * 'F' or 'f'               -> Pipe is Flat:  increase blower PWM by +0.5%
//       * 'S' or 's'               -> Pipe is Sharp: decrease blower PWM by -0.5%
//       * 'R' or 'r'               -> Reset blower PWM to nominal 50.0%
//       * "PWM:<0..100>\n"         -> Direct PID / manual PWM duty cycle target (%)
//       * "A4_1" / "A4_0"          -> Safely ignored (no-op)
//   - Enforces a 500 ms anti-oscillation settling delay between step adjustments
//     so the blower impeller and windchest reservoir stabilize before the next step.
// ============================================================================

const int pwmPin = 3;           // PWM output to IBT-2 RPWM (Pin 3)
const int enPin = 53;           // Driver Enable output to IBT-2 R_EN and L_EN (Pin 53)
const int currentSensePin = A0; // Analog input for IBT-2 IS current sense (Pin A0)
const int switchPin = 8;        // ON-ON toggle switch (Pin 8 to GND, active LOW)

float currentPwmPercent = 50.0;            // Nominal starting pressure at 50.0% power
unsigned long lastAdjustmentTime = 0;      // Timestamp (ms) of the last pitch adjustment
const unsigned long adjustmentDelay = 500; // 500 ms settling delay to prevent oscillation

bool lastSwitchState = false;              // Tracks Pin 8 switch transitions
unsigned long lastTelemetryTime = 0;       // Periodic telemetry stream timestamp
float smoothedCurrentAmps = 0.0;           // Low-pass filtered motor current in Amperes
String lineBuffer = "";                    // Line accumulator for serial commands

// Helper: converts 0.0-100.0% duty cycle to 8-bit timer value (0-255) and writes to Pin 3
void applyBlowerPwm(float percent) {
  int pwmCounts = (int)((percent / 100.0) * 255.0 + 0.5);
  if (pwmCounts < 0) pwmCounts = 0;
  if (pwmCounts > 255) pwmCounts = 255;
  analogWrite(pwmPin, pwmCounts);
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
  // Calculate instantaneous load current in Amperes: ADC * (5.0V * 8.5A / 1023)
  float rawAmps = avgAdc * (5.0 * 8.5 / 1023.0);
  if (rawAmps < 0.02) rawAmps = 0.0; // Noise threshold clamp

  // Exponential Moving Average low-pass filter (alpha = 0.20)
  const float alpha = 0.20;
  smoothedCurrentAmps = (alpha * rawAmps) + ((1.0 - alpha) * smoothedCurrentAmps);
  return smoothedCurrentAmps;
}

// Transmits live PWM duty cycle and motor current in Amperes back to the OpenOrgel GUI
void reportTelemetry() {
  Serial.print("PWM:");
  Serial.println(currentPwmPercent, 1);
  Serial.print("CURRENT:");
  Serial.println(smoothedCurrentAmps, 2);
}

// Processes a single step command ('F' = Flat -> +0.5%, 'S' = Sharp -> -0.5%, 'R' = Reset 50%)
void processStepCommand(char command) {
  unsigned long now = millis();

  if (command == 'R' || command == 'r') {
    currentPwmPercent = 50.0;
    lastAdjustmentTime = now;
    reportTelemetry();
    return;
  }

  // Only allow +0.5% / -0.5% adjustments if the 500 ms anti-oscillation delay has elapsed
  if (now - lastAdjustmentTime >= adjustmentDelay) {
    if (command == 'F' || command == 'f') {
      // Pitch is flat -> increase blower pressure by +0.5%
      currentPwmPercent += 0.5;
      if (currentPwmPercent > 100.0) currentPwmPercent = 100.0;
      lastAdjustmentTime = now;
      reportTelemetry();
    } else if (command == 'S' || command == 's') {
      // Pitch is sharp -> decrease blower pressure by -0.5%
      currentPwmPercent -= 0.5;
      if (currentPwmPercent < 0.0) currentPwmPercent = 0.0;
      lastAdjustmentTime = now;
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

  // Direct PWM setpoint command from OpenOrgel PID / Reset controls ("PWM:50.0")
  if (cmd.startsWith("PWM:")) {
    float target = cmd.substring(4).toFloat();
    if (target < 0.0) target = 0.0;
    if (target > 100.0) target = 100.0;
    currentPwmPercent = target;
    lastAdjustmentTime = millis();
    reportTelemetry();
    return;
  }
}

void setup() {
  Serial.begin(115200); // High baud rate (115200) for low-latency communication with OpenOrgel

  pinMode(pwmPin, OUTPUT);
  pinMode(enPin, OUTPUT);
  pinMode(switchPin, INPUT_PULLUP); // ON-ON toggle switch between Pin 8 & GND
  pinMode(currentSensePin, INPUT);

  // Initialize outputs safely disabled
  digitalWrite(enPin, LOW);
  analogWrite(pwmPin, 0);

  // Read initial physical switch state
  bool isSystemOn = (digitalRead(switchPin) == LOW);
  lastSwitchState = isSystemOn;

  if (isSystemOn) {
    // Switch is ON on boot: enable driver immediately for standalone operation
    digitalWrite(enPin, HIGH);
    applyBlowerPwm(currentPwmPercent);
  }
}

void loop() {
  // Read Pin 8 ON-ON toggle switch state (LOW means Pin 8 connects to GND -> System ON)
  bool isSystemOn = (digitalRead(switchPin) == LOW);

  // Handle physical switch state changes
  if (isSystemOn != lastSwitchState) {
    lastSwitchState = isSystemOn;

    if (isSystemOn) {
      // Switch flipped ON: Enable IBT-2 driver (Pin 53) and apply nominal PWM (Pin 3)
      digitalWrite(enPin, HIGH);
      applyBlowerPwm(currentPwmPercent);
    } else {
      // Switch flipped OFF: Disable IBT-2 driver immediately
      digitalWrite(enPin, LOW);
      analogWrite(pwmPin, 0);
      currentPwmPercent = 50.0; // Reset to 50.0% nominal ready for next startup
      smoothedCurrentAmps = 0.0;
    }

    // Immediately report switch transition event to OpenOrgel
    Serial.print("BLOWER:");
    Serial.println(isSystemOn ? 1 : 0);
    if (isSystemOn) {
      reportTelemetry();
    }
  }

  // When system is OFF: keep outputs disabled, drain serial buffer, and broadcast OFF heartbeat
  if (!isSystemOn) {
    digitalWrite(enPin, LOW);
    analogWrite(pwmPin, 0);
    smoothedCurrentAmps = 0.0;

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
    }
    return;
  }

  // System is ON:
  // Ensure IBT-2 BTS7960 enable pin (Pin 53) is active HIGH
  digitalWrite(enPin, HIGH);

  // Apply current PWM duty cycle to Pin 3 (runs reliably standalone even if tuner/app is closed)
  applyBlowerPwm(currentPwmPercent);

  // Measure motor current via BTS7960 IS on Pin A0
  readMotorCurrentAmps();

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
