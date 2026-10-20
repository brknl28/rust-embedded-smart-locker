#include "../locker_uno/controller.h"
#include "../locker_uno/i2c_protocol.h"
#include <stdio.h>

#define CHECK(condition) do { if (!(condition)) { \
    fprintf(stderr, "FAIL line %d: %s\n", __LINE__, #condition); return 1; \
} } while (0)

int main() {
    locker::Controller state;
    CHECK(state.mask() == 0 && !state.anyOpen());
    CHECK(state.set(1, true) && state.isOpen(1) && !state.isOpen(2));
    CHECK(state.set(2, true) && state.mask() == 3);
    CHECK(state.set(1, false) && state.anyOpen() && state.mask() == 2);
    CHECK(state.set(2, false) && !state.anyOpen());
    CHECK(state.set(2, true) && state.set(2, true) && state.mask() == 2);
    CHECK(!state.set(0, true) && !state.set(3, false) && state.mask() == 2);
    CHECK(!state.isOpen(0) && !state.isOpen(255));

    CHECK(locker::parse("OPEN 1").action == locker::Action::Open);
    CHECK(locker::parse("OPEN 2").id == 2);
    CHECK(locker::parse("CLOSE 1").action == locker::Action::Close);
    CHECK(locker::parse("CLOSE 2").id == 2);
    CHECK(locker::parse("STATUS").action == locker::Action::Status);
    CHECK(locker::parse("HELP").action == locker::Action::Help);
    const char* invalid[] = {"", "OPEN 0", "OPEN 3", "OPEN 12", "OPEN 1 junk", "open 1", " STATUS", "STATUS "};
    for (const char* line : invalid) CHECK(locker::parse(line).action == locker::Action::Invalid);

    locker::LineReader reader;
    for (char ch : "OPEN 1") {
        if (ch) CHECK(reader.feed(static_cast<uint8_t>(ch)) == locker::LineEvent::None);
    }
    CHECK(reader.feed('\r') == locker::LineEvent::Ready);
    CHECK(strcmp(reader.text(), "OPEN 1") == 0);
    CHECK(reader.feed('\n') == locker::LineEvent::None); // CRLF executes once.
    for (int i = 0; i < 23; ++i) CHECK(reader.feed('X') == locker::LineEvent::None);
    CHECK(reader.feed('\n') == locker::LineEvent::Ready); // Exact storage boundary.
    for (int i = 0; i < 24; ++i) CHECK(reader.feed('X') == locker::LineEvent::None);
    CHECK(reader.feed('\n') == locker::LineEvent::TooLong);
    CHECK(reader.feed('H') == locker::LineEvent::None);
    CHECK(reader.feed('\n') == locker::LineEvent::Ready && strcmp(reader.text(), "H") == 0);
    CHECK(reader.feed(0) == locker::LineEvent::None);
    CHECK(reader.feed('A') == locker::LineEvent::None);
    CHECK(reader.feed('\n') == locker::LineEvent::InvalidInput);
    CHECK(reader.feed(0xff) == locker::LineEvent::None);
    CHECK(reader.feed('\n') == locker::LineEvent::InvalidInput);
    CHECK(reader.feed('\n') == locker::LineEvent::None);
    locker::Controller busState;
    CHECK(locker::executeI2c(busState, 1, 1, 2) == locker::Ok && busState.mask() == 1);
    CHECK(locker::executeI2c(busState, 1, 2, 2) == locker::Ok && busState.mask() == 3);
    CHECK(locker::executeI2c(busState, 2, 1, 2) == locker::Ok && busState.mask() == 2);
    CHECK(locker::executeI2c(busState, 3, 0, 2) == locker::Ok && busState.mask() == 2);
    CHECK(locker::executeI2c(busState, 0, 0, 2) == locker::BadOpcode);
    CHECK(locker::executeI2c(busState, 255, 0, 2) == locker::BadOpcode);
    CHECK(locker::executeI2c(busState, 1, 0, 2) == locker::BadId);
    CHECK(locker::executeI2c(busState, 2, 3, 2) == locker::BadId);
    CHECK(locker::executeI2c(busState, 3, 1, 2) == locker::BadId);
    CHECK(locker::executeI2c(busState, 2, 2, 0) == locker::BadLength);
    CHECK(busState.mask() == 2); // Malformed requests never change state.
    CHECK(locker::executeI2c(busState, 2, 2, 2) == locker::Ok && !busState.anyOpen());
    puts("PASS: serial parser, locker states, I2C operations and malformed request rejection");
    return 0;
}
