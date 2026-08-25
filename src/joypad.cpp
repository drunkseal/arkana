#include "joypad.h"

#include <array>
#include <print>
#include <string>

#include <fcntl.h>
#include <linux/input.h>
#include <poll.h>
#include <unistd.h>

#include "unique_fd.h"

constexpr float STICK_THRESHOLD = 0.5f;

enum struct Dir { None, Left, Right, Up, Down };

static Dir dir_from_x(float value) {
    if (value < -STICK_THRESHOLD) return Dir::Left;
    if (value > STICK_THRESHOLD) return Dir::Right;
    return Dir::None;
}

static Dir dir_from_y(float value) {
    if (value < -STICK_THRESHOLD) return Dir::Up;
    if (value > STICK_THRESHOLD) return Dir::Down;
    return Dir::None;
}

static std::string_view key_name_for_dir(Dir d) {
    switch (d) {
        case Dir::Left:  return "ArrowLeft";
        case Dir::Right: return "ArrowRight";
        case Dir::Up:    return "ArrowUp";
        case Dir::Down:  return "ArrowDown";
        default:         return {};
    }
}

static void send_key_press(slint::ComponentWeakHandle<MainWindow> weak_window, std::string_view key) {
    std::string key_str(key);
    slint::invoke_from_event_loop([weak_window, key_str]() {
        if (auto w = weak_window.lock()) {
            (*w)->window().dispatch_key_press_event(slint::SharedString(key_str));
        }
    });
}

static void send_key_release(slint::ComponentWeakHandle<MainWindow> weak_window, std::string_view key) {
    std::string key_str(key);
    slint::invoke_from_event_loop([weak_window, key_str]() {
        if (auto w = weak_window.lock()) {
            (*w)->window().dispatch_key_release_event(slint::SharedString(key_str));
        }
    });
}

static void send_key(slint::ComponentWeakHandle<MainWindow> weak_window, std::string_view key, bool pressed) {
    if (pressed) {
        send_key_press(weak_window, key);
    } else {
        send_key_release(weak_window, key);
    }
}

static std::string_view button_to_key(uint16_t code) {
    switch (code) {
        case BTN_DPAD_UP:    return "ArrowUp";
        case BTN_DPAD_DOWN:  return "ArrowDown";
        case BTN_DPAD_LEFT:  return "ArrowLeft";
        case BTN_DPAD_RIGHT: return "ArrowRight";
        case BTN_SOUTH:      return "Return";
        case BTN_EAST:       return "Backspace";
        case BTN_START:      return "Space";
        case BTN_SELECT:     return "Menu";
        default:             return {};
    }
}

static void handle_stick(slint::ComponentWeakHandle<MainWindow> weak_window,
                         Dir& current, Dir new_dir) {
    if (current == new_dir) return;

    Dir release_dir = Dir::None;
    Dir press_dir = Dir::None;

    if (new_dir == Dir::None) {
        release_dir = current;
    } else if (current == Dir::None) {
        press_dir = new_dir;
    } else {
        release_dir = current;
        press_dir = new_dir;
    }

    if (release_dir != Dir::None) {
        auto name = key_name_for_dir(release_dir);
        if (!name.empty()) send_key(weak_window, name, false);
    }
    if (press_dir != Dir::None) {
        auto name = key_name_for_dir(press_dir);
        if (!name.empty()) send_key(weak_window, name, true);
    }

    current = new_dir;
}

JoypadManager::~JoypadManager() {
    stop();
}

void JoypadManager::stop() {
    thread_.request_stop();
}

void JoypadManager::start(slint::ComponentWeakHandle<MainWindow> weak_window,
                           std::shared_ptr<TerminalManager> terminal) {
    thread_ = std::jthread([weak_window, terminal](std::stop_token stop_token) {
        UniqueFd gamepad_fd;

        for (int i = 0; i < 32 && !stop_token.stop_requested(); i++) {
            std::string path = "/dev/input/event" + std::to_string(i);
            int raw_fd = open(path.c_str(), O_RDONLY | O_NONBLOCK);
            if (raw_fd < 0) continue;

            std::array<char, 256> name{};
            ioctl(raw_fd, EVIOCGNAME(name.size()), name.data());

            unsigned long bits[KEY_MAX / 8 / sizeof(unsigned long) + 1] = {};
            if (ioctl(raw_fd, EVIOCGBIT(EV_KEY, sizeof(bits)), bits) >= 0) {
                if (bits[BTN_DPAD_UP / 8 / sizeof(unsigned long)] &
                    (1UL << ((BTN_DPAD_UP / 8) % sizeof(unsigned long)))) {
                    gamepad_fd.reset(raw_fd);
                    break;
                }
            }
            ::close(raw_fd);
        }

        if (!gamepad_fd.valid()) {
            std::println(std::cerr, "joypad: no gamepad found");
            return;
        }

        std::println(std::cerr, "joypad: device opened");

        Dir stick_x = Dir::None;
        Dir stick_y = Dir::None;
        bool l2 = false;
        bool r2 = false;

        struct pollfd pfd = {};
        pfd.fd = gamepad_fd.get();
        pfd.events = POLLIN;

        struct input_event ev;
        while (!stop_token.stop_requested()) {
            int ret = poll(&pfd, 1, 100);
            if (ret <= 0) continue;
            if (!(pfd.revents & POLLIN)) continue;

            ssize_t n = ::read(gamepad_fd.get(), &ev, sizeof(ev));
            if (n != sizeof(ev)) break;

            if (ev.type == EV_KEY) {
                if (ev.code == BTN_TL2) {
                    l2 = (ev.value != 0);
                } else if (ev.code == BTN_TR2) {
                    r2 = (ev.value != 0);
                } else {
                    auto key = button_to_key(ev.code);
                    if (!key.empty()) {
                        send_key(weak_window, key, ev.value != 0);
                    }
                }
            } else if (ev.type == EV_ABS) {
                if (ev.code == ABS_X) {
                    float value = ev.value / 32767.0f;
                    Dir new_dir = dir_from_x(value);
                    handle_stick(weak_window, stick_x, new_dir);
                } else if (ev.code == ABS_Y) {
                    float value = ev.value / 32767.0f;
                    Dir new_dir = dir_from_y(value);
                    handle_stick(weak_window, stick_y, new_dir);
                }
            }

            if (l2 && r2 && terminal->is_running()) {
                terminal->terminate(weak_window);
                l2 = false;
                r2 = false;
            }
        }
    });
}
