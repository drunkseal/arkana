#pragma once

#include <cstdint>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <thread>
#include <vector>

#include <slint.h>
#include "main_window.slint.h"

#include "entries.h"

constexpr int COVER_WINDOW = 2;
constexpr uint32_t COVER_MAX_DIM = 640;

struct CoverState : public std::enable_shared_from_this<CoverState> {
    CoverState(const CoverState&) = delete;
    CoverState& operator=(const CoverState&) = delete;

    ~CoverState();

    static std::shared_ptr<slint::VectorModel<GameViewData>> build_model(
        const std::vector<GameEntry>& games);

    static std::shared_ptr<CoverState> init(
        const std::vector<GameEntry>& games,
        std::shared_ptr<slint::VectorModel<GameViewData>> model);

    void set_current(size_t index);

private:
    CoverState(const std::vector<GameEntry>& games,
               std::shared_ptr<slint::VectorModel<GameViewData>> model);

    void refresh();
    void drain_results();
    void apply_cover(size_t index, const std::optional<slint::Image>& image);

    std::vector<GameEntry> games_;
    std::shared_ptr<slint::VectorModel<GameViewData>> model_;
    std::vector<std::optional<slint::Image>> cache_;
    std::vector<bool> loading_;
    size_t current_;

    mutable std::mutex result_mutex_;
    struct DecodeResult {
        size_t index;
        std::optional<slint::Image> image;
    };
    std::vector<DecodeResult> pending_results_;

    std::mutex threads_mutex_;
    std::vector<std::jthread> loading_threads_;

    slint::Timer drain_timer_;
};

slint::Color hsl_to_rgb(float hue, float saturation, float lightness);
std::pair<slint::Color, slint::Color> placeholder_colors(const std::string& name);
size_t circular_distance(size_t a, size_t b, size_t n);
