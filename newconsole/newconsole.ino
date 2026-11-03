// ============================================================================
// OPENORGEL CENTRIFUGAL BLOWER CONTROLLER & TELEMETRY (IBT-2 BTS7960 & SERVO)
// ============================================================================
// Hardware Configuration:
//   - Pin 13 (pwmPin):          PWM speed output driving IBT-2 RPWM (Pin 13)
//                               (Centrifugal blower forward drive; LPWM unused / held LOW)
//   - Pin A1 (enPin):           Driver Enable output driving IBT-2 R_EN and L_EN (Pin A1 as digital)
//                               (HIGH = H-bridge active, LOW = H-bridge disabled; Pins 12/52/53 also driven as mirrors)
//   - Pin A2 (vccPin):          Dedicated 5V logic supply output for IBT-2 VCC (Pin A2 as digital, constant HIGH)
//   - Pin A0 (currentSensePin): Analog input measuring IBT-2 IS current-sense output
//                               (R_IS / L_IS connected to Pin A0)
//   - Pin 9  (servoPin):        180-degree RC Servo output controlling acoustic airflow manifold throttle valve
//   - Pin A3 (servoPotPin):     Analog input for B10K Pot 1 (Manifold Throttle Valve: 0..180 degrees)
//   - Pin A4 (tunePotPin):      Analog input for B10K Pot 2 (Fine Tune pitch adjust) -> affects motor only
//   - Pin 7  (pullup5vPin):     Dedicated 5V reference output on Pin 7 (adjacent to Pin 8 for clean pull-up resistor connection)
//   - Pin 8  (switchPin):       Physical ON-ON toggle switch wired between Pin 8 & GND
//                               (Uses internal pull-up resistor: LOW = ON, HIGH = OFF)
//
// Standalone Operation Guarantee:
//   - When Pin 8 switch is ON (Pin 8 connected to GND / LOW):
//       * Pin A2 (vccPin) provides steady 5V logic power to IBT-2 buffer chip.
//       * Pin A1 (enPin) is driven HIGH (IBT-2 BTS7960 output stage ENABLED).
//       * Pin 13 (pwmPin) is driven at currentPwmPercent (nominal 75.0% on boot).
//       * The centrifugal organ blower runs reliably and continuously EVEN IF:
//           - No USB cable is connected (standalone physical organ performance)
//           - The laptop microphone tuner is OFF or disabled
//           - The OpenOrgel desktop application is not open or running
//   - When Pin 8 switch is OFF (HIGH):
//       * Pin A1 (enPin) is driven LOW (IBT-2 BTS7960 disabled to prevent power draw).
//       * Pin 13 (pwmPin) is set to 0.
//       * currentPwmPercent resets to 75.0% nominal ready for the next startup.
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
//       * 'R' or 'r'               -> Reset blower PWM to nominal 75.0%
//       * "PWM:<0..100>\n"         -> Direct PID / manual PWM duty cycle target (%)
//       * "A4_1" / "A4_0"          -> Safely ignored (no-op)
//   - Enforces a 500 ms anti-oscillation settling delay between step adjustments
//     so the blower impeller and windchest reservoir stabilize before the next step.
// ============================================================================

#include <Servo.h>

const int pwmPin = 13;          // PWM output to IBT-2 RPWM (Pin 13)
const int enPin = A1;           // Primary Driver Enable output to IBT-2 R_EN and L_EN (Pin A1 as digital)
const int vccPin = A2;          // Dedicated 5V logic supply output for IBT-2 VCC (Pin A2 as digital, constant HIGH)
const int enPinAlt = 12;        // Redundant Driver Enable output (Pin 12)
const int enPinAlt2 = 52;       // Redundant Driver Enable output (Pin 52)
const int enPinAlt3 = 53;       // Redundant Driver Enable output (Pin 53)
const int currentSensePin = A0; // Analog input for IBT-2 IS current sense (Pin A0)
const int servoPin = 9;         // 180-degree RC Servo output controlling airflow manifold throttle valve (Pin 9)
const int servoPotPin = A3;     // B10K Pot 1 Wiper: Manifold throttle valve servo angle (0..180 deg)
const int tunePotPin = A4;      // B10K Pot 2 Wiper: Fine tune pitch adjust (affects motor only)
const int pullup5vPin = 7;      // Dedicated 5V reference output on Pin 7 (adjacent to Pin 8 for clean pull-up resistor connection)
const int switchPin = 8;        // ON-ON toggle switch (Pin 8 to GND, active LOW)

Servo manifoldServo;            // 180-degree RC Servo controlling windchest intake throttle valve
float smoothedServoPot = 512.0; // Low-pass filtered ADC reading for servo pot
int currentServoAngle = 90;     // Commanded servo angle (0 to 180 degrees)
int lastReportedServoAngle = -1;// Change-detection tracker for serial telemetry broadcast

// Helper: drives Pin A1, Pin 12, Pin 52, and Pin 53 HIGH (enabled) or LOW (disabled)
void setDriverEnable(bool enable) {
  digitalWrite(enPin, enable ? HIGH : LOW);
  digitalWrite(enPinAlt, enable ? HIGH : LOW);
  digitalWrite(enPinAlt2, enable ? HIGH : LOW);
  digitalWrite(enPinAlt3, enable ? HIGH : LOW);
}

float currentPwmPercent = 75.0;            // Nominal starting pressure at 75.0% power
unsigned long lastAdjustmentTime = 0;      // Timestamp (ms) of the last pitch adjustment
const unsigned long adjustmentDelay = 500; // 500 ms settling delay to prevent oscillation

bool debouncedSwitchState = false;         // Stable, filtered switch state
int lastRawReading = HIGH;                 // Previous raw pin read for edge detection
unsigned long lastDebounceTime = 0;        // Timestamp (ms) of the last raw pin change
const unsigned long debounceDelayMs = 50;  // 50 ms stability window to reject EMI / contact bounce
unsigned long lastTelemetryTime = 0;       // Periodic telemetry stream timestamp
float smoothedCurrentAmps = 0.0;           // Low-pass filtered motor current in Amperes
float fineTuneTrimPwm = 0.0;               // Fine tune motor trim (-10.0% to +10.0%)
float effectiveMotorPwm = 75.0;            // Combined PWM written to motor
String lineBuffer = "";                    // Line accumulator for serial commands

// Helper: converts 0.0-100.0% duty cycle to 8-bit timer value (0-255) and writes to Pin 13
void applyBlowerPwm(float percent) {
  int pwmCounts = (int)((percent / 100.0) * 255.0 + 0.5);
  if (pwmCounts < 0) pwmCounts = 0;
  if (pwmCounts > 255) pwmCounts = 255;
  analogWrite(pwmPin, pwmCounts);
}

// Reads B10K Pot 1 (Manifold Servo on A3) and Pot 2 (Fine Tune on A4)
void readPotentiometers() {
  // 1. Read Pot 1 (Manifold Throttle Valve Servo on Pin A3): 0..1023 -> 0..180 deg
  int rawServo = analogRead(servoPotPin);
  // Low-pass EMA filter (alpha = 0.20) to suppress wiper electrical noise and eliminate servo gear chatter
  smoothedServoPot = 0.20 * (float)rawServo + 0.80 * smoothedServoPot;
  int targetAngle = (int)map((long)(smoothedServoPot + 0.5), 0, 1023, 0, 180);
  if (targetAngle < 0) targetAngle = 0;
  if (targetAngle > 180) targetAngle = 180;

  if (targetAngle != currentServoAngle) {
    currentServoAngle = targetAngle;
    manifoldServo.write(currentServoAngle);
  }

  // 2. Read Pot 2 (Fine Tune Pitch on Pin A4): 0..1023 (Center ~512)
  int rawTune = analogRead(tunePotPin);
  // Center deadband (490..534) gives a solid neutral 0.0% trim at center knob position
  if (rawTune < 490) {
    // Trims flatter: 0.0% down to -10.0% PWM
    fineTuneTrimPwm = ((float)rawTune - 490.0) / 490.0 * 10.0;
  } else if (rawTune > 534) {
    // Trims sharper: 0.0% up to +10.0% PWM
    fineTuneTrimPwm = ((float)rawTune - 534.0) / (1023.0 - 534.0) * 10.0;
  } else {
    fineTuneTrimPwm = 0.0;
  }

  // 3. Compute combined effective blower motor PWM:
  // Base nominal pressure setpoint (75.0%) + manual fine tune trim (-10.0% to +10.0%)
  effectiveMotorPwm = currentPwmPercent + fineTuneTrimPwm;
  if (effectiveMotorPwm < 0.0) effectiveMotorPwm = 0.0;
  if (effectiveMotorPwm > 100.0) effectiveMotorPwm = 100.0;
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

// Transmits live PWM duty cycle, motor current in Amperes, and manifold servo angle back to OpenOrgel GUI
void reportTelemetry() {
  Serial.print("PWM:");
  Serial.println(effectiveMotorPwm, 1);
  Serial.print("CURRENT:");
  Serial.println(smoothedCurrentAmps, 2);

  // Broadcast manifold throttle valve servo angle if changed by >= 1 degree
  if (currentServoAngle != lastReportedServoAngle) {
    lastReportedServoAngle = currentServoAngle;
    Serial.print("SERVO:");
    Serial.println(currentServoAngle);
  }
}

// Processes a single step command ('F' = Flat -> +0.5%, 'S' = Sharp -> -0.5%, 'R' = Reset 75%)
void processStepCommand(char command) {
  unsigned long now = millis();

  if (command == 'R' || command == 'r') {
    currentPwmPercent = 75.0;
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

// Processes a newline-terminated command string (e.g. "F", "S", "PWM:52.5", "SERVO:90", "A4_1")
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

  // Direct Servo angle command ("SERVO:90" or "VALVE:45")
  if (cmd.startsWith("SERVO:") || cmd.startsWith("VALVE:")) {
    int colonIdx = cmd.indexOf(':');
    int target = cmd.substring(colonIdx + 1).toInt();
    if (target < 0) target = 0;
    if (target > 180) target = 180;
    currentServoAngle = target;
    manifoldServo.write(currentServoAngle);
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
  pinMode(switchPin, INPUT_PULLUP); // ON-ON toggle switch between Pin 8 & GND
  pinMode(currentSensePin, INPUT);
  pinMode(servoPotPin, INPUT);
  pinMode(tunePotPin, INPUT);

  // Attach 180-degree RC Servo on Pin 9
  manifoldServo.attach(servoPin);

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

  // Initialize servo position to Pot 1 reading
  readPotentiometers();
  manifoldServo.write(currentServoAngle);

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
        readPotentiometers();
        applyBlowerPwm(effectiveMotorPwm);
      } else {
        // Switch flipped OFF: Disable IBT-2 driver immediately
        setDriverEnable(false);
        analogWrite(pwmPin, 0);
        currentPwmPercent = 75.0; // Reset to 75.0% nominal ready for next startup
        smoothedCurrentAmps = 0.0;
      }

      // Immediately report switch transition event to OpenOrgel
      Serial.print("BLOWER:");
      Serial.println(debouncedSwitchState ? 1 : 0);
      if (debouncedSwitchState) {
        reportTelemetry();
      }
    }
  }

  // Keep Pot 1 (Manifold Servo) responsive at all times so user can position/test the valve even when blower is OFF
  readPotentiometers();

  bool isSystemOn = debouncedSwitchState;

  // When system is OFF: keep outputs disabled, drain serial buffer, and broadcast OFF heartbeat
  if (!isSystemOn) {
    setDriverEnable(false);
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
      Serial.print("SERVO:");
      Serial.println(currentServoAngle);
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
