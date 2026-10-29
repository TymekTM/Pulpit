//! Tactile feedback on the Android client: View.performHapticFeedback
//! honours system haptics settings, falling back to VibrationEffect /
//! Vibrator with API-level guards down to minSdk 24 (SM-T561 Android 7.1).
//! Zero per-frame allocations during drag, pure rate limiter and
//! preference mapping.

package app.pulpit.mobile.ui

import android.content.Context
import android.os.Build
import android.os.SystemClock
import android.os.VibrationEffect
import android.os.Vibrator
import android.provider.Settings
import android.view.HapticFeedbackConstants
import android.view.View
import androidx.compose.runtime.staticCompositionLocalOf
import app.pulpit.mobile.R

/** Local intensity preference: Lekkie / Normalne / Mocne (resource
 *  backed; the label is display-only, [fromString] keeps parsing the
 *  persisted preference words in both languages). */
enum class HapticIntensity(val labelRes: Int) {
    Light(R.string.main_haptic_light),
    Normal(R.string.main_haptic_normal),
    Strong(R.string.main_haptic_strong);

    companion object {
        fun fromString(value: String?, default: HapticIntensity = Normal): HapticIntensity =
            when (value?.lowercase()?.trim()) {
                "light", "lekkie" -> Light
                "normal", "normalne" -> Normal
                "strong", "mocne" -> Strong
                else -> default
            }
    }
}

/** Local on-device tactile feedback setting. */
data class HapticConfig(
    val enabled: Boolean = true,
    val intensity: HapticIntensity = HapticIntensity.Normal,
) {
    companion object {
        const val PREF_KEY_ENABLED = "haptics_enabled"
        const val PREF_KEY_INTENSITY = "haptics_intensity"

        val DEFAULT = HapticConfig(enabled = true, intensity = HapticIntensity.Normal)

        fun fromPreferences(enabled: Boolean?, intensityStr: String?): HapticConfig =
            HapticConfig(
                enabled = enabled ?: true,
                intensity = HapticIntensity.fromString(intensityStr, HapticIntensity.Normal),
            )
    }
}

/** Distinct tactile feedback events described in the spec. */
enum class HapticKind {
    /** Short click on tile press-start. */
    Click,
    /** Distinct tick when a toggle flips. */
    ToggleFlip,
    /** Light tick while dragging a slider across a ~5% step. */
    SliderTick,
    /** Heavier pulse on recognised long-press / double-tap / swipe gestures. */
    Heavy,
}

interface Haptics {
    fun click()
    fun toggleFlip()
    fun sliderTick()
    fun heavy()
    fun perform(kind: HapticKind)
}

object NoOpHaptics : Haptics {
    override fun click() {}
    override fun toggleFlip() {}
    override fun sliderTick() {}
    override fun heavy() {}
    override fun perform(kind: HapticKind) {}
}

val LocalHaptics = staticCompositionLocalOf<Haptics> { NoOpHaptics }

/** Pure resolution of View.performHapticFeedback constant per kind and intensity. */
fun resolveHapticFeedbackConstant(kind: HapticKind, intensity: HapticIntensity): Int? =
    when (kind) {
        HapticKind.Click -> when (intensity) {
            HapticIntensity.Light -> HapticFeedbackConstants.CLOCK_TICK
            HapticIntensity.Normal -> HapticFeedbackConstants.KEYBOARD_TAP
            HapticIntensity.Strong -> HapticFeedbackConstants.VIRTUAL_KEY
        }
        HapticKind.ToggleFlip ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                HapticFeedbackConstants.CONFIRM
            } else {
                when (intensity) {
                    HapticIntensity.Light -> HapticFeedbackConstants.CLOCK_TICK
                    HapticIntensity.Normal -> HapticFeedbackConstants.KEYBOARD_TAP
                    HapticIntensity.Strong -> HapticFeedbackConstants.VIRTUAL_KEY
                }
            }
        HapticKind.SliderTick -> when (intensity) {
            HapticIntensity.Light, HapticIntensity.Normal -> HapticFeedbackConstants.CLOCK_TICK
            HapticIntensity.Strong -> HapticFeedbackConstants.KEYBOARD_TAP
        }
        HapticKind.Heavy -> HapticFeedbackConstants.LONG_PRESS
    }

/** Pure fallback parameters for Vibrator (duration in ms, amplitude 1..255). */
fun fallbackVibrationParams(kind: HapticKind, intensity: HapticIntensity): Pair<Long, Int> {
    val durationMs = when (kind) {
        HapticKind.SliderTick -> when (intensity) {
            HapticIntensity.Light -> 5L
            HapticIntensity.Normal -> 8L
            HapticIntensity.Strong -> 12L
        }
        HapticKind.Click -> when (intensity) {
            HapticIntensity.Light -> 8L
            HapticIntensity.Normal -> 12L
            HapticIntensity.Strong -> 20L
        }
        HapticKind.ToggleFlip -> when (intensity) {
            HapticIntensity.Light -> 12L
            HapticIntensity.Normal -> 18L
            HapticIntensity.Strong -> 28L
        }
        HapticKind.Heavy -> when (intensity) {
            HapticIntensity.Light -> 25L
            HapticIntensity.Normal -> 40L
            HapticIntensity.Strong -> 65L
        }
    }
    val amplitude = when (intensity) {
        HapticIntensity.Light -> 80
        HapticIntensity.Normal -> 150
        HapticIntensity.Strong -> 240
    }
    return Pair(durationMs, amplitude)
}

/** Check whether system-level haptic feedback is enabled in Android settings. */
@Suppress("DEPRECATION")
fun isSystemHapticFeedbackEnabled(context: Context): Boolean =
    runCatching {
        Settings.System.getInt(
            context.contentResolver,
            Settings.System.HAPTIC_FEEDBACK_ENABLED,
            1,
        ) != 0
    }.getOrDefault(true)

/** How long a read of the system haptics switch stays valid. A slider drag
 *  ticks up to ~20 times a second and each Settings.System read is a binder
 *  round trip; one read per second keeps the user's toggle effective within
 *  a second without a ContentObserver (AndroidHaptics has no dispose hook,
 *  and a leaked observer is worse than the occasional IPC). */
internal const val SYSTEM_HAPTICS_CACHE_MS = 1_000L

/** Production Haptics implementation honoring system settings and local preference. */
class AndroidHaptics(
    private val viewProvider: () -> View?,
    private val context: Context,
    private val configProvider: () -> HapticConfig,
) : Haptics {

    @Suppress("DEPRECATION")
    private val vibrator: Vibrator? by lazy {
        context.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
    }

    private var systemHapticsEnabled = true
    private var systemHapticsReadAtMs = 0L
    private var systemHapticsRead = false

    /** [isSystemHapticFeedbackEnabled] re-read at most once per
     *  [SYSTEM_HAPTICS_CACHE_MS]; perform() is called from the main thread. */
    private fun systemHapticsOn(): Boolean {
        val now = SystemClock.elapsedRealtime()
        if (!systemHapticsRead || now - systemHapticsReadAtMs >= SYSTEM_HAPTICS_CACHE_MS) {
            systemHapticsEnabled = isSystemHapticFeedbackEnabled(context)
            systemHapticsReadAtMs = now
            systemHapticsRead = true
        }
        return systemHapticsEnabled
    }

    override fun click() = perform(HapticKind.Click)
    override fun toggleFlip() = perform(HapticKind.ToggleFlip)
    override fun sliderTick() = perform(HapticKind.SliderTick)
    override fun heavy() = perform(HapticKind.Heavy)

    override fun perform(kind: HapticKind) {
        val config = configProvider()
        if (!config.enabled) return

        if (!systemHapticsOn()) return

        val view = viewProvider()
        val constant = resolveHapticFeedbackConstant(kind, config.intensity)
        var handled = false
        if (view != null && constant != null) {
            handled = view.performHapticFeedback(
                constant,
                HapticFeedbackConstants.FLAG_IGNORE_VIEW_SETTING,
            )
        }

        if (!handled) {
            performVibratorFallback(kind, config.intensity)
        }
    }

    private fun performVibratorFallback(kind: HapticKind, intensity: HapticIntensity) {
        val vib = vibrator ?: return
        if (!vib.hasVibrator()) return

        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val predefined = when (kind) {
                    HapticKind.Click -> VibrationEffect.EFFECT_CLICK
                    HapticKind.ToggleFlip -> VibrationEffect.EFFECT_DOUBLE_CLICK
                    HapticKind.SliderTick -> VibrationEffect.EFFECT_TICK
                    HapticKind.Heavy -> VibrationEffect.EFFECT_HEAVY_CLICK
                }
                try {
                    vib.vibrate(VibrationEffect.createPredefined(predefined))
                    return
                } catch (_: Exception) {
                    // Predefined effect unsupported by device HAL; fallback to one-shot
                }
            }

            val (durationMs, amplitude) = fallbackVibrationParams(kind, intensity)

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val effect = if (vib.hasAmplitudeControl() && amplitude > 0) {
                    VibrationEffect.createOneShot(durationMs, amplitude.coerceIn(1, 255))
                } else {
                    VibrationEffect.createOneShot(durationMs, VibrationEffect.DEFAULT_AMPLITUDE)
                }
                vib.vibrate(effect)
            } else {
                @Suppress("DEPRECATION")
                vib.vibrate(durationMs)
            }
        } catch (_: SecurityException) {
            // Missing VIBRATE permission on odd sandbox runtimes
        } catch (_: Exception) {
            // Guard against vendor vibrator driver anomalies
        }
    }
}