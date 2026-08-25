#include "launch.h"

#include <cerrno>
#include <cstdio>
#include <filesystem>
#include <format>
#include <fstream>
#include <iostream>
#include <print>
#include <string>

#include <fcntl.h>
#include <unistd.h>

#include "unique_fd.h"

static std::string game_pid_file() {
    const char* env = std::getenv("ARKANA_GAME_PIDFILE");
    if (env && env[0]) {
        return std::string(env);
    }
    return "/run/arkana-game.pid";
}

bool launch_game(const std::string& exec) {
    pid_t launcher_pid = getpid();

    auto script = std::format("while kill -0 {} 2>/dev/null; do sleep 0.2; done\n{}",
                              launcher_pid, exec);

    pid_t child = fork();
    if (child < 0) {
        std::println(std::cerr, "launch_game: fork failed: {}", std::format("errno {}", errno));
        return false;
    }

    if (child == 0) {
        setsid();
        int devnull_fd = open("/dev/null", O_RDWR);
        UniqueFd devnull(devnull_fd);
        if (devnull.valid()) {
            dup2(devnull.get(), STDIN_FILENO);
            dup2(devnull.get(), STDOUT_FILENO);
            dup2(devnull.get(), STDERR_FILENO);
        }
        execlp("sh", "sh", "-c", script.c_str(), nullptr);
        _exit(127);
    }

    std::string pidfile = game_pid_file();
    std::ofstream pf;
    pf.open(pidfile);
    if (pf.is_open()) {
        pf << child;
    }

    return true;
}
