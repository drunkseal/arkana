#pragma once

#include <unistd.h>
#include <utility>

struct UniqueFd {
    UniqueFd() : fd_(-1) {}
    explicit UniqueFd(int fd) : fd_(fd) {}

    ~UniqueFd() { close(); }

    UniqueFd(const UniqueFd&) = delete;
    UniqueFd& operator=(const UniqueFd&) = delete;

    UniqueFd(UniqueFd&& other) noexcept : fd_(other.fd_) { other.fd_ = -1; }
    UniqueFd& operator=(UniqueFd&& other) noexcept {
        if (this != &other) {
            close();
            fd_ = other.fd_;
            other.fd_ = -1;
        }
        return *this;
    }

    [[nodiscard]] int get() const { return fd_; }
    [[nodiscard]] int release() { int fd = fd_; fd_ = -1; return fd; }
    [[nodiscard]] bool valid() const { return fd_ >= 0; }

    void close() {
        if (fd_ >= 0) {
            ::close(fd_);
            fd_ = -1;
        }
    }

    void reset(int fd = -1) {
        close();
        fd_ = fd;
    }

private:
    int fd_;
};
