#include "terminal.h"

#include <algorithm>
#include <atomic>
#include <cerrno>
#include <cstring>
#include <format>
#include <memory>
#include <print>
#include <thread>

#include <signal.h>
#include <sys/ioctl.h>
#include <sys/wait.h>
#include <termios.h>
#include <unistd.h>
#include <utmp.h>

#include <pty.h>

#include "unique_fd.h"
#include "nothrow.h"

static std::string errno_str() {
    char buf[256];
    if (strerror_r(errno, buf, sizeof(buf)) == 0) {
        return std::string(buf);
    }
    return std::format("errno {}", errno);
}

Screen::Screen()
    : row_(0), col_(0), esc_(false) {
    std::ranges::fill(cells_, static_cast<uint8_t>(' '));
}

void Screen::feed(const uint8_t* data, size_t len) {
    for (size_t i = 0; i < len; ++i) {
        step(data[i]);
    }
}

void Screen::step(uint8_t b) {
    if (!csi_params_.empty()) {
        if (b >= 0x40 && b <= 0x7e) {
            auto params = std::move(csi_params_);
            csi_params_.clear();
            csi_exec(params.data(), params.size(), b);
        } else {
            csi_params_.push_back(b);
        }
        return;
    }

    if (esc_) {
        esc_ = false;
        if (b == '[') {
            csi_params_.clear();
        }
        return;
    }

    switch (b) {
        case 0x1b:
            esc_ = true;
            break;
        case '\r':
            col_ = 0;
            break;
        case '\n':
            newline();
            break;
        case '\x08':
            if (col_ > 0) col_--;
            break;
        case '\t':
            col_ = (col_ / 8 + 1) * 8;
            if (col_ >= GRID_COLS) col_ = GRID_COLS - 1;
            break;
        default:
            if (b >= 0x20) {
                put(b);
            }
            break;
    }
}

void Screen::put(uint8_t b) {
    if (row_ < GRID_ROWS) {
        cells_[idx(row_, col_)] = b;
    }
    col_++;
    if (col_ >= GRID_COLS) {
        col_ = 0;
        newline();
    }
}

void Screen::newline() {
    if (row_ + 1 >= GRID_ROWS) {
        scroll_up();
    } else {
        row_++;
    }
}

void Screen::scroll_up() {
    std::ranges::copy(cells_ + GRID_COLS, cells_ + GRID_COLS * GRID_ROWS, cells_);
    std::ranges::fill(cells_ + GRID_COLS * (GRID_ROWS - 1),
                      cells_ + GRID_COLS * GRID_ROWS,
                      static_cast<uint8_t>(' '));
}

void Screen::erase_line(int mode) {
    int row_start = row_ * GRID_COLS;
    switch (mode) {
        case 0:
            std::ranges::fill(cells_ + row_start + col_, cells_ + row_start + GRID_COLS, ' ');
            break;
        case 1:
            std::ranges::fill(cells_ + row_start, cells_ + row_start + col_ + 1, ' ');
            break;
        default:
            std::ranges::fill(cells_ + row_start, cells_ + row_start + GRID_COLS, ' ');
            break;
    }
}

void Screen::erase_screen(int mode) {
    switch (mode) {
        case 0:
            erase_line(0);
            for (int r = row_ + 1; r < GRID_ROWS; r++) {
                std::ranges::fill(cells_ + r * GRID_COLS,
                                  cells_ + (r + 1) * GRID_COLS,
                                  static_cast<uint8_t>(' '));
            }
            break;
        case 1:
            erase_line(1);
            for (int r = 0; r < row_; r++) {
                std::ranges::fill(cells_ + r * GRID_COLS,
                                  cells_ + (r + 1) * GRID_COLS,
                                  static_cast<uint8_t>(' '));
            }
            break;
        default:
            std::ranges::fill(cells_, cells_ + sizeof(cells_),
                              static_cast<uint8_t>(' '));
            break;
    }
}

static std::vector<int> parse_params(const uint8_t* data, size_t len) {
    std::vector<int> out;
    int cur = 0;
    bool has_cur = false;

    for (size_t i = 0; i < len; i++) {
        uint8_t b = data[i];
        if (b >= '0' && b <= '9') {
            cur = cur * 10 + (b - '0');
            has_cur = true;
        } else if (b == ';') {
            out.push_back(has_cur ? cur : 1);
            cur = 0;
            has_cur = false;
        }
    }

    if (has_cur) {
        out.push_back(cur);
    } else if (out.empty()) {
        out.push_back(1);
    }
    return out;
}

void Screen::csi_exec(const uint8_t* params, size_t len, uint8_t final_byte) {
    if (final_byte == 'm' || final_byte == 'h' || final_byte == 'l' ||
        final_byte == 'r' || final_byte == 's' || final_byte == 'u' ||
        final_byte == 't' || final_byte == 'Z') {
        return;
    }

    auto p = parse_params(params, len);
    auto get = [&](size_t i) -> int {
        return (i < p.size()) ? p[i] : 1;
    };

    switch (final_byte) {
        case 'H':
        case 'f':
            row_ = std::clamp(get(0) - 1, 0, GRID_ROWS - 1);
            col_ = std::clamp(get(1) - 1, 0, GRID_COLS - 1);
            break;
        case 'A':
            row_ = std::max(0, row_ - get(0));
            break;
        case 'B':
            row_ = std::min(GRID_ROWS - 1, row_ + get(0));
            break;
        case 'C':
            col_ = std::min(GRID_COLS - 1, col_ + get(0));
            break;
        case 'D':
            col_ = std::max(0, col_ - get(0));
            break;
        case 'G':
            col_ = std::clamp(get(0) - 1, 0, GRID_COLS - 1);
            break;
        case 'd':
            row_ = std::clamp(get(0) - 1, 0, GRID_ROWS - 1);
            break;
        case 'J':
            erase_screen(p.empty() ? 0 : p[0]);
            break;
        case 'K':
            erase_line(p.empty() ? 0 : p[0]);
            break;
        default:
            break;
    }
}

std::string Screen::as_text() const {
    std::string s;
    s.reserve(GRID_COLS * GRID_ROWS + GRID_ROWS);
    for (int r = 0; r < GRID_ROWS; r++) {
        if (r > 0) s.push_back('\n');
        const uint8_t* row_data = cells_ + r * GRID_COLS;
        s.append(reinterpret_cast<const char*>(row_data), GRID_COLS);
    }
    return s;
}

// --- TerminalManager implementation ---

TerminalManager::TerminalManager() = default;

TerminalManager::~TerminalManager() {
    terminate_all();
}

bool TerminalManager::is_running() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return session_ != nullptr;
}

void TerminalManager::spawn(const std::string& exec, int32_t setting_id,
                            slint::ComponentWeakHandle<MainWindow> weak_window) {
    std::lock_guard<std::mutex> lock(mutex_);

    if (session_) {
        std::println(std::cerr, "terminal already running");
        return;
    }

    int raw_master_fd = -1;
    pid_t child_pid = -1;

    struct winsize ws = {};
    ws.ws_row = GRID_ROWS;
    ws.ws_col = GRID_COLS;

    child_pid = forkpty(&raw_master_fd, nullptr, nullptr, &ws);
    if (child_pid < 0) {
        std::println(std::cerr, "forkpty error: {}", errno_str());
        slint::invoke_from_event_loop([weak_window, setting_id]() {
            if (auto w = weak_window.lock()) {
                (*w)->set_terminal_active(false);
                (*w)->set_terminal_id(0);
            }
        });
        return;
    }

    if (child_pid == 0) {
        setsid();
        execlp("sh", "sh", "-c", exec.c_str(), nullptr);
        std::println(std::cerr, "execlp: {}", errno_str());
        _exit(127);
    }

    UniqueFd master_fd(raw_master_fd);

    pid_t pgid = child_pid;
    setpgid(child_pid, pgid);

    session_ = make_nothrow<Session>(Session{
        .master_fd = master_fd.get(),
        .child_pid = child_pid,
        .process_group = pgid,
    });

    slint::invoke_from_event_loop([weak_window, setting_id]() {
        if (auto w = weak_window.lock()) {
            (*w)->set_terminal_active(true);
            (*w)->set_terminal_id(setting_id);
        }
    });

    int fd_for_reader = master_fd.get();
    reader_thread_ = std::thread([this, fd_for_reader, child_pid, weak_window]() {
        Screen screen;
        char buffer[4096];
        std::string last_text;

        while (true) {
            ssize_t n = ::read(fd_for_reader, buffer, sizeof(buffer));
            if (n <= 0) break;

            screen.feed(reinterpret_cast<uint8_t*>(buffer), n);
            std::string text = screen.as_text();
            if (text != last_text) {
                last_text = text;
                auto text_copy = text;
                slint::invoke_from_event_loop([weak_window, text_copy]() {
                    if (auto w = weak_window.lock()) {
                        (*w)->set_terminal_output(slint::SharedString(text_copy));
                    }
                });
            }
        }

        int status;
        waitpid(child_pid, &status, 0);
        std::println(std::cerr, "interactive session ended");

        slint::invoke_from_event_loop([weak_window]() {
            if (auto w = weak_window.lock()) {
                (*w)->set_terminal_active(false);
                (*w)->set_terminal_id(0);
                (*w)->set_terminal_output(slint::SharedString(""));
            }
        });
    });

    static_cast<void>(master_fd.release());
}

void TerminalManager::write(const std::string& text) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (session_) {
        ::write(session_->master_fd, text.data(), text.size());
        tcdrain(session_->master_fd);
    }
}

void TerminalManager::terminate(slint::ComponentWeakHandle<MainWindow> weak_window) {
    std::unique_ptr<Session> session;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        session = std::move(session_);
    }

    if (session) {
        std::println(std::cerr, "terminating interactive program");
        ::kill(-session->process_group, SIGTERM);
        std::this_thread::sleep_for(std::chrono::milliseconds(200));
        ::kill(-session->process_group, SIGKILL);
        ::close(session->master_fd);
    } else {
        std::println(std::cerr, "requested to terminate interactive program but none is running");
    }

    if (reader_thread_.joinable()) {
        reader_thread_.join();
    }

    finish(weak_window);
}

void TerminalManager::terminate_all() {
    std::unique_ptr<Session> session;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        session = std::move(session_);
    }

    if (session) {
        ::kill(-session->process_group, SIGKILL);
        ::close(session->master_fd);
    }

    if (reader_thread_.joinable()) {
        reader_thread_.join();
    }
}

void TerminalManager::finish(slint::ComponentWeakHandle<MainWindow> weak_window) {
    slint::invoke_from_event_loop([weak_window]() {
        if (auto w = weak_window.lock()) {
            (*w)->set_terminal_active(false);
            (*w)->set_terminal_id(0);
            (*w)->set_terminal_output(slint::SharedString(""));
        }
    });
}
