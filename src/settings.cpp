#include "settings.h"

#include <algorithm>
#include <array>
#include <chrono>
#include <cstdio>
#include <filesystem>
#include <format>
#include <fstream>
#include <memory>
#include <print>
#include <system_error>

const std::vector<SettingsEntry>& get_settings() {
    static const std::vector<SettingsEntry> settings = {
        {1, "Power Off",   "poweroff",  false, SettingKind::Command},
        {2, "Reboot",      "reboot",    false, SettingKind::Command},
        {3, "IP Info",     "ip a",      false, SettingKind::Command},
        {4, "Input Test",  "evtest",    true,  SettingKind::Command},
        {5, "Volume",      "",          false, SettingKind::Volume},
        {6, "Brightness",  "",          false, SettingKind::Brightness},
    };
    return settings;
}

bool is_interactive(uint32_t id) {
    const auto& settings = get_settings();
    auto it = std::ranges::find_if(settings, [id](const auto& s) { return s.id == id; });
    return it != settings.end() && it->interactive;
}

bool is_ui_only(uint32_t id) {
    const auto& settings = get_settings();
    auto it = std::ranges::find_if(settings, [id](const auto& s) { return s.id == id; });
    return it != settings.end() && it->kind != SettingKind::Command;
}

std::string_view find_command(uint32_t id) {
    const auto& settings = get_settings();
    auto it = std::ranges::find_if(settings, [id](const auto& s) { return s.id == id; });
    return it != settings.end() ? it->exec : std::string_view{};
}

std::string execute_command(uint32_t id) {
    const auto& settings = get_settings();
    auto it = std::ranges::find_if(settings, [id](const auto& s) { return s.id == id; });
    if (it == settings.end() || it->kind != SettingKind::Command) {
        return "";
    }

    auto cmd = std::format("sh -c \"{}\" 2>&1", it->exec);
    std::unique_ptr<FILE, decltype([](FILE* f) { pclose(f); })> pipe(popen(cmd.c_str(), "r"));
    if (!pipe) {
        return "(command failed)";
    }

    std::string result;
    std::array<char, 4096> buffer{};
    while (fgets(buffer.data(), static_cast<int>(buffer.size()), pipe.get())) {
        result += buffer.data();
    }
    return result;
}

BatteryWatcher::~BatteryWatcher() {
    thread_.request_stop();
}

void BatteryWatcher::start(std::function<void(int32_t)> callback) {
    thread_ = std::jthread([callback = std::move(callback)](std::stop_token stop_token) {
        std::string capacity_path;
        std::error_code ec;
        for (const auto& entry : std::filesystem::directory_iterator("/sys/class/power_supply", ec)) {
            auto path = entry.path() / "capacity";
            if (std::filesystem::exists(path)) {
                capacity_path = path.string();
                break;
            }
        }

        if (capacity_path.empty()) {
            return;
        }

        while (!stop_token.stop_requested()) {
            std::ifstream file;
            file.open(capacity_path.c_str());
            if (file.is_open()) {
                int32_t capacity = 0;
                file >> capacity;
                if (capacity >= 0 && capacity <= 100) {
                    callback(capacity);
                }
            }
            std::this_thread::sleep_for(std::chrono::seconds(10));
        }
    });
}
