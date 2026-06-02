#include <Arduino.h>

// --- ORGAN CONSOLE SWITCHBOARD ---

// Master Switch
const int pinMaster = A4;

// L293D Blower Motor Pins
const int speedPin = A3; // Enable pin (turns motor on/off)
const int dir1 = A2;     // Direction 1
const int dir2 = A1;     // Direction 2

// Individual Stop Pins
struct SingleStop {
  int pin;
  const char* name;
  bool lastState;
};

// Map your individual pins to the exact python string names
SingleStop singleStops[] = {
  {A5, "Diapason 8'", false},
  {5,  "Clarinet 8'", false},
  {10, "Oboe 8'", false},
  {3,  "Bassoon 16'", false},
  {6,  "Bombarde 16'", false},
  {7,  "Ophicleide 16'", false},
  {9,  "Mixture IV", false}
};
const int numSingleStops = sizeof(singleStops) / sizeof(singleStops[0]);

// Grouped Pin 8 (All 4' and 2' Stops)
const int pinAll4and2 = 8;
bool lastAll4and2 = false;

// The exact string names of all 4' and 2' stops from your Python script
const char* stops4and2[] = {
  "Flute 4'", "Clarinet 4'", "Viol 4'", 
  "Crystal Flute 4' (Glassy)", "Gedeckt 4' (Hollow)", 
  "Recorder 4'", "Ottavino 2'", "Piccolo 2'"
};
const int num4and2Stops = sizeof(stops4and2) / sizeof(stops4and2[0]);


void setup() {
  Serial.begin(9600);
  
  // Set up the L293D motor driver pins
  pinMode(speedPin, OUTPUT);
  pinMode(dir1, OUTPUT);
  pinMode(dir2, OUTPUT);
  
  // Ensure the motor starts OFF
  digitalWrite(speedPin, LOW);
  digitalWrite(dir1, LOW);
  digitalWrite(dir2, LOW);

  // Set up all input switches using internal pullups (wire switches to GND)
  pinMode(pinMaster, INPUT_PULLUP);
  pinMode(pinAll4and2, INPUT_PULLUP);
  
  for (int i = 0; i < numSingleStops; i++) {
    pinMode(singleStops[i].pin, INPUT_PULLUP);
  }
}

// Helper function to send the data cleanly to Python
void sendStopState(const char* name, bool state) {
  Serial.print(name);
  Serial.print(":");
  Serial.println(state ? 1 : 0);
}

void loop() {
  // Read Master Switch (LOW means closed/ON)
  bool masterState = !digitalRead(pinMaster); 
  
  // Control the L293D Blower Motor
  if (masterState) {
    // Spin motor forwards and turn speed to max
    digitalWrite(dir1, HIGH);
    digitalWrite(dir2, LOW);
    digitalWrite(speedPin, HIGH); 
  } else {
    // Turn motor off safely
    digitalWrite(dir1, LOW);
    digitalWrite(dir2, LOW);
    digitalWrite(speedPin, LOW);
  }

  // 1. Process Individual Single-Pin Stops
  for (int i = 0; i < numSingleStops; i++) {
    // Stop is active only if BOTH its switch and the master switch are ON
    bool currentState = masterState && !digitalRead(singleStops[i].pin);
    
    if (currentState != singleStops[i].lastState) {
      singleStops[i].lastState = currentState;
      sendStopState(singleStops[i].name, currentState);
      delay(10); // Tiny debounce delay
    }
  }

  // 2. Process Pin 7 (The "All 4' and 2'" Switch)
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
