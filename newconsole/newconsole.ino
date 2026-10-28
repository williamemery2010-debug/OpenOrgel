const int pwmPin = 3;      // PWM output to the MOSFET/motor controller
const int switchPin = 8;   // ON-ON toggle switch

float currentPwmPercent = 50.0; // Start with nominal pressure at 50%
unsigned long lastAdjustmentTime = 0;
const unsigned long adjustmentDelay = 500; // Delay in milliseconds to prevent oscillation

void setup() {
  Serial.begin(115200); // High baud rate for fast communication from your software
  
  pinMode(pwmPin, OUTPUT);
  // Using internal pull-up resistor. Wire the switch between Pin 8 and GND.
  pinMode(switchPin, INPUT_PULLUP); 
}

void loop() {
  // Read switch state (LOW means the switch is connecting Pin 8 to GND)
  bool isSystemOn = (digitalRead(switchPin) == LOW);

  if (!isSystemOn) {
    analogWrite(pwmPin, 0);       // Stop the blower
    currentPwmPercent = 50.0;     // Reset to nominal pressure for the next startup
    return;                       // Skip pitch adjustments while off
  }

  // If system is on, apply the current PWM value
  analogWrite(pwmPin, (currentPwmPercent / 100.0) * 255);

  // Check for incoming data from your custom software
  if (Serial.available() > 0) {
    char command = Serial.read();

    // Only allow adjustments if the delay period has passed
    if (millis() - lastAdjustmentTime >= adjustmentDelay) {
      
      if (command == 'F' || command == 'f') { 
        // Pitch is flat -> increase pressure
        currentPwmPercent += 0.5;
        if (currentPwmPercent > 100.0) currentPwmPercent = 100.0;
        
        lastAdjustmentTime = millis();
        
      } else if (command == 'S' || command == 's') { 
        // Pitch is sharp -> decrease pressure
        currentPwmPercent -= 0.5;
        if (currentPwmPercent < 0.0) currentPwmPercent = 0.0;
        
        lastAdjustmentTime = millis();
      }
      // If the pitch is in tune, the software can just send nothing, 
      // and the currentPwmPercent will be maintained.
    }
  }
}