#pragma once

#include <memory>
#include <new>
#include <utility>

template<typename T, typename... Args>
std::unique_ptr<T> make_nothrow(Args&&... args) noexcept {
    return std::unique_ptr<T>(new(std::nothrow) T(std::forward<Args>(args)...));
}

template<typename T>
std::shared_ptr<T> make_shared_nothrow() noexcept {
    T* raw = new(std::nothrow) T();
    if (!raw) return {};
    return std::shared_ptr<T>(raw);
}

template<typename T>
std::shared_ptr<T> adopt_nothrow(T* raw) noexcept {
    if (!raw) return {};
    return std::shared_ptr<T>(raw);
}
