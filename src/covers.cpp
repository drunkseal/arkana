#include "covers.h"

#include <algorithm>
#include <cmath>
#include <numbers>
#include <thread>

#include "nothrow.h"

// --- Color utilities ---

slint::Color hsl_to_rgb(float hue, float saturation, float lightness) {
    float chroma = (1.0f - std::abs(2.0f * lightness - 1.0f)) * saturation;
    float hp = hue / 60.0f;
    float x = chroma * (1.0f - std::abs(std::fmod(hp, 2.0f) - 1.0f));

    float r1, g1, b1;
    uint32_t sector = static_cast<uint32_t>(hp) % 6;
    switch (sector) {
        case 0: r1 = chroma; g1 = x; b1 = 0; break;
        case 1: r1 = x; g1 = chroma; b1 = 0; break;
        case 2: r1 = 0; g1 = chroma; b1 = x; break;
        case 3: r1 = 0; g1 = x; b1 = chroma; break;
        case 4: r1 = x; g1 = 0; b1 = chroma; break;
        default: r1 = chroma; g1 = 0; b1 = x; break;
    }

    float m = lightness - chroma / 2.0f;
    uint8_t r = static_cast<uint8_t>(std::round((r1 + m) * 255.0f));
    uint8_t g = static_cast<uint8_t>(std::round((g1 + m) * 255.0f));
    uint8_t b = static_cast<uint8_t>(std::round((b1 + m) * 255.0f));

    return slint::Color::from_rgb_uint8(r, g, b);
}

std::pair<slint::Color, slint::Color> placeholder_colors(const std::string& name) {
    constexpr float SATURATION = 0.55f;
    constexpr float TOP_LIGHTNESS = 0.30f;
    constexpr float BOTTOM_LIGHTNESS = 0.08f;

    uint32_t hash = 0;
    for (uint8_t c : name) {
        hash = hash * 31 + c;
    }
    uint32_t hue = hash % 360;

    return {
        hsl_to_rgb(static_cast<float>(hue), SATURATION, TOP_LIGHTNESS),
        hsl_to_rgb(static_cast<float>(hue), SATURATION, BOTTOM_LIGHTNESS),
    };
}

size_t circular_distance(size_t a, size_t b, size_t n) {
    if (n == 0) return SIZE_MAX;
    size_t forward = (a + n - b) % n;
    return std::min(forward, n - forward);
}

// --- Helper to create GameViewData ---

static GameViewData make_view_data(const GameEntry& game, const slint::Image& cover_art) {
    auto [c1, c2] = placeholder_colors(game.name);

    std::string initial;
    if (!game.name.empty()) {
        char c = static_cast<char>(std::toupper(static_cast<unsigned char>(game.name[0])));
        initial = std::string(1, c);
    }

    bool has_cover = cover_art.size().width > 0 && cover_art.size().height > 0;

    GameViewData vd;
    vd.game_id = static_cast<int32_t>(game.id);
    vd.title = slint::SharedString(game.name);
    vd.initial = slint::SharedString(initial);
    vd.cover_art = cover_art;
    vd.has_cover = has_cover;
    vd.c1 = c1;
    vd.c2 = c2;
    return vd;
}

// --- CoverState implementation ---

CoverState::CoverState(const std::vector<GameEntry>& games,
                       std::shared_ptr<slint::VectorModel<GameViewData>> model)
    : games_(games)
    , model_(model)
    , cache_(games.size())
    , loading_(games.size(), false)
    , current_(0) {
}

CoverState::~CoverState() {
    {
        std::lock_guard<std::mutex> lock(threads_mutex_);
        for (auto& t : loading_threads_) {
            t.request_stop();
        }
    }
    loading_threads_.clear();
}

std::shared_ptr<slint::VectorModel<GameViewData>> CoverState::build_model(
    const std::vector<GameEntry>& games) {
    auto model = std::shared_ptr<slint::VectorModel<GameViewData>>(new(std::nothrow) slint::VectorModel<GameViewData>());
    for (const auto& game : games) {
        model->push_back(make_view_data(game, slint::Image()));
    }
    return model;
}

std::shared_ptr<CoverState> CoverState::init(
    const std::vector<GameEntry>& games,
    std::shared_ptr<slint::VectorModel<GameViewData>> model) {
    auto state = std::shared_ptr<CoverState>(new(std::nothrow) CoverState(games, model));

    state->drain_timer_.start(slint::TimerMode::Repeated,
        std::chrono::milliseconds(16),
        [weak = std::weak_ptr<CoverState>(state)]() {
            if (auto s = weak.lock()) {
                s->drain_results();
            }
        });

    state->refresh();
    return state;
}

void CoverState::set_current(size_t index) {
    current_ = index;
    refresh();
}

void CoverState::refresh() {
    size_t n = games_.size();

    for (size_t i = 0; i < n; ++i) {
        bool in_window = (n <= 2 * COVER_WINDOW + 1) ||
                         (circular_distance(i, current_, n) <= COVER_WINDOW);

        if (in_window && !cache_[i] && !loading_[i]) {
            if (games_[i].cover) {
                loading_[i] = true;
                size_t idx = i;
                std::string path = *games_[i].cover;
                auto self = shared_from_this();

                std::lock_guard<std::mutex> lock(threads_mutex_);
                loading_threads_.emplace_back([self, idx, path](std::stop_token) {
                    std::optional<slint::Image> result;

                    auto image = slint::Image::load_from_path(slint::SharedString(path));
                    if (image.size().width > 0 && image.size().height > 0) {
                        result = image;
                    }

                    std::lock_guard<std::mutex> lock(self->result_mutex_);
                    self->pending_results_.push_back({idx, result});
                });
            } else {
                cache_[i] = slint::Image();
            }
        } else if (!in_window && cache_[i]) {
            cache_[i] = std::nullopt;
            loading_[i] = false;
            model_->set_row_data(i, make_view_data(games_[i], slint::Image()));
        }
    }
}

void CoverState::drain_results() {
    std::vector<DecodeResult> results;
    {
        std::lock_guard<std::mutex> lock(result_mutex_);
        results = std::move(pending_results_);
    }

    for (auto& [index, image] : results) {
        apply_cover(index, image);
    }
}

void CoverState::apply_cover(size_t index, const std::optional<slint::Image>& image) {
    size_t n = games_.size();
    if (index >= loading_.size()) return;

    loading_[index] = false;

    bool in_window = (n <= 2 * COVER_WINDOW + 1) ||
                     (circular_distance(index, current_, n) <= COVER_WINDOW);
    if (!in_window) return;
    if (cache_[index]) return;

    cache_[index] = image.value_or(slint::Image());
    model_->set_row_data(index, make_view_data(games_[index], *cache_[index]));
}
