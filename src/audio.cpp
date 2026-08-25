#include "audio.h"
#include <print>

#ifdef ARKANA_HAS_ALSA

#include <cmath>
#include <iostream>
#include <memory>
#include <mutex>
#include <numbers>
#include <vector>

#include "nothrow.h"

#include <alsa/asoundlib.h>

constexpr int SAMPLE_RATE = 44100;
constexpr int CHANNELS = 2;
constexpr int CLICK_DURATION_MS = 30;
constexpr float CLICK_FREQ = 800.0f;
constexpr float CLICK_VOLUME = 0.4f;

struct NavAudio {
    snd_pcm_t* pcm;
    std::vector<int16_t> click_buffer;
    std::mutex mutex;
};

static std::unique_ptr<NavAudio> g_audio;

static std::vector<int16_t> generate_click() {
    int num_samples = SAMPLE_RATE * CLICK_DURATION_MS / 1000;
    std::vector<int16_t> buf(num_samples * CHANNELS);

    for (int i = 0; i < num_samples; i++) {
        float t = static_cast<float>(i) / SAMPLE_RATE;
        float envelope = 1.0f - static_cast<float>(i) / num_samples;
        envelope = envelope * envelope;
        float sample = std::sin(2.0f * std::numbers::pi_v<float> * CLICK_FREQ * t) * CLICK_VOLUME * envelope;
        int16_t s = static_cast<int16_t>(sample * 32767.0f);
        buf[i * 2] = s;
        buf[i * 2 + 1] = s;
    }
    return buf;
}

void audio_init() {
    snd_pcm_t* pcm = nullptr;
    int err = snd_pcm_open(&pcm, "default", SND_PCM_STREAM_PLAYBACK, 0);
    if (err < 0) {
        std::println(std::cerr, "audio init error: {}", snd_strerror(err));
        return;
    }

    snd_pcm_set_params(pcm,
                       SND_PCM_FORMAT_S16_LE,
                       SND_PCM_ACCESS_RW_INTERLEAVED,
                       CHANNELS,
                       SAMPLE_RATE,
                       1,
                       100000);

    auto nav = make_nothrow<NavAudio>();
    nav->pcm = pcm;
    nav->click_buffer = generate_click();
    g_audio = std::move(nav);

    std::println(std::cerr, "audio: initialized");
}

void audio_play() {
    if (!g_audio) return;

    std::lock_guard<std::mutex> lock(g_audio->mutex);
    snd_pcm_t* pcm = g_audio->pcm;
    snd_pcm_prepare(pcm);
    const auto& buf = g_audio->click_buffer;
    snd_pcm_writei(pcm, buf.data(), buf.size() / CHANNELS);
}

#else

void audio_init() {
    std::println(std::cerr, "audio: disabled (no ALSA)");
}

void audio_play() {}

#endif
