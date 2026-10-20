#pragma once

#include <stdint.h>
#include <string.h>

namespace locker {

enum class Action : uint8_t { Open, Close, Status, Help, Invalid };
struct Command { Action action; uint8_t id; };

inline Command parse(const char* line) {
    if (strcmp(line, "OPEN 1") == 0) return {Action::Open, 1};
    if (strcmp(line, "OPEN 2") == 0) return {Action::Open, 2};
    if (strcmp(line, "CLOSE 1") == 0) return {Action::Close, 1};
    if (strcmp(line, "CLOSE 2") == 0) return {Action::Close, 2};
    if (strcmp(line, "STATUS") == 0) return {Action::Status, 0};
    if (strcmp(line, "HELP") == 0) return {Action::Help, 0};
    return {Action::Invalid, 0};
}

class Controller {
public:
    Controller() : mask_(0) {}
    bool set(uint8_t id, bool open) {
        if (id < 1 || id > 2) return false;
        const uint8_t bit = static_cast<uint8_t>(1u << (id - 1));
        if (open) mask_ |= bit;
        else mask_ &= static_cast<uint8_t>(~bit);
        return true;
    }
    bool isOpen(uint8_t id) const {
        return id >= 1 && id <= 2 && (mask_ & (1u << (id - 1))) != 0;
    }
    bool anyOpen() const { return mask_ != 0; }
    uint8_t mask() const { return mask_; }
private:
    uint8_t mask_;
};

enum class LineEvent : uint8_t { None, Ready, TooLong, InvalidInput };

class LineReader {
public:
    LineReader() : used_(0), dropped_(LineEvent::None) { text_[0] = '\0'; }
    LineEvent feed(uint8_t byte) {
        if (byte == '\r' || byte == '\n') {
            text_[used_] = '\0';
            const LineEvent result = dropped_ != LineEvent::None
                ? dropped_ : (used_ ? LineEvent::Ready : LineEvent::None);
            used_ = 0;
            dropped_ = LineEvent::None;
            return result;
        }
        if (dropped_ != LineEvent::None) return LineEvent::None;
        if (byte < 32 || byte > 126) {
            dropped_ = LineEvent::InvalidInput;
            return LineEvent::None;
        }
        if (used_ == sizeof(text_) - 1) {
            dropped_ = LineEvent::TooLong;
            return LineEvent::None;
        }
        text_[used_++] = static_cast<char>(byte);
        return LineEvent::None;
    }
    const char* text() const { return text_; }
private:
    char text_[24]; // 23 printable characters plus the NUL terminator.
    uint8_t used_;
    LineEvent dropped_;
};

} // namespace locker
