#pragma once

#include <cstdint>
#include <functional>
#include <string>
#include <string_view>
#include <thread>
#include <vector>

enum struct SettingKind : int32_t {
    Command = 0,
    Volume = 1,
    Brightness = 2,
};

struct SettingsEntry {
    uint32_t id;
    std::string_view title;
    std::string_view exec;
    bool interactive;
    SettingKind kind;
};

const std::vector<SettingsEntry>& get_settings();

bool is_interactive(uint32_t id);
bool is_ui_only(uint32_t id);
std::string_view find_command(uint32_t id);
std::string execute_command(uint32_t id);

struct BatteryWatcher {
    BatteryWatcher() = default;
    ~BatteryWatcher();

    BatteryWatcher(const BatteryWatcher&) = delete;
    BatteryWatcher& operator=(const BatteryWatcher&) = delete;

    void start(std::function<void(int32_t)> callback);

private:
    std::jthread thread_;
};
