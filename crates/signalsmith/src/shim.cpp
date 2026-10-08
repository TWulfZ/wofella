// C ABI over the vendored Signalsmith Stretch (ADR 0025). No C++ exception may cross into Rust.

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <memory>
#include <new>
#include <vector>

#include "signalsmith-stretch.h"

namespace {

constexpr int WL_OK = 0;
constexpr int WL_EXCEPTION = 1;
constexpr int WL_BAD_ARGS = 3;

constexpr int WL_PRESET_DEFAULT = 0;
constexpr int WL_PRESET_CHEAPER = 1;

// The default constructor seeds from std::random_device; a fixed seed makes renders repeatable.
constexpr long kSeed = 0x776f6c;

using Stretch = signalsmith::stretch::SignalsmithStretch<float>;

struct Handle {
	int channels;
	float sample_rate;
	int preset;
	std::unique_ptr<Stretch> stretch;
};

// Upstream accepts any `buffer[channel][index]` type, so interleaved data is read in place.
template<typename T>
struct Interleaved {
	T* data;
	int channels;

	struct Channel {
		T* first;
		int stride;
		T& operator[](int i) const { return first[static_cast<size_t>(i) * static_cast<size_t>(stride)]; }
	};
	Channel operator[](int c) const { return { data + c, channels }; }
};

void
configure(Handle& h)
{
	// A fresh object per render: the random engine (used past 2x stretch) never carries
	// state from one call into the next.
	h.stretch.reset(new Stretch(kSeed));
	if (h.preset == WL_PRESET_CHEAPER) {
		h.stretch->presetCheaper(h.channels, h.sample_rate);
	} else {
		h.stretch->presetDefault(h.channels, h.sample_rate);
	}
}

} // namespace

extern "C" {

void*
wl_stretch_new(int channels, float sample_rate, int preset) noexcept
{
	if (channels < 1 || !(sample_rate > 0.F) ||
		(preset != WL_PRESET_DEFAULT && preset != WL_PRESET_CHEAPER)) {
		return nullptr;
	}
	try {
		auto* h = new Handle{ channels, sample_rate, preset, nullptr };
		try {
			configure(*h);
		} catch (...) {
			delete h;
			return nullptr;
		}
		return h;
	} catch (...) {
		return nullptr;
	}
}

void
wl_stretch_free(void* handle) noexcept
{
	delete static_cast<Handle*>(handle);
}

// Offline stretch of `in_frames` interleaved frames into exactly `out_frames` frames,
// aligned to the input (upstream's `exact`: seek for pre-roll, flush for the tail).
// `transpose` multiplies every frequency; 1 keeps the pitch.
int
wl_stretch_process(void* handle,
				   const float* input,
				   size_t in_frames,
				   float* output,
				   size_t out_frames,
				   float transpose) noexcept
{
	auto* h = static_cast<Handle*>(handle);
	if (h == nullptr || input == nullptr || output == nullptr || in_frames == 0 ||
		out_frames == 0 || !(transpose > 0.F) || !std::isfinite(transpose)) {
		return WL_BAD_ARGS;
	}
	try {
		configure(*h);
		Stretch& s = *h->stretch;
		// A zero tonality limit scales every frequency, as resampling (osu!'s NC) does.
		s.setTransposeFactor(transpose);
		const size_t ch = static_cast<size_t>(h->channels);
		const double rate = static_cast<double>(in_frames) / static_cast<double>(out_frames);

		// `exact` refuses (and zeroes) inputs shorter than its seek length; padding with
		// silence keeps short clips audible, and the padded tail is cut below.
		const size_t seek = static_cast<size_t>(s.outputSeekLength(static_cast<float>(rate))) + 1;
		if (in_frames >= seek) {
			Interleaved<const float> in{ input, h->channels };
			Interleaved<float> out{ output, h->channels };
			if (!s.exact(in, static_cast<int>(in_frames), out, static_cast<int>(out_frames))) {
				return WL_EXCEPTION;
			}
			return WL_OK;
		}
		const size_t padded_in = seek;
		const size_t padded_out = static_cast<size_t>(std::llround(static_cast<double>(padded_in) / rate));
		std::vector<float> in_buf(padded_in * ch, 0.F);
		std::copy(input, input + in_frames * ch, in_buf.begin());
		std::vector<float> out_buf(std::max(padded_out, out_frames) * ch, 0.F);
		Interleaved<const float> in{ in_buf.data(), h->channels };
		Interleaved<float> out{ out_buf.data(), h->channels };
		if (!s.exact(in, static_cast<int>(padded_in), out, static_cast<int>(padded_out))) {
			return WL_EXCEPTION;
		}
		std::copy(out_buf.begin(), out_buf.begin() + static_cast<std::ptrdiff_t>(out_frames * ch), output);
		return WL_OK;
	} catch (...) {
		return WL_EXCEPTION;
	}
}

} // extern "C"
