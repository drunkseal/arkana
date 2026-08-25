#include <cstdlib>
#include <filesystem>
#include <memory>
#include <mutex>
#include <ranges>
#include <string>
#include <string_view>
#include <vector>

#include <slint.h>
#include "main_window.slint.h"

#include "entries.h"
#include "covers.h"
#include "joypad.h"
#include "launch.h"
#include "nothrow.h"
#include "settings.h"
#include "terminal.h"
#include "audio.h"

struct Pending {
    std::string exec;
};

static std::mutex g_pending_mutex;
static std::unique_ptr<Pending> g_pending;

static slint::SharedString terminal_key_to_bytes(const slint::SharedString& text) {
    std::string_view s = text;
    if (s == "ArrowUp") return slint::SharedString("\x1b[A");
    if (s == "ArrowDown") return slint::SharedString("\x1b[B");
    if (s == "ArrowRight") return slint::SharedString("\x1b[C");
    if (s == "ArrowLeft") return slint::SharedString("\x1b[D");
    if (s == "\r") return slint::SharedString("\r");
    if (s == "\x7f" || s == "\x08") return slint::SharedString("\x7f");
    return text;
}

int main() {
    auto main_window = MainWindow::create();
    auto terminal_manager = adopt_nothrow(new(std::nothrow) TerminalManager());

    audio_init();

    slint::ComponentWeakHandle<MainWindow> weak_window(main_window);

    main_window->on_settings_run_command(
        [terminal_manager, weak_window](int32_t command_id) {
            uint32_t id = static_cast<uint32_t>(command_id);
            if (is_ui_only(id)) return;

            if (is_interactive(id)) {
                auto cmd = find_command(id);
                if (!cmd.empty()) {
                    terminal_manager->spawn(std::string(cmd), command_id, weak_window);
                }
            } else if (auto w = weak_window.lock()) {
                std::string output = execute_command(id);
                (*w)->set_settings_command_output(slint::SharedString(output));
            }
        });

    main_window->on_terminal_key([terminal_manager](slint::SharedString text) {
        auto bytes = terminal_key_to_bytes(text);
        terminal_manager->write(std::string(bytes));
    });

    main_window->on_terminal_exit([terminal_manager, weak_window]() {
        terminal_manager->terminate(weak_window);
    });

    main_window->on_play_navigation_sound([]() { audio_play(); });

    BatteryWatcher battery_watcher;
    battery_watcher.start([weak_window](int32_t level) {
        slint::invoke_from_event_loop([weak_window, level]() {
            if (auto w = weak_window.lock()) {
                (*w)->set_battery_level(level);
            }
        });
    });

    std::vector<std::filesystem::path> game_dirs;
    const char* env = std::getenv("GAME_DIRS");
    if (env && env[0]) {
        std::string_view dirs_str(env);
        for (auto dir : std::views::split(dirs_str, ':')) {
            if (!dir.empty()) {
                game_dirs.emplace_back(dir.begin(), dir.end());
            }
        }
    }

    auto games = load_games(game_dirs);

    const auto& settings = get_settings();
    auto settings_model = std::shared_ptr<slint::VectorModel<SettingData>>(new(std::nothrow) slint::VectorModel<SettingData>());
    for (const auto& s : settings) {
        SettingData sd;
        sd.setting_id = static_cast<int32_t>(s.id);
        sd.title = slint::SharedString(s.title);
        sd.kind = static_cast<int32_t>(s.kind);
        settings_model->push_back(sd);
    }
    main_window->set_settings_data_array(std::static_pointer_cast<slint::Model<SettingData>>(settings_model));

    main_window->on_launch_game([games](int32_t index) {
        if (index >= 0 && static_cast<size_t>(index) < games.size()) {
            const auto& game = games[index];
            if (!game.exec.empty()) {
                std::lock_guard<std::mutex> lock(g_pending_mutex);
                g_pending = make_nothrow<Pending>(Pending{game.exec});
                slint::quit_event_loop();
            }
        }
    });

    auto cover_model = CoverState::build_model(games);
    auto cover_state = CoverState::init(games, cover_model);
    main_window->set_game_data_array(std::static_pointer_cast<slint::Model<GameViewData>>(cover_model));

    main_window->on_current_game_changed([cover_state](int32_t index) {
        if (index >= 0) {
            cover_state->set_current(static_cast<size_t>(index));
        }
    });

    JoypadManager joypad;
    joypad.start(weak_window, terminal_manager);

    main_window->run();

    joypad.stop();
    terminal_manager->terminate_all();

    {
        std::lock_guard<std::mutex> lock(g_pending_mutex);
        if (g_pending) {
            launch_game(g_pending->exec);
        }
    }

    return 0;
}
