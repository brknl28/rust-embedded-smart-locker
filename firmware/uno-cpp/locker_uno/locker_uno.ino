#include "controller.h"
#include "i2c_protocol.h"
#include <Wire.h>
#include <util/atomic.h>

#if !defined(ARDUINO_AVR_UNO) || !defined(__AVR_ATmega328P__)
#error "This sketch requires Arduino Uno / ATmega328P."
#endif

locker::Controller controller;
locker::LineReader input;
volatile bool requestPending = false;
volatile uint8_t requestOpcode = 0, requestId = 0, requestLength = 0;
volatile uint8_t responseResult = locker::Ok, responseMask = 0;

// Wire callbacks run in interrupt context. Only copy bytes; never print here.
void receiveI2c(int length) {
    uint8_t bytes[2] = {0, 0};
    uint8_t position = 0;
    while (Wire.available()) {
        const uint8_t byte = static_cast<uint8_t>(Wire.read());
        if (position < 2) bytes[position] = byte;
        ++position;
    }
    if (requestPending) return; // One outstanding command; keep the first.
    requestOpcode = bytes[0];
    requestId = bytes[1];
    requestLength = length == 2 ? 2 : 0;
    responseResult = locker::Busy;
    requestPending = true;
}

void sendI2c() {
    const uint8_t response[2] = {responseResult, responseMask};
    Wire.write(response, sizeof(response)); // Copies into Wire's transmit buffer.
}

void processI2c() {
    if (!requestPending) return;
    const uint8_t result = locker::executeI2c(controller, requestOpcode, requestId, requestLength);
    updateLed();
    ATOMIC_BLOCK(ATOMIC_RESTORESTATE) {
        responseMask = controller.mask();
        responseResult = result;
        requestPending = false;
    }
}

void refreshSnapshot() {
    ATOMIC_BLOCK(ATOMIC_RESTORESTATE) { responseMask = controller.mask(); }
}

void updateLed() {
    digitalWrite(LED_BUILTIN, controller.anyOpen() ? HIGH : LOW);
}

void printStatus() {
    Serial.print(F("SIMULATED STATUS locker1="));
    Serial.print(controller.isOpen(1) ? F("OPEN") : F("CLOSED"));
    Serial.print(F(" locker2="));
    Serial.println(controller.isOpen(2) ? F("OPEN") : F("CLOSED"));
}

void handleCommand(const char* line) {
    const locker::Command command = locker::parse(line);
    switch (command.action) {
    case locker::Action::Open:
    case locker::Action::Close:
        controller.set(command.id, command.action == locker::Action::Open);
        updateLed();
        refreshSnapshot();
        printStatus();
        break;
    case locker::Action::Status:
        printStatus();
        break;
    case locker::Action::Help:
        Serial.println(F("SIMULATION ONLY: OPEN 1 | OPEN 2 | CLOSE 1 | CLOSE 2 | STATUS | HELP"));
        break;
    case locker::Action::Invalid:
        Serial.println(F("ERROR invalid command; use HELP (simulation only)"));
        break;
    }
}

void setup() {
    pinMode(LED_BUILTIN, OUTPUT);
    updateLed(); // Both software lockers start closed after reset.
    Serial.begin(9600);
    Wire.begin(locker::I2C_ADDRESS);
    Wire.onReceive(receiveI2c);
    Wire.onRequest(sendI2c);
    Serial.println(F("UNO SIMULATED LOCKERS ready; 9600 baud; use HELP"));
    printStatus();
}

void loop() {
    processI2c();
    // Fixed input storage; no String, heap allocation, delay or read-until timeout.
    for (uint8_t count = 0; count < 16 && Serial.available() > 0; ++count) {
        const locker::LineEvent event = input.feed(static_cast<uint8_t>(Serial.read()));
        if (event == locker::LineEvent::Ready) handleCommand(input.text());
        else if (event == locker::LineEvent::TooLong)
            Serial.println(F("ERROR line too long; discarded (simulation only)"));
        else if (event == locker::LineEvent::InvalidInput)
            Serial.println(F("ERROR non-printable input; discarded (simulation only)"));
    }
}
