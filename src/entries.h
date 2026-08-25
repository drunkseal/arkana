#pragma once

#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>
#include <vector>

struct GameEntry {
    uint32_t id;
    std::string name;
    std::string exec;
    std::optional<std::string> cover;
};

std::vector<GameEntry> load_games(const std::vector<std::filesystem::path>& dirs);
std::optional<GameEntry> parse_game(const std::filesystem::path& path);
std::optional<GameEntry> parse_game_contents(const std::string& contents);
