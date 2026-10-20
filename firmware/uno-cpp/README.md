# Uno Locker Peripheral

The Uno software is written in C++ for **ATmega328P / 16 MHz**, using the Arduino board target `arduino:avr:uno`. The open/closed states of two lockers are stored in software and shown by the onboard D13 LED. There are no physical locks or door sensors.

Both lockers start CLOSED after a reset. D13 is HIGH when at least one locker is OPEN and LOW when both are CLOSED. States are not saved to EEPROM. PIN validation runs on the Mega.

## Interfaces

USB serial uses **9600 baud, 8N1**. Commands are uppercase and end with LF, CR or CRLF. Replies include SIMULATED to identify the states as simulated.

| Commands | Action |
| --- | --- |
| OPEN 1 / OPEN 2 | Open the selected simulated locker |
| CLOSE 1 / CLOSE 2 | Close the selected simulated locker |
| STATUS | Print both current states |
| HELP | List commands |

Invalid commands and oversized lines do not change state. The application uses fixed-size storage instead of Arduino String or dynamic allocation.

I2C uses the `Wire` library at address **0x12**. Requests are handled in the main loop; interrupt callbacks only exchange data. USB commands and I2C use the same locker states. The request and response format is described in the [I2C protocol](../I2C_PROTOCOL.md).

## Build and host tests

With Arduino CLI and Arduino AVR Boards installed, run from the repository root:

```text
arduino-cli compile --fqbn arduino:avr:uno firmware/uno-cpp/locker_uno
```

For host logic tests, create `firmware/uno-cpp/build` and use an MSVC developer command prompt:

```text
cl /nologo /std:c++14 /W4 /WX /EHsc /Fe:firmware/uno-cpp/build/controller_tests.exe /Fo:firmware/uno-cpp/build/controller_tests.obj firmware/uno-cpp/tests/controller_tests.cpp
firmware/uno-cpp/build/controller_tests.exe
```

The build, host logic tests, USB serial commands and Mega–Uno I2C communication were verified. The D13 LED was physically observed turning on and off with the locker states.
