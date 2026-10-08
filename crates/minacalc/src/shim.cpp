// C ABI over the vendored MinaCalc (ADR 0022). No C++ exception may cross into Rust.

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <vector>

// MinaCalcHelpers.h uses pow/erfc without including <cmath> itself.
#include "MinaCalc.h"
#include "MinaCalcHelpers.h"

// The Rust side passes exactly 8 floats and the length never crosses the ABI; a re-vendor that adds a skillset must fail here, not overflow.
static_assert(NUM_Skillset == 8, "wolluf_minacalc::Skillsets is [f32; 8]");

namespace {

constexpr int WL_OK = 0;
constexpr int WL_EXCEPTION = 1;
constexpr int WL_REJECTED = 2;
constexpr int WL_BAD_ARGS = 3;

auto
run(Calc* calc,
	const uint32_t* notes,
	const float* times,
	size_t n,
	float rate,
	float goal,
	bool ssr,
	unsigned keycount,
	float* out) noexcept -> int
{
	// Upstream returns all zeros for fewer than two rows instead of rating them.
	if (calc == nullptr || notes == nullptr || times == nullptr ||
		out == nullptr || n < 2) {
		return WL_BAD_ARGS;
	}
	try {
		std::vector<NoteInfo> rows(n);
		for (size_t i = 0; i < n; ++i) {
			rows[i] = NoteInfo{ notes[i], times[i] };
		}
		calc->loadparams = false;
		std::vector<float> values;
		if (ssr) {
			values = MinaSDCalc(rows, rate, goal, keycount, calc);
		} else {
			// Same settings as the all-rates MinaSDCalc overload, for one rate.
			calc->ssr = false;
			calc->debugmode = false;
			calc->keycount = keycount;
			values = calc->CalcMain(rows, rate, default_score_goal);
		}
		if (values.size() != NUM_Skillset) {
			return WL_EXCEPTION;
		}
		// Upstream signals a skipped file (joke density, too long, bad mask)
		// only through the all-zero output.
		bool any_positive = false;
		for (const auto v : values) {
			if (!std::isfinite(v)) {
				return WL_REJECTED;
			}
			any_positive = any_positive || v > 0.F;
		}
		if (!any_positive) {
			return WL_REJECTED;
		}
		for (size_t i = 0; i < NUM_Skillset; ++i) {
			out[i] = values[i];
		}
		return WL_OK;
	} catch (...) {
		return WL_EXCEPTION;
	}
}

} // namespace

extern "C" {

auto
wl_calc_new() noexcept -> Calc*
{
	try {
		return new Calc();
	} catch (...) {
		return nullptr;
	}
}

void
wl_calc_free(Calc* calc) noexcept
{
	delete calc;
}

auto
wl_calc_version() noexcept -> int
{
	try {
		return GetCalcVersion();
	} catch (...) {
		return -1;
	}
}

auto
wl_msd(Calc* calc,
	   const uint32_t* notes,
	   const float* times,
	   size_t n,
	   float rate,
	   unsigned keycount,
	   float* out) noexcept -> int
{
	return run(calc, notes, times, n, rate, default_score_goal, false, keycount, out);
}

auto
wl_ssr(Calc* calc,
	   const uint32_t* notes,
	   const float* times,
	   size_t n,
	   float rate,
	   float goal,
	   unsigned keycount,
	   float* out) noexcept -> int
{
	return run(calc, notes, times, n, rate, goal, true, keycount, out);
}

} // extern "C"
