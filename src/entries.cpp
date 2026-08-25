#include "entries.h"

#include <fstream>
#include <iterator>
#include <sstream>

namespace fs = std::filesystem;

std::vector<GameEntry> load_games(const std::vector<fs::path>& dirs) {
    std::vector<GameEntry> games;
    uint32_t next_id = 0;

    for (const auto& dir : dirs) {
        std::error_code ec;
        for (const auto& entry : fs::directory_iterator(dir, ec)) {
            if (entry.is_regular_file() && entry.path().extension() == ".toml") {
                auto game = parse_game(entry.path());
                if (game && !game->exec.empty()) {
                    game->id = next_id++;
                    games.push_back(std::move(*game));
                }
            }
        }
    }

    if (games.empty()) {
        games.push_back(GameEntry{
            .id = 0,
            .name = "Howdy!",
            .exec = "",
            .cover = std::nullopt,
        });
    }

    return games;
}

std::optional<GameEntry> parse_game(const fs::path& path) {
    std::ifstream file;
    file.open(path.c_str());
    if (!file.is_open()) {
        return std::nullopt;
    }

    std::string contents((std::istreambuf_iterator<char>(file)),
                          std::istreambuf_iterator<char>());
    return parse_game_contents(contents);
}

std::optional<GameEntry> parse_game_contents(const std::string& contents) {
    std::optional<std::string> name;
    std::optional<std::string> exec;
    std::optional<std::string> cover;

    std::istringstream stream(contents);
    std::string line;

    while (std::getline(stream, line)) {
        auto start = line.find_first_not_of(" \t\r\n");
        if (start == std::string::npos) continue;
        auto end = line.find_last_not_of(" \t\r\n");
        line = line.substr(start, end - start + 1);

        if (line.empty() || line[0] == '#') continue;
        if (line[0] == '[') continue;

        auto eq = line.find('=');
        if (eq == std::string::npos) continue;

        std::string key = line.substr(0, eq);
        auto key_start = key.find_first_not_of(" \t");
        auto key_end = key.find_last_not_of(" \t");
        if (key_start == std::string::npos) continue;
        key = key.substr(key_start, key_end - key_start + 1);

        std::string value = line.substr(eq + 1);
        auto val_start = value.find_first_not_of(" \t");
        if (val_start == std::string::npos) continue;
        value = value.substr(val_start);
        auto val_end = value.find_last_not_of(" \t");
        if (val_end != std::string::npos) {
            value = value.substr(0, val_end + 1);
        }
        if (value.size() >= 2 && value.front() == '"' && value.back() == '"') {
            value = value.substr(1, value.size() - 2);
        }

        if (key == "name") {
            name = value;
        } else if (key == "exec") {
            exec = value;
        } else if (key == "cover") {
            cover = value;
        }
    }

    if (!name || !exec) {
        return std::nullopt;
    }

    return GameEntry{
        .id = 0,
        .name = std::move(*name),
        .exec = std::move(*exec),
        .cover = std::move(cover),
    };
}
