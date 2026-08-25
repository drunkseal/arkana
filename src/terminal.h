#pragma once

#include <atomic>
#include <cstdint>
#include <functional>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include <slint.h>
#include "main_window.slint.h"

constexpr int GRID_COLS = 80;
constexpr int GRID_ROWS = 22;

struct Screen {
    Screen();

    void feed(const uint8_t* data, size_t len);
    std::string as_text() const;

private:
    void step(uint8_t b);
    void csi_exec(const uint8_t* params, size_t len, uint8_t final_byte);
    void put(uint8_t b);
    void newline();
    void scroll_up();
    void erase_line(int mode);
    void erase_screen(int mode);

    static size_t idx(int row, int col) {
        return row * GRID_COLS + col;
    }

    uint8_t cells_[GRID_COLS * GRID_ROWS];
    int row_;
    int col_;
    bool esc_;
    std::vector<uint8_t> csi_params_;
};

struct TerminalManager {
    TerminalManager();
    ~TerminalManager();

    TerminalManager(const TerminalManager&) = delete;
    TerminalManager& operator=(const TerminalManager&) = delete;

    bool is_running() const;
    void spawn(const std::string& exec, int32_t setting_id,
               slint::ComponentWeakHandle<MainWindow> weak_window);
    void write(const std::string& text);
    void terminate(slint::ComponentWeakHandle<MainWindow> weak_window);
    void terminate_all();

private:
    void finish(slint::ComponentWeakHandle<MainWindow> weak_window);

    struct Session {
        int master_fd;
        pid_t child_pid;
        pid_t process_group;
    };

    mutable std::mutex mutex_;
    std::unique_ptr<Session> session_;
    std::thread reader_thread_;
};
