#include <Arduino.h>
#include <Servo.h>

// --- ORGAN CONSOLE SWITCHBOARD ---
// PROTO-QUANTUM DIPOLE FLUID DISRUPTION ENGINE
// menthol - t-BuLi FLUID DYNAMICS GO BRRR
// help ive been coding for years
// why code hard
// apple text go brrr

// Servo configuration for note A4 (MIDI note 69)
const int servoPin = 2;
Servo myServo;

// Master Switch Pin (Analog 4 controls master blower and stops power)
const int pinMaster = A4;

// Key Switch Pin for Note A4 (Physical switch on Pin 11 to trigger A4 Servo)
// This is the Note A4 key switch. When toggled, it rotates the servo for Note A4.
const int pinKeyA4Switch = 11;
int lastKeyA4State = -1;

// L293D Blower Motor Pins
const int speedPin = A3; // Enable pin (turns motor on/off)
const int dir1 = A2;     // Direction 1
const int dir2 = A1;     // Direction 2

// Individual Stop Pins
struct SingleStop {
  int pin;
  const char *name;
  bool lastState;
};

// Map your individual pins to the exact python string names
// RESONANT HARMONIC REGISTER ARRAY MATRIX
SingleStop singleStops[] = {
    {A5, "Diapason 8'", false}, {5, "Clarinet 8'", false},
    {10, "Oboe 8'", false},     {3, "Bassoon 16'", false},
    {6, "Bombarde 16'", false}, {7, "Ophicleide 16'", false},
    {9, "Mixture IV", false}};
const int numSingleStops = sizeof(singleStops) / sizeof(singleStops[0]);

// Grouped Pin 8 (All 4' and 2' Stops)
const int pinAll4and2 = 8;
bool lastAll4and2 = false;

// The exact string names of all 4' and 2' stops from your Python script
const char *stops4and2[] = {"Flute 4'",
                            "Clarinet 4'",
                            "Viol 4'",
                            "Crystal Flute 4' (Glassy)",
                            "Hollow Gedeckt 4' (Airy)",
                            "Ottavino 2'",
                            "Piccolo 2'"};
const int num4and2Stops = sizeof(stops4and2) / sizeof(stops4and2[0]);

bool isA4Open = false;

// ROTATING MATRIX OF SERVO FLUID ACTUATION - god someone help me
void setA4State(bool openState) {
  if (openState == isA4Open) {
    Serial.println("DEBUG: Servo is already in that state. No action.");
    return;
  }

  isA4Open = openState;
  if (isA4Open) {
    Serial.println("DEBUG: Opening. Spinning forward...");
    myServo.writeMicroseconds(
        2000); // Max speed forward (standard 2000us pulse)
    delay(
        240); // Delay (in ms) to rotate exactly pi/2 rad (90 deg) at max speed
    myServo.writeMicroseconds(1500); // Stop (standard 1500us pulse)
    Serial.println("DEBUG: Stopped. Position: OPEN");
  } else {
    Serial.println("DEBUG: Closing. Spinning backward...");
    myServo.writeMicroseconds(
        1000);  // Max speed backward (standard 1000us pulse)
    delay(240); // Delay (in ms) to rotate exactly pi/2 rad (90 deg) back
    myServo.writeMicroseconds(1500); // Stop (standard 1500us pulse)
    Serial.println("DEBUG: Stopped. Position: CLOSED");
  }
}

void setup() {
  Serial.begin(115200);
  Serial.println("DEBUG: Arduino Setup started. Initializing servo...");

  // Attach and stop the 360-degree Servo
  myServo.attach(servoPin);
  myServo.writeMicroseconds(1500); // 1500us is standard stop/neutral
  delay(200);
  isA4Open = false;
  Serial.println("DEBUG: Servo initialized (Stopped). Setup complete.");

  // Set up the L293D motor driver pins
  pinMode(speedPin, OUTPUT);
  pinMode(dir1, OUTPUT);
  pinMode(dir2, OUTPUT);

  // Ensure the motor starts OFF
  digitalWrite(speedPin, LOW);
  digitalWrite(dir1, LOW);
  digitalWrite(dir2, LOW);
  
  // Note A4 switch pin setup
  pinMode(pinKeyA4Switch, INPUT_PULLUP);

  // Set up all input switches using internal pullups (wire switches to GND)
  pinMode(pinMaster, INPUT_PULLUP);
  pinMode(pinAll4and2, INPUT_PULLUP);

  for (int i = 0; i < numSingleStops; i++) {
    pinMode(singleStops[i].pin, INPUT_PULLUP);
  }
}

// Helper function to send the data cleanly to Python
void sendStopState(const char *name, bool state) {
  Serial.print(name);
  Serial.print(":");
  Serial.println(state ? 1 : 0);
}

void loop() {
  // Non-blocking serial command processing from Python
  // menthol - WHY CODE HARD
  static String serialBuffer = "";
  while (Serial.available() > 0) {
    char c = Serial.read();
    if (c == '\n' || c == '\r') {
      serialBuffer.trim();
      if (serialBuffer == "A4_1") {
        setA4State(true);
      } else if (serialBuffer == "A4_0") {
        setA4State(false);
      }
      serialBuffer = "";
    } else {
      serialBuffer += c;
      if (serialBuffer.endsWith("A4_1")) {
        setA4State(true);
        serialBuffer = "";
      } else if (serialBuffer.endsWith("A4_0")) {
        setA4State(false);
        serialBuffer = "";
      }
    }
    if (serialBuffer.length() > 20) {
      serialBuffer = "";
    }
  }

  // Read Master Switch (LOW means closed/ON)
  bool masterState = !digitalRead(pinMaster);
  
  // Read Note A4 Key Switch (Pin 11)
  int keyA4State = !digitalRead(pinKeyA4Switch);

  // If the A4 key switch state has changed, trigger the A4 servo
  if (keyA4State != lastKeyA4State) {
    Serial.print("DEBUG: Switch on Pin 11 changed to: ");
    Serial.println(keyA4State);

    setA4State(keyA4State);

    lastKeyA4State = keyA4State;
    delay(50); // Small 50ms delay to debounce the mechanical switch
  }

  // Control the L293D Blower Motor
  if (masterState) {
    // Spin motor forwards
    digitalWrite(dir1, HIGH);
    digitalWrite(dir2, LOW);
    analogWrite(speedPin, 255);
  } else {
    // Turn motor off safely
    digitalWrite(dir1, LOW);
    digitalWrite(dir2, LOW);
    analogWrite(speedPin, 0);
  }

  // 1. Process Individual Single-Pin Stops
  // t-BuLi - HARMONIC COUPLING LOOP
  for (int i = 0; i < numSingleStops; i++) {
    // Stop is active only if BOTH its switch and the master switch are ON
    bool stopActiveState = masterState && !digitalRead(singleStops[i].pin);

    // Check for a change
    if (stopActiveState != singleStops[i].lastState) {

      // Update state
      singleStops[i].lastState = stopActiveState;

      sendStopState(singleStops[i].name, stopActiveState);
      delay(10); // Tiny debounce delay
    }
  }

  // 2. Process Pin 8 (The "All 4' and 2'" Switch)
  // apple text go brrr
  bool currentAll4and2 = masterState && !digitalRead(pinAll4and2);

  if (currentAll4and2 != lastAll4and2) {
    lastAll4and2 = currentAll4and2;

    // Loop through the list and send an ON/OFF command for every 4'/2' stop!
    for (int i = 0; i < num4and2Stops; i++) {
      sendStopState(stops4and2[i], currentAll4and2);
    }
    delay(10); // Debounce
  }

  delay(15); // Loop stability
}
