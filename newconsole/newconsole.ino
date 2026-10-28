// ============================================================================
// OPENORGEL CENTRIFUGAL BLOWER AIR-PRESSURE & PITCH-LOCK CONTROLLER
// ============================================================================
// Hardware Configuration:
//   - Pin 3 (pwmPin):    PWM output driving the GMOS / MOSFET motor controller
//                        for the centrifugal organ blower.
//   - Pin 8 (switchPin): ON-ON toggle switch wired between Pin 8 and GND
//                        (uses internal pull-up resistor: LOW = ON, HIGH = OFF).
//
// Control Architecture:
//   - Starts at nominal windchest pressure: 50.0% PWM duty cycle (127.5 / 255).
//   - Receives real-time pitch error / step commands over USB Serial (115200 baud)
//     from the OpenOrgel laptop microphone chromatic / Eb5 pitch detector:
//       * 'F' or 'f'       -> Pipe is Flat:  increase blower PWM by +0.5%
//       * 'S' or 's'       -> Pipe is Sharp: decrease blower PWM by -0.5%
//       * 'R' or 'r'       -> Reset blower PWM to nominal 50.0%
//       * "PWM:<0..100>\n" -> Direct PID / manual PWM duty cycle target (%)
//   - Enforces a 500 ms anti-oscillation settling delay between step adjustments
//     so the blower impeller and windchest reservoir stabilize before the next step.
//   - Reports hardware switch state ("BLOWER:1" / "BLOWER:0") and live PWM duty
//     cycle ("PWM:<percent>") back to the OpenOrgel desktop console.
// ============================================================================

const int pwmPin = 3;      // PWM output to the GMOS/MOSFET motor controller (Pin 3)
const int switchPin = 8;   // ON-ON toggle switch (Pin 8 to GND, active LOW)

float currentPwmPercent = 50.0;            // Nominal starting pressure at 50.0% power
unsigned long lastAdjustmentTime = 0;      // Timestamp (ms) of the last pitch adjustment
const unsigned long adjustmentDelay = 500; // 500 ms settling delay to prevent oscillation

bool lastSwitchState = false;              // Tracks Pin 8 switch changes for telemetry
unsigned long lastTelemetryTime = 0;       // Periodic telemetry heartbeat timestamp
String lineBuffer = "";                    // Line accumulator for multi-character commands

// Helper: converts 0.0-100.0% duty cycle to 8-bit timer value (0-255) and writes to Pin 3
void applyBlowerPwm(float percent) {
  int pwmCounts = (int)((percent / 100.0) * 255.0 + 0.5);
  if (pwmCounts < 0) pwmCounts = 0;
  if (pwmCounts > 255) pwmCounts = 255;
  analogWrite(pwmPin, pwmCounts);
}

// Helper: transmits current PWM percentage back to the OpenOrgel GUI
void reportPwmTelemetry() {
  Serial.print("PWM:");
  Serial.println(currentPwmPercent, 1);
}

// Processes a single step command ('F' = Flat -> +0.5%, 'S' = Sharp -> -0.5%, 'R' = Reset 50%)
void processStepCommand(char command) {
  unsigned long now = millis();

  if (command == 'R' || command == 'r') {
    currentPwmPercent = 50.0;
    lastAdjustmentTime = now;
    reportPwmTelemetry();
    return;
  }

  // Only allow +0.5% / -0.5% adjustments if the 500 ms anti-oscillation delay has elapsed
  if (now - lastAdjustmentTime >= adjustmentDelay) {
    if (command == 'F' || command == 'f') {
      // Pitch is flat -> increase blower pressure by +0.5%
      currentPwmPercent += 0.5;
      if (currentPwmPercent > 100.0) currentPwmPercent = 100.0;
      lastAdjustmentTime = now;
      reportPwmTelemetry();
    } else if (command == 'S' || command == 's') {
      // Pitch is sharp -> decrease blower pressure by -0.5%
      currentPwmPercent -= 0.5;
      if (currentPwmPercent < 0.0) currentPwmPercent = 0.0;
      lastAdjustmentTime = now;
      reportPwmTelemetry();
    }
    // If the pitch is in tune, the software sends nothing and currentPwmPercent is maintained.
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
    reportPwmTelemetry();
    return;
  }
}

void setup() {
  Serial.begin(115200); // High baud rate (115200) for low-latency communication with OpenOrgel

  pinMode(pwmPin, OUTPUT);
  // Configure Pin 8 with internal pull-up resistor (wire ON-ON toggle switch between Pin 8 and GND)
  pinMode(switchPin, INPUT_PULLUP);
  analogWrite(pwmPin, 0);
}

void loop() {
  // Read Pin 8 ON-ON toggle switch state (LOW means Pin 8 is connected to GND -> System ON)
  bool isSystemOn = (digitalRead(switchPin) == LOW);

  // Report switch state transitions immediately to OpenOrgel
  if (isSystemOn != lastSwitchState || (millis() - lastTelemetryTime >= 2000)) {
    lastSwitchState = isSystemOn;
    lastTelemetryTime = millis();
    Serial.print("BLOWER:");
    Serial.println(isSystemOn ? 1 : 0);
    if (isSystemOn) {
      reportPwmTelemetry();
    }
  }

  if (!isSystemOn) {
    analogWrite(pwmPin, 0);       // Stop the centrifugal blower motor
    currentPwmPercent = 50.0;     // Reset to 50.0% nominal pressure for the next startup
    // Drain any queued serial bytes while the blower switch is OFF
    while (Serial.available() > 0) {
      Serial.read();
    }
    lineBuffer = "";
    return;                       // Skip pitch adjustments while switch is OFF
  }

  // System is ON: apply the current PWM duty cycle to Pin 3
  applyBlowerPwm(currentPwmPercent);

  // Check for incoming pitch-correction commands from the OpenOrgel software
  while (Serial.available() > 0) {
    char c = (char)Serial.read();

    if (c == '\n' || c == '\r') {
      if (lineBuffer.length() > 0) {
        processLineCommand(lineBuffer);
        lineBuffer = "";
      }
    } else if (lineBuffer.length() == 0 && (c == 'F' || c == 'f' || c == 'S' || c == 's' || c == 'R' || c == 'r')) {
      // Immediate support for raw single-byte 'F', 'S', or 'R' characters even without a newline
      processStepCommand(c);
    } else if (lineBuffer.length() < 64) {
      lineBuffer += c;
    }
  }
}
