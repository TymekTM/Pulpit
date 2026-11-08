//! Seasonal themes on the tablet (.scratch/christmas-mode/spec.md). The
//! desktop's season producer pushes `season` ("christmas", "halloween",
//! "easter", "valentines" or "off") as a custom value, so it arrives here
//! as the `ext.season` state channel - no protocol change. While a season
//! is on the board gets its ambient particles (snow, autumn leaves,
//! petals, rising hearts), a burst on every touch (snowflakes, embers,
//! eggs, hearts) and the season's accent on the board chip.
//!
//! Motion runs on raw frame time (`withFrameNanos`), like the board
//! switch: deck tablets often run with the system animator scales at 0,
//! which would freeze standard Compose animations. Frames are throttled
//! to ~30 fps and only the draw phase reads the clock, so the board
//! never recomposes for the snow.

package app.pulpit.mobile.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.changedToDown
import androidx.compose.ui.input.pointer.pointerInput
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/** The state channel the season producer's `season` key lands on. */
const val SEASON_CHANNEL = "ext.season"

internal enum class ParticleShape { SNOW, LEAF, PETAL, HEART }
internal enum class BurstShape { FLAKE, EMBER, EGG, HEART }

/** How a season's ambient particles look and move: [sizeDp] is the
 *  radius range, [share] the part of the particle budget used, [spinDps]
 *  the top rotation speed (degrees per second), [rise] floats them up. */
internal class AmbientLook(
    val shape: ParticleShape,
    val colors: List<Color>,
    val sizeDp: ClosedFloatingPointRange<Float>,
    val share: Float,
    val spinDps: Float,
    val rise: Boolean,
)

private fun rgb(v: Long) = Color(0xFF000000 or v)

/**
 * The seasons, same names, windows and palettes as the desktop
 * (src/season.css). Red stays out of the chrome (it means "error" in the
 * design system), so no accent is red.
 */
enum class Season(
    val wire: String,
    val accent: Color,
    internal val ambient: AmbientLook,
    internal val burst: BurstShape,
    internal val burstColors: List<Color>,
) {
    CHRISTMAS(
        "christmas",
        rgb(0x3FB56B),
        AmbientLook(ParticleShape.SNOW, listOf(Color.White), 1.2f..3.4f, 1f, 0f, rise = false),
        BurstShape.FLAKE,
        listOf(Color.White),
    ),
    HALLOWEEN(
        "halloween",
        rgb(0xF0882A),
        AmbientLook(
            ParticleShape.LEAF,
            listOf(rgb(0xF08A24), rgb(0xD9692A), rgb(0xA8552A), rgb(0xE3B04B)),
            3.5f..6f, 0.55f, 120f, rise = false,
        ),
        BurstShape.EMBER,
        listOf(rgb(0xFF9A3C), rgb(0xB57CFF), rgb(0xFFD36E)),
    ),
    EASTER(
        "easter",
        rgb(0xB18BE8),
        AmbientLook(
            ParticleShape.PETAL,
            listOf(rgb(0xF7C6D9), rgb(0xFFF3B0), rgb(0xCDE8B5), rgb(0xC9D8F5), rgb(0xE2D1F7)),
            2.5f..4.5f, 0.7f, 80f, rise = false,
        ),
        BurstShape.EGG,
        listOf(rgb(0xF7C6D9), rgb(0xFFF3B0), rgb(0xCDE8B5), rgb(0xC9D8F5), rgb(0xE2D1F7)),
    ),
    VALENTINES(
        "valentines",
        rgb(0xE2679F),
        AmbientLook(
            ParticleShape.HEART,
            listOf(rgb(0xF06292), rgb(0xEC407A), rgb(0xF8BBD0), rgb(0xE57399)),
            4f..6.5f, 0.5f, 0f, rise = true,
        ),
        BurstShape.HEART,
        listOf(rgb(0xF06292), rgb(0xFF8FB1), rgb(0xF8BBD0)),
    ),
    ;

    companion object {
        /** A season by its wire name; `null` for "off" or anything unknown. */
        fun fromWire(name: String): Season? = entries.firstOrNull { it.wire == name }
    }
}

/** The season the pushed values put the board in, or `null` for none. */
fun seasonOf(values: Map<String, JsonElement>): Season? =
    (values[SEASON_CHANNEL] as? JsonPrimitive)?.takeIf { it.isString }?.content?.let(Season::fromWire)

/** One ambient particle: horizontal slot (0..1 of the width), start
 *  phase and speed (fractions of the height per second), size, sway,
 *  palette slot and spin. */
internal data class Flake(
    val x: Float,
    val phase: Float,
    val speed: Float,
    val radiusDp: Float,
    val alpha: Float,
    val swayDp: Float,
    val swayHz: Float,
    val color: Int = 0,
    val spinDps: Float = 0f,
)

/** A fixed sky: the same seed always gives the same particles, so a
 *  recomposition never reshuffles them. */
internal fun makeFlakes(count: Int, seed: Long = 1224L, look: AmbientLook = Season.CHRISTMAS.ambient): List<Flake> {
    val r = java.util.Random(seed)
    val n = maxOf(1, Math.round(count * look.share))
    val span = look.sizeDp.endInclusive - look.sizeDp.start
    return List(n) {
        Flake(
            x = r.nextFloat(),
            phase = r.nextFloat(),
            speed = 0.05f + r.nextFloat() * 0.06f,
            radiusDp = look.sizeDp.start + r.nextFloat() * span,
            alpha = 0.35f + r.nextFloat() * 0.5f,
            swayDp = 6f + r.nextFloat() * 18f,
            swayHz = 0.08f + r.nextFloat() * 0.12f,
            color = r.nextInt(look.colors.size),
            spinDps = (if (r.nextBoolean()) 1f else -1f) * look.spinDps * (0.5f + r.nextFloat() * 0.5f),
        )
    }
}

/** Vertical position of [f] at [tSec], as a fraction of the height in
 *  0..1; it wraps, so the flake re-enters at the top. */
internal fun flakeY(f: Flake, tSec: Float): Float {
    val y = (f.phase + f.speed * tSec) % 1f
    return if (y < 0f) y + 1f else y
}

/** Lifetime of one touch burst. */
internal const val BURST_MS = 700L
internal const val BURST_PARTICLES = 8

/** 0 at the touch, 1 when the burst is gone. */
internal fun burstProgress(startNanos: Long, nowNanos: Long): Float =
    ((nowNanos - startNanos) / 1_000_000f / BURST_MS).coerceIn(0f, 1f)

internal class Burst(val at: Offset, val startNanos: Long)

/** Cap on live bursts, so a frantic tapper can't pile them up. */
private const val MAX_BURSTS = 6
private const val FRAME_NS = 33_000_000L // ~30 fps
private const val FLAKE_COUNT = 34

/** Touch bursts waiting to finish; fed by [SeasonLayer]'s touch watcher
 *  and drained by its frame loop. */
private class BurstState {
    val bursts: SnapshotStateList<Burst> = mutableStateListOf()
    var now by mutableLongStateOf(0L)
}

/**
 * Board overlay for the seasonal themes: draws the [season]'s particles
 * and touch bursts over [content] without taking any touches from it
 * (`null` draws nothing). The content sits at the same place in the tree
 * either way, so switching the season never rebuilds the board's tiles.
 */
@Composable
fun SeasonLayer(season: Season?, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val state = remember { BurstState() }
    val touchWatch = if (season != null) {
        Modifier.pointerInput(Unit) {
            // watch downs on the way in (Initial pass) without consuming
            // them: the tile under the finger still gets the gesture
            awaitPointerEventScope {
                while (true) {
                    val event = awaitPointerEvent(PointerEventPass.Initial)
                    for (change in event.changes) {
                        if (change.changedToDown()) {
                            if (state.bursts.size >= MAX_BURSTS) state.bursts.removeAt(0)
                            state.bursts.add(Burst(change.position, state.now))
                        }
                    }
                }
            }
        }
    } else {
        Modifier
    }
    Box(modifier.then(touchWatch)) {
        content()
        if (season != null) SnowOverlay(state, season)
    }
}

@Composable
private fun SnowOverlay(state: BurstState, season: Season) {
    val flakes = remember(season) { makeFlakes(FLAKE_COUNT, look = season.ambient) }
    var origin by remember { mutableLongStateOf(0L) }

    LaunchedEffect(Unit) {
        origin = withFrameNanos { it }
        state.now = origin
        while (true) {
            val t = withFrameNanos { it }
            if (t - state.now < FRAME_NS) continue
            state.now = t
            if (state.bursts.isNotEmpty()) {
                state.bursts.removeAll { burstProgress(it.startNanos, t) >= 1f }
            }
        }
    }
    DisposableEffect(Unit) {
        onDispose { state.bursts.clear() }
    }

    // only the draw phase reads the clock: no recomposition per frame
    Canvas(Modifier.fillMaxSize()) {
        val now = state.now
        drawAmbient(season.ambient, flakes, (now - origin) / 1_000_000_000f)
        for (b in state.bursts) drawBurst(season, b, burstProgress(b.startNanos, now))
    }
}

// unit shapes, -1..1 around the origin; scaled and rotated per particle.
// Lazy: a Path is a platform object, and building it at class load would
// break the JVM unit tests that touch this file
private val leafPath by lazy { Path().apply {
    moveTo(0f, -1f)
    cubicTo(0.8f, -0.5f, 0.8f, 0.5f, 0f, 1f)
    cubicTo(-0.8f, 0.5f, -0.8f, -0.5f, 0f, -1f)
    close()
} }
private val petalPath by lazy { Path().apply {
    moveTo(0f, -1f)
    cubicTo(0.7f, -0.3f, 0.8f, 0.9f, 0f, 0.9f)
    cubicTo(-0.8f, 0.9f, -0.7f, -0.3f, 0f, -1f)
    close()
} }
private val heartPath by lazy { Path().apply {
    moveTo(0f, 0.9f)
    cubicTo(-1.5f, -0.1f, -0.9f, -1.2f, 0f, -0.45f)
    cubicTo(0.9f, -1.2f, 1.5f, -0.1f, 0f, 0.9f)
    close()
} }

private fun DrawScope.drawUnit(path: Path, center: Offset, radius: Float, degrees: Float, color: Color) {
    withTransform({
        translate(center.x, center.y)
        rotate(degrees, pivot = Offset.Zero)
        scale(radius, radius, pivot = Offset.Zero)
    }) {
        drawPath(path, color)
    }
}

private fun DrawScope.drawAmbient(look: AmbientLook, flakes: List<Flake>, tSec: Float) {
    val h = size.height
    val w = size.width
    for (f in flakes) {
        val r = f.radiusDp * density
        val fy = flakeY(f, tSec)
        val y = (if (look.rise) 1f - fy else fy) * (h + 4 * r) - 2 * r
        val sway = sin(2 * PI * f.swayHz * tSec + f.phase * 6.28).toFloat() * f.swayDp * density
        val center = Offset(f.x * w + sway, y)
        val color = look.colors[f.color % look.colors.size].copy(alpha = f.alpha * 0.8f)
        val spin = (f.spinDps * tSec + f.phase * 360f) % 360f
        when (look.shape) {
            ParticleShape.SNOW -> drawCircle(color = color, radius = r, center = center)
            ParticleShape.LEAF -> drawUnit(leafPath, center, r, spin, color)
            ParticleShape.PETAL -> drawUnit(petalPath, center, r, spin, color)
            ParticleShape.HEART -> drawUnit(heartPath, center, r, sway / density * 0.6f, color)
        }
    }
}

private fun DrawScope.drawBurst(season: Season, b: Burst, p: Float) {
    if (p >= 1f) return
    val ease = 1f - (1f - p) * (1f - p) // ease-out
    val dist = (14f + 34f * ease) * density
    val arm = (3.5f + 2f * (1f - p)) * density
    val alpha = 1f - p
    for (i in 0 until BURST_PARTICLES) {
        val a = (i.toFloat() / BURST_PARTICLES) * 2 * PI.toFloat() + p * 1.2f
        val c = Offset(b.at.x + cos(a) * dist, b.at.y + sin(a) * dist)
        val color = season.burstColors[i % season.burstColors.size].copy(alpha = alpha)
        when (season.burst) {
            BurstShape.FLAKE -> {
                // a tiny six-armed flake: three crossing strokes
                for (k in 0 until 3) {
                    val ka = a + k * (PI.toFloat() / 3f)
                    val dx = cos(ka) * arm
                    val dy = sin(ka) * arm
                    drawLine(
                        color = color,
                        start = Offset(c.x - dx, c.y - dy),
                        end = Offset(c.x + dx, c.y + dy),
                        strokeWidth = 1.4f * density,
                        cap = StrokeCap.Round,
                    )
                }
            }
            BurstShape.EMBER -> {
                // a glowing ember: soft halo plus a hot core
                drawCircle(color.copy(alpha = alpha * 0.3f), radius = 5f * density, center = c)
                drawCircle(color, radius = 2.2f * density, center = c)
            }
            BurstShape.EGG -> {
                val ew = 4f * density
                val eh = 5.5f * density
                drawOval(color, topLeft = Offset(c.x - ew, c.y - eh), size = Size(2 * ew, 2 * eh))
            }
            BurstShape.HEART -> drawUnit(heartPath, c, arm * 1.3f, 0f, color)
        }
    }
}
