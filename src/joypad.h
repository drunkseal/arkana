#pragma once

#include <memory>
#include <thread>

#include <slint.h>
#include "main_window.slint.h"
#include "terminal.h"

struct JoypadManager {
    JoypadManager() = default;
    ~JoypadManager();

    JoypadManager(const JoypadManager&) = delete;
    JoypadManager& operator=(const JoypadManager&) = delete;

    void start(slint::ComponentWeakHandle<MainWindow> weak_window,
               std::shared_ptr<TerminalManager> terminal);
    void stop();

private:
    std::jthread thread_;
};
