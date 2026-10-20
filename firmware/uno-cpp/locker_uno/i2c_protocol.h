#pragma once
#include "controller.h"

namespace locker {
constexpr uint8_t I2C_ADDRESS = 0x12;
enum Result : uint8_t { Ok = 0, BadOpcode = 1, BadId = 2, BadLength = 3, Busy = 4 };

inline uint8_t executeI2c(Controller& controller, uint8_t opcode, uint8_t id, uint8_t length) {
    if (length != 2) return BadLength;
    if (opcode < 1 || opcode > 3) return BadOpcode;
    if ((opcode == 3 && id != 0) || (opcode != 3 && (id < 1 || id > 2))) return BadId;
    if (opcode != 3) controller.set(id, opcode == 1);
    return Ok;
}
} // namespace locker
