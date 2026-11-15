//! Native port of the desktop pet sprite (apps/desktop/src/components/
//! TamagotchiSprite.vue). Same 200x200 viewBox, the same path data, colors
//! and stroke widths, and the same CSS keyframes replayed on a frame clock,
//! so a tablet draws the very animal the editor shows. When the Vue sprite
//! changes, change this file with it.
//!
//! Coordinates: every part is drawn in the SVG's user space. The caller maps
//! the viewBox onto the canvas; groups below translate to the ground point
//! (100, 172) exactly like the SVG does.

package app.pulpit.mobile.ui

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser

// ---------------------------------------------------------------------------
// CSS animation engine (pure, unit-tested)
// ---------------------------------------------------------------------------

/** CSS `cubic-bezier(x1, y1, x2, y2)` timing function. */
internal fun cubicBezier(x1: Float, y1: Float, x2: Float, y2: Float): (Float) -> Float {
    fun bez(t: Float, a: Float, b: Float): Float {
        val u = 1f - t
        return 3f * u * u * t * a + 3f * u * t * t * b + t * t * t
    }
    return { x ->
        when {
            x <= 0f -> 0f
            x >= 1f -> 1f
            else -> {
                var lo = 0f
                var hi = 1f
                repeat(22) {
                    val mid = (lo + hi) / 2f
                    if (bez(mid, x1, x2) < x) lo = mid else hi = mid
                }
                bez((lo + hi) / 2f, y1, y2)
            }
        }
    }
}

internal val EaseInOut = cubicBezier(0.42f, 0f, 0.58f, 1f)
internal val EaseIn = cubicBezier(0.42f, 0f, 1f, 1f)
internal val EaseOut = cubicBezier(0f, 0f, 0.58f, 1f)
internal val Ease = cubicBezier(0.25f, 0.1f, 0.25f, 1f)
internal val Linear: (Float) -> Float = { it }
private val PopEase = cubicBezier(0.2f, 0.9f, 0.3f, 1.3f)

/** One property of a CSS `@keyframes` rule: values at stops in 0..1. The
 *  timing function applies per segment, like a browser does. */
internal class Track(private val stops: FloatArray, private val values: FloatArray) {
    fun at(p: Float, ease: (Float) -> Float): Float {
        if (p <= stops[0]) return values[0]
        for (i in 1 until stops.size) {
            if (p <= stops[i]) {
                val a = stops[i - 1]
                val span = stops[i] - a
                val k = if (span <= 0f) 1f else ease((p - a) / span)
                return values[i - 1] + (values[i] - values[i - 1]) * k
            }
        }
        return values.last()
    }
}

/** `track(stop0, value0, stop1, value1, ...)`. */
internal fun track(vararg sv: Float): Track =
    Track(FloatArray(sv.size / 2) { sv[it * 2] }, FloatArray(sv.size / 2) { sv[it * 2 + 1] })

/** Directed progress (0..1) of an infinite CSS animation [tMs] after it
 *  started. A negative delay starts mid-cycle; a positive one holds the
 *  first keyframe (fill-mode backwards). `alternate` plays every second
 *  cycle backwards, `reverse` every cycle. */
internal fun cssProgress(
    tMs: Long,
    durMs: Int,
    delayMs: Int = 0,
    alternate: Boolean = false,
    reverse: Boolean = false,
): Float {
    val local = tMs - delayMs
    if (local < 0) return if (reverse) 1f else 0f
    val cycle = local / durMs
    var p = (local % durMs).toFloat() / durMs
    if (alternate && cycle % 2L == 1L) p = 1f - p
    if (reverse) p = 1f - p
    return p
}

// ---------------------------------------------------------------------------
// palette (TamagotchiSprite.vue <style>)
// ---------------------------------------------------------------------------

/** Creature colors per species: `--c-shade/light/stroke/belly`. */
internal class PetPalette(val shade: Color, val light: Color, val stroke: Color, val belly: Color)

internal fun petPalette(species: String): PetPalette = when (species) {
    "cat" -> PetPalette(Color(0xFF7189A2), Color(0xFF8FA6BD), Color(0xFF4F667D), Color(0xFFEFE0C4))
    "frog" -> PetPalette(Color(0xFF5F9732), Color(0xFF7CB342), Color(0xFF3F6F22), Color(0xFFF6F0A8))
    "chick" -> PetPalette(Color(0xFFE9B937), Color(0xFFF7D154), Color(0xFFB88A14), Color(0xFFFFF4C2))
    "axolotl" -> PetPalette(Color(0xFFDD7D9B), Color(0xFFF19BB4), Color(0xFFB6527A), Color(0xFFE9DFFA))
    else -> PetPalette(Color(0xFF16A085), Color(0xFF1ABC9C), Color(0xFF0E7A65), Color(0xFFDFF6F0))
}

/** Egg speckle / dark speckle / hatch-hole colors: the egg hints who is
 *  inside. The first one also tints the ghost's halo (not for blob). */
internal fun petAccent(species: String): List<Color> = when (species) {
    "cat" -> listOf(Color(0xFF8FA6BD), Color(0xFF6F89A3), Color(0xFF52697F))
    "frog" -> listOf(Color(0xFF7CB342), Color(0xFF5F9732), Color(0xFF456F25))
    "chick" -> listOf(Color(0xFFF39C12), Color(0xFFD68910), Color(0xFFA8680A))
    "axolotl" -> listOf(Color(0xFFE06A92), Color(0xFFC24F78), Color(0xFF963A5A))
    else -> listOf(Color(0xFF1ABC9C), Color(0xFF16A085), Color(0xFF12806B))
}

private val Ink = Color(0xFF22313A)
private val White = Color.White
private val Black = Color.Black
private val Blush = Color(0xFFFF8FA3)
private val Drop = Color(0xFF8FD8F5)

// ---------------------------------------------------------------------------
// path data, verbatim from the Vue template
// ---------------------------------------------------------------------------

/** Every path string the sprite draws; `all` lets a unit test parse them. */
internal object PetPaths {
    val all = mutableListOf<String>()
    private fun d(s: String): String = s.also { all += it }

    val EGG = d("M0,-150 C36,-150 60,-96 60,-58 C60,-22 34,-2 0,-2 C-34,-2 -60,-22 -60,-58 C-60,-96 -36,-150 0,-150 Z")
    val HOLE = d("M-32,-84 L-23,-95 L-11,-83 L0,-97 L11,-83 L23,-95 L32,-84 L28,-72 Q0,-60 -28,-72 Z")
    val CRACKS = listOf(
        d("M-32,-84 L-40,-76 L-35,-68 L-44,-58"),
        d("M32,-84 L41,-78 L36,-70"),
        d("M0,-97 L5,-108 L-2,-116 L4,-127"),
        d("M-23,-95 L-27,-104"),
        d("M-18,-40 L-11,-49 L-16,-56"),
    )
    val GHOST = d("M-47,-40 C-47,-70 -27,-84 0,-84 C27,-84 47,-70 47,-40 L47,-10 Q31.3,4 15.7,-10 Q0,4 -15.7,-10 Q-31.3,4 -47,-10 Z")
    val X_EYE_L = d("M-22,-59 L-12,-49 M-12,-59 L-22,-49")
    val X_EYE_R = d("M12,-59 L22,-49 M22,-59 L12,-49")

    val POO_TIP = d("M-2.4,-19 Q-1.5,-26 3.2,-24.5 Q0.4,-22.6 1.8,-19.4 Z")
    val POO_SHINE = d("M-8,-7.5 Q-6.4,-10 -3.8,-10.2")
    val POO_SMILE = d("M-1.8,-2.4 Q0,-1 1.8,-2.4")

    private val ROUND_BODY = d("M-47,-40 C-47,-70 -27,-84 0,-84 C27,-84 47,-70 47,-40 C47,-14 40,-4 23,-4 L-23,-4 C-40,-4 -47,-14 -47,-40 Z")
    val BODY = mapOf(
        "blob" to ROUND_BODY,
        "cat" to ROUND_BODY,
        "frog" to d("M-52,-30 C-52,-56 -34,-70 0,-70 C34,-70 52,-56 52,-30 C52,-12 44,-4 26,-4 L-26,-4 C-44,-4 -52,-12 -52,-30 Z"),
        "chick" to d("M-44,-42 C-44,-70 -26,-84 0,-84 C26,-84 44,-70 44,-42 C44,-16 31,-4 0,-4 C-31,-4 -44,-16 -44,-42 Z"),
        "axolotl" to d("M-48,-34 C-48,-60 -31,-74 0,-74 C31,-74 48,-60 48,-34 C48,-14 40,-4 23,-4 L-23,-4 C-40,-4 -48,-14 -48,-34 Z"),
    )

    val SPROUT_STEM = d("M0,-82 C0,-88 1,-92 3,-95")
    val SPROUT_LEAF = d("M3,-95 C0,-103 8,-108 14,-105 C14,-98 9,-93 3,-95 Z")
    val SPROUT_VEIN = d("M5,-97 Q8.5,-100 11.5,-102")

    val CAT_TAIL = d("M36,-12 C58,-10 64,-30 57,-50")
    val CAT_TAIL_TIP = d("M60,-43 Q58.6,-46.6 57,-50")
    val CAT_EAR_L = d("M-45,-56 L-39,-103 Q-38,-105.5 -35.5,-104 L-9,-79 Z")
    val CAT_EAR_L_IN = d("M-38,-71 L-36,-93 L-21,-79 Z")
    val CAT_EAR_R = d("M45,-56 L39,-103 Q38,-105.5 35.5,-104 L9,-79 Z")
    val CAT_EAR_R_IN = d("M38,-71 L36,-93 L21,-79 Z")
    val CAT_RUFF = d("M-26,-38 C-31,-28 -23,-17 -15,-23 C-13,-14 -3,-12 0,-21 C3,-12 13,-14 15,-23 C23,-17 31,-28 26,-38 Q0,-30 -26,-38 Z")
    val CAT_NOSE = d("M-3.8,-48.6 H3.8 L0,-44.6 Z")
    val CAT_WHISKERS = listOf(
        d("M-23,-44 L-53,-48"),
        d("M-23,-38 L-52,-34"),
        d("M23,-44 L53,-48"),
        d("M23,-38 L52,-34"),
    )

    val CHICK_CURL = d("M0,-83 C-2,-93 9,-97 10,-90 C11,-85 5,-85 5,-89")
    val CHICK_FOOT = d("M-10,0.5 Q-10,-6 -5,-6.5 Q0,-7.5 5,-6.5 Q10,-6 10,0.5 Q7,2.5 3.5,0.5 Q0,3 -3.5,0.5 Q-7,2.5 -10,0.5 Z")
    val CHICK_WING = d("M-40,-56 C-52,-54 -58,-38 -52,-26 C-48,-20 -40,-24 -37,-30 C-33,-40 -34,-52 -40,-56 Z")
    val CHICK_WING_PATCH = d("M-45,-47 C-50,-42 -51,-34 -47,-30 C-43,-33 -42,-42 -45,-47 Z")
    val BEAK_TOP = d("M-6.5,-47 Q0,-52.5 6.5,-47 Q3,-44 0,-44 Q-3,-44 -6.5,-47 Z")
    val BEAK_BOTTOM = d("M-4.6,-42.6 Q0,-41.6 4.6,-42.6 Q3.4,-37.4 0,-37.4 Q-3.4,-37.4 -4.6,-42.6 Z")
    val BEAK_SHUT = d("M-6.5,-47 Q0,-52.5 6.5,-47 Q4.5,-41.5 0,-40.5 Q-4.5,-41.5 -6.5,-47 Z")

    val AXO_TAIL = d("M32,-8 C48,-4 60,-12 59,-26 C58,-33 50,-26 32,-22 Z")
    val AXO_FIN_RIB = d("M38,-14 C46,-12 53,-16 55,-23")

    // eyes
    val SLEEP_EYE_L = d("M-23,-54 Q-17,-48.5 -11,-54")
    val SLEEP_EYE_R = d("M11,-54 Q17,-48.5 23,-54")
    val HAPPY_EYE_L = d("M-23,-52 Q-17,-61 -11,-52")
    val HAPPY_EYE_R = d("M11,-52 Q17,-61 23,-52")
    val SAD_BROW_L = d("M-25,-61 L-12,-65")
    val SAD_BROW_R = d("M12,-65 L25,-61")
    val HUNGRY_EYE_L = d("M-22.2,-57 A6,6 0 1 0 -11.8,-57 Z")
    val HUNGRY_EYE_R = d("M11.8,-57 A6,6 0 1 0 22.2,-57 Z")
    val HUNGRY_BROW_L = d("M-24,-62 L-12,-65.5")
    val HUNGRY_BROW_R = d("M12,-65.5 L24,-62")
    val SLEEPY_EYE_L = d("M-22,-54 A5,5 0 0 0 -12,-54 Z")
    val SLEEPY_EYE_R = d("M12,-54 A5,5 0 0 0 22,-54 Z")
    val SLEEPY_LID_L = d("M-23.5,-54 H-10.5")
    val SLEEPY_LID_R = d("M10.5,-54 H23.5")
    val DIRTY_EYE_L = d("M-23,-54 H-11")
    val DIRTY_EYE_R = d("M11,-54 H23")
    val SPIRAL = d("M0,0 A1.5,1.5 0 0 1 3,0 A3,3 0 0 1 -3,0 A4.5,4.5 0 0 1 6,0 A6,6 0 0 1 -6,0")

    // frog mouths
    val FROG_DEAD = d("M-14,-41 H14")
    val FROG_HAPPY = d("M-19,-46 Q0,-26 19,-46 Z")
    val FROG_SAD = d("M-22,-39 Q0,-48 22,-39")
    val FROG_SLEEPY = d("M-20,-43 Q0,-39.5 20,-43")
    val FROG_DIRTY = d("M-20,-42 q5,-4 10,0 t10,0 t10,0 t10,0")
    val FROG_SICK = d("M-17,-42 q4.25,-3.5 8.5,0 t8.5,0 t8.5,0 t8.5,0")
    val FROG_NEUTRAL = d("M-27,-45 Q0,-36 27,-45")

    // blob / cat / axolotl mouths
    val MOUTH_DEAD = d("M-6,-41 H6")
    val AXO_HAPPY = d("M-14,-47 Q0,-28 14,-47 Z")
    val MOUTH_HAPPY = d("M-9,-46 Q0,-32 9,-46 Z")
    val MOUTH_SAD = d("M-7,-39 Q0,-46 7,-39")
    val AXO_SLEEPY = d("M-9,-44 Q0,-39.5 9,-44")
    val MOUTH_DIRTY = d("M-9,-42 q3,-3 6,0 t6,0 t6,0")
    val MOUTH_SICK = d("M-7,-42 q2.3,-3 4.6,0 t4.6,0 t4.6,0")
    val AXO_NEUTRAL = d("M-15,-46 Q0,-33 15,-46")
    val CAT_NEUTRAL = d("M-8.5,-42 Q-4.2,-37.5 0,-42.5 Q4.2,-37.5 8.5,-42")
    val MOUTH_NEUTRAL = d("M-6,-43 Q0,-38 6,-43")

    // extras
    val DROP = d("M0,0 C0,0 -3.6,5 -3.6,7.6 A3.6,3.6 0 0 0 3.6,7.6 C3.6,5 0,0 0,0 Z")
    val RUMBLE = d("M-10,-20 q2.5,-3.5 5,0 t5,0 t5,0 t5,0")
    val STINK = d("M0,0 q-4,-4 0,-8 t0,-8")
    val HEART = d("M0,5 C-10,-1 -8,-9 -4,-9 C-2,-9 0,-7 0,-6 C0,-7 2,-9 4,-9 C8,-9 10,-1 0,5 Z")
    val Z = d("M-4,-4 H4 L-4,4 H4")
}

private val pathCache = HashMap<String, Path>()

private fun svg(d: String): Path = pathCache.getOrPut(d) { PathParser().parsePathString(d).toPath() }

// ---------------------------------------------------------------------------
// drawing primitives (SVG semantics: fill first, stroke centered on top)
// ---------------------------------------------------------------------------

private fun DrawScope.fill(d: String, color: Color, alpha: Float = 1f) =
    drawPath(svg(d), color, alpha.coerceIn(0f, 1f))

private fun DrawScope.line(d: String, color: Color, width: Float, alpha: Float = 1f) =
    drawPath(
        svg(d),
        color,
        alpha.coerceIn(0f, 1f),
        style = Stroke(width, cap = StrokeCap.Round, join = StrokeJoin.Round),
    )

private fun DrawScope.shape(d: String, fill: Color, stroke: Color, width: Float) {
    fill(d, fill)
    line(d, stroke, width)
}

private fun DrawScope.oval(cx: Float, cy: Float, rx: Float, ry: Float, color: Color, alpha: Float = 1f) =
    drawOval(color, Offset(cx - rx, cy - ry), Size(rx * 2f, ry * 2f), alpha.coerceIn(0f, 1f))

private fun DrawScope.ovalLine(cx: Float, cy: Float, rx: Float, ry: Float, color: Color, width: Float) =
    drawOval(color, Offset(cx - rx, cy - ry), Size(rx * 2f, ry * 2f), style = Stroke(width))

private fun DrawScope.ovalShape(cx: Float, cy: Float, rx: Float, ry: Float, fill: Color, stroke: Color, width: Float) {
    oval(cx, cy, rx, ry, fill)
    ovalLine(cx, cy, rx, ry, stroke, width)
}

private fun DrawScope.dot(cx: Float, cy: Float, r: Float, color: Color) =
    drawCircle(color, r, Offset(cx, cy))

/** SVG `translate(x y)`. */
private fun DrawScope.at(x: Float, y: Float, block: DrawScope.() -> Unit) =
    withTransform({ translate(x, y) }, block)

/** SVG `rotate(deg px py)`. */
private fun DrawScope.rot(deg: Float, px: Float, py: Float, block: DrawScope.() -> Unit) =
    withTransform({ rotate(deg, Offset(px, py)) }, block)

/** SVG `scale(sx sy)` around the origin. */
private fun DrawScope.scl(sx: Float, sy: Float = sx, block: DrawScope.() -> Unit) =
    withTransform({ scale(sx, sy, Offset.Zero) }, block)

/** A CSS `transform: translate() rotate() scale()` with `transform-origin`
 *  (ox, oy) in the element's user space. */
private fun DrawScope.css(
    ox: Float,
    oy: Float,
    tx: Float = 0f,
    ty: Float = 0f,
    rot: Float = 0f,
    sx: Float = 1f,
    sy: Float = 1f,
    block: DrawScope.() -> Unit,
) = withTransform(
    {
        translate(ox + tx, oy + ty)
        if (rot != 0f) rotate(rot, Offset.Zero)
        if (sx != 1f || sy != 1f) scale(sx, sy, Offset.Zero)
        translate(-ox, -oy)
    },
    block,
)

private val layerPaint = Paint()
private val LayerBounds = Rect(-400f, -400f, 400f, 400f)

/** A group with `opacity`: overlapping children composite as one layer. */
private fun DrawScope.group(alpha: Float, block: DrawScope.() -> Unit) {
    when {
        alpha >= 0.999f -> block()
        alpha <= 0.001f -> Unit
        else -> {
            layerPaint.alpha = alpha
            drawContext.canvas.saveLayer(LayerBounds, layerPaint)
            block()
            drawContext.canvas.restore()
        }
    }
}

// ---------------------------------------------------------------------------
// keyframes (TamagotchiSprite.vue @keyframes, one Track per property)
// ---------------------------------------------------------------------------

private val POP_S = track(0f, 0.55f, 0.55f, 1.07f, 1f, 1f)
private val POP_A = track(0f, 0f, 0.55f, 1f, 1f, 1f)

private val BOB_Y = track(0f, 0f, 0.5f, -3f, 1f, 0f)
private val BOB_SX = track(0f, 1f, 0.5f, 0.985f, 1f, 1f)
private val BOB_SY = track(0f, 1f, 0.5f, 1.025f, 1f, 1f)
private val MUNCH_Y = track(0f, 0f, 0.25f, 0.5f, 0.5f, 1f, 0.75f, 0.5f, 1f, 0f)
private val MUNCH_R = track(0f, 0f, 0.25f, -1.5f, 0.5f, 0f, 0.75f, 1.5f, 1f, 0f)
private val MUNCH_SX = track(0f, 1f, 0.25f, 1.02f, 0.5f, 1.03f, 0.75f, 1.02f, 1f, 1f)
private val MUNCH_SY = track(0f, 1f, 0.25f, 0.97f, 0.5f, 0.95f, 0.75f, 0.97f, 1f, 1f)
private val BOUNCE_Y = track(0f, 0f, 0.25f, -12f, 0.5f, 0f, 0.75f, -12f, 1f, 0f)
private val BOUNCE_R = track(0f, 0f, 0.25f, -7f, 0.5f, 0f, 0.75f, 7f, 1f, 0f)
private val BOUNCE_SX = track(0f, 1.07f, 0.25f, 0.96f, 0.5f, 1.07f, 0.75f, 0.96f, 1f, 1.07f)
private val BOUNCE_SY = track(0f, 0.9f, 0.25f, 1.06f, 0.5f, 0.9f, 0.75f, 1.06f, 1f, 0.9f)
private val SHADOW_PLAY_S = track(0f, 1f, 0.25f, 0.8f, 0.5f, 1f, 0.75f, 0.8f, 1f, 1f)
private val SHADOW_PLAY_A = track(0f, 0.3f, 0.25f, 0.18f, 0.5f, 0.3f, 0.75f, 0.18f, 1f, 0.3f)
private val SWAY_R = track(0f, -2.5f, 0.5f, 2.5f, 1f, -2.5f)
private val SWAY_SX = track(0f, 1.02f, 0.5f, 0.99f, 1f, 1.02f)
private val SWAY_SY = track(0f, 0.98f, 0.5f, 1.02f, 1f, 0.98f)
private val BREATHE_SX = track(0f, 1.04f, 0.5f, 1.02f, 1f, 1.04f)
private val BREATHE_SY = track(0f, 0.93f, 0.5f, 0.96f, 1f, 0.93f)

private val EGG_WOBBLE = track(0f, 0f, 0.62f, 0f, 0.68f, -4f, 0.74f, 3.5f, 0.8f, -2.5f, 0.86f, 1.5f, 0.92f, 0f, 1f, 0f)
private val HATCH_WOBBLE = track(0f, 0f, 0.15f, -8f, 0.3f, 7f, 0.45f, -5f, 0.6f, 4f, 0.75f, -1.5f, 1f, 0f)

private val BLINK = track(0f, 1f, 0.9f, 1f, 0.94f, 0.1f, 0.97f, 1f, 1f, 1f)
private val LID = track(0f, 0f, 0.9f, 0f, 0.94f, 1f, 0.97f, 0f, 1f, 0f)
private val CHEW_SX = track(0f, 1f, 0.5f, 1.15f, 1f, 1f)
private val CHEW_SY = track(0f, 1f, 0.5f, 1.6f, 1f, 1f)
private val SPROUT_R = track(0f, -4f, 1f, 5f)

private val CRUMB_TY = track(0f, 0f, 0.15f, -6f, 1f, 26f)
private val CRUMB_S = track(0f, 0.3f, 0.15f, 1f, 1f, 0.8f)
private val CRUMB_A = track(0f, 0f, 0.15f, 1f, 1f, 0f)
private val HEART_TY = track(0f, 0f, 0.2f, -4f, 1f, -24f)
private val HEART_S = track(0f, 0.4f, 0.2f, 1f, 1f, 0.9f)
private val HEART_A = track(0f, 0f, 0.2f, 1f, 1f, 0f)
private val Z_TX = track(0f, 0f, 1f, 16f)
private val Z_TY = track(0f, 0f, 1f, -28f)
private val Z_A = track(0f, 0f, 0.15f, 1f, 1f, 0f)
private val STINK_TY = track(0f, 4f, 1f, -6f)
private val STINK_A = track(0f, 0f, 0.3f, 0.9f, 1f, 0f)
private val TEAR_TY = track(0f, 0f, 0.15f, 0f, 0.25f, 0f, 0.8f, 14f, 1f, 18f)
private val TEAR_S = track(0f, 0.6f, 0.15f, 0.6f, 0.25f, 1f, 1f, 1f)
private val TEAR_A = track(0f, 0f, 0.15f, 0f, 0.25f, 1f, 0.8f, 1f, 1f, 0f)
private val DROOL_TY = track(0f, 0f, 0.45f, 0f, 1f, 12f)
private val DROOL_SY = track(0f, 0.3f, 0.45f, 1f, 1f, 1f)
private val DROOL_A = track(0f, 1f, 0.45f, 1f, 1f, 0f)
private val RUMBLE_X = track(0f, -1.4f, 1f, 1.4f)
private val POO_SX = track(0f, 1f, 0.5f, 1.03f, 1f, 1f)
private val POO_SY = track(0f, 1f, 0.5f, 0.95f, 1f, 1f)

private val TAIL_R = track(0f, -7f, 1f, 8f)
private val EAR_L = track(0f, 0f, 0.9f, 0f, 0.93f, -8f, 0.96f, 2f, 1f, 0f)
private val EAR_R = track(0f, 0f, 0.88f, 0f, 0.91f, 8f, 0.95f, -2f, 1f, 0f)
private val WING = track(0f, 0f, 0.5f, 4f, 1f, 0f)
private val FLAP = track(0f, 0f, 1f, 34f)
private val GILL_R = track(0f, -9f, 1f, 9f)
private val GILL_SX = track(0f, 1f, 1f, 1.1f)
private val GILL_SY = track(0f, 1f, 1f, 1.14f)

private val GHOST_FLOAT = track(0f, -12f, 0.5f, -22f, 1f, -12f)
private val GHOST_SHADOW_S = track(0f, 1f, 0.5f, 0.82f, 1f, 1f)
private val GHOST_SHADOW_A = track(0f, 0.18f, 0.5f, 0.1f, 1f, 0.18f)
private val HALO_R = track(0f, -5f, 1f, 5f)
private val HALO_Y = track(0f, 0f, 1f, -1.5f)

// ---------------------------------------------------------------------------
// the sprite
// ---------------------------------------------------------------------------

/** Clocks the sprite animates on, all in milliseconds: [t] since the tile
 *  appeared, [enter] since the egg / hatching / ghost / creature branch
 *  last changed (the pop-in), [face] since the face last changed (fade). */
internal class PetClock(val t: Long, val enter: Long, val face: Long)

/** Which top-level branch of the template draws [stage]. */
internal fun petSpriteBranch(stage: String): String = when (stage) {
    "egg", "hatching", "dead" -> stage
    else -> "creature"
}

/** The sprite in viewBox units (0..200 on both axes). */
internal fun DrawScope.drawPetSprite(pet: PetState, clock: PetClock) {
    when (petSpriteBranch(pet.stage)) {
        "egg" -> drawEgg(hatching = false, pet.species, clock)
        "hatching" -> drawEgg(hatching = true, pet.species, clock)
        "dead" -> drawGhost(pet.species, clock)
        else -> drawCreature(pet, clock)
    }
}

/** `.tam-enter`: the half-second pop when a branch mounts. */
private fun DrawScope.enter(ms: Long, block: DrawScope.() -> Unit) {
    if (ms >= 500) return block()
    val p = ms.coerceAtLeast(0) / 500f
    scl(POP_S.at(p, PopEase)) { group(POP_A.at(p, PopEase).coerceIn(0f, 1f), block) }
}

private class Speck(val x: Float, val y: Float, val r: Float, val dark: Boolean)

private val EGG_SPECKS = listOf(
    Speck(-22f, -102f, 3.4f, false), Speck(16f, -122f, 2.6f, true), Speck(30f, -88f, 3.8f, false),
    Speck(-38f, -68f, 3f, true), Speck(4f, -78f, 2.2f, false), Speck(-12f, -46f, 3.6f, false),
    Speck(30f, -44f, 2.8f, true), Speck(-32f, -30f, 2.4f, true), Speck(12f, -20f, 2.2f, false),
    Speck(-4f, -132f, 1.8f, true),
)
private val HATCH_SPECKS = listOf(
    Speck(-22f, -110f, 3.2f, false), Speck(22f, -120f, 2.4f, true), Speck(-40f, -60f, 3f, true),
    Speck(-14f, -44f, 3.4f, false), Speck(32f, -46f, 2.8f, true), Speck(-30f, -28f, 2.4f, true),
    Speck(12f, -22f, 2.2f, false),
)

private fun DrawScope.drawEgg(hatching: Boolean, species: String, clock: PetClock) {
    val acc = petAccent(species)
    val sw = 2.6f
    val crackColor = Color(0xFF8A7D5C)
    at(100f, 172f) {
        oval(0f, 2f, 50f, 7f, Black, 0.3f)
        enter(clock.enter) {
            val wobble = if (hatching) {
                HATCH_WOBBLE.at(cssProgress(clock.t, 800), EaseInOut)
            } else {
                EGG_WOBBLE.at(cssProgress(clock.t, 4000), EaseInOut)
            }
            css(0f, -2f, rot = wobble) {
                shape(PetPaths.EGG, Color(0xFFE2D8BA), Color(0xFFB0A37F), sw)
                at(-4f, -3f) { scl(0.93f, 0.96f) { fill(PetPaths.EGG, Color(0xFFF7F2E3)) } }
                for (s in if (hatching) HATCH_SPECKS else EGG_SPECKS) {
                    dot(s.x, s.y, s.r, if (s.dark) acc[1] else acc[0])
                }
                if (hatching) {
                    rot(24f, -32f, -112f) { oval(-32f, -112f, 8f, 14f, White, 0.5f) }
                    shape(PetPaths.HOLE, acc[2], crackColor, sw * 0.85f)
                    css(0f, -77f, sy = BLINK.at(cssProgress(clock.t, 3200), EaseInOut)) {
                        dot(-10f, -77f, 4.2f, Color(0xFFFDFDF5))
                        dot(10f, -77f, 4.2f, Color(0xFFFDFDF5))
                        dot(-9.4f, -76.4f, 2.2f, Ink)
                        dot(9.4f, -76.4f, 2.2f, Ink)
                    }
                    for (c in PetPaths.CRACKS) line(c, crackColor, sw * 0.85f)
                } else {
                    rot(24f, -30f, -100f) { oval(-30f, -100f, 9f, 17f, White, 0.55f) }
                }
            }
        }
    }
}

private fun DrawScope.drawGhost(species: String, clock: PetClock) {
    val halo = if (species == "blob") Color(0xFFE9D98A) else petAccent(species)[0]
    val sw = 2.2f
    val shade = Color(0xFF8E99A1)
    val stroke = Color(0xFF6D7880)
    val ink = Color(0xFF3A464E)
    val p = cssProgress(clock.t, 4000)
    at(100f, 172f) {
        val s = GHOST_SHADOW_S.at(p, EaseInOut)
        css(0f, 2f, sx = s, sy = s) { oval(0f, 2f, 40f, 6f, Black, GHOST_SHADOW_A.at(p, EaseInOut)) }
        enter(clock.enter) {
            scl(1.3f) {
                at(0f, GHOST_FLOAT.at(p, EaseInOut)) {
                    // halo: rotate(r) translateY(y) around (0, -96)
                    val hp = cssProgress(clock.t, 3000, alternate = true)
                    withTransform({
                        translate(0f, -96f)
                        rotate(HALO_R.at(hp, EaseInOut), Offset.Zero)
                        translate(0f, HALO_Y.at(hp, EaseInOut) + 96f)
                    }) { ovalLine(0f, -96f, 16f, 4.6f, halo, sw) }
                    group(0.8f) {
                        rot(-32f, -46f, -40f) { ovalShape(-46f, -40f, 5.5f, 8.5f, shade, stroke, sw) }
                        rot(32f, 46f, -40f) { ovalShape(46f, -40f, 5.5f, 8.5f, shade, stroke, sw) }
                        shape(PetPaths.GHOST, shade, stroke, sw)
                        at(-3f, -2f) { scl(0.94f, 0.95f) { fill(PetPaths.GHOST, Color(0xFFC4CDD3)) } }
                        rot(-38f, -27f, -68f) { oval(-27f, -68f, 9f, 4.6f, White, 0.3f) }
                    }
                    line(PetPaths.X_EYE_L, ink, sw)
                    line(PetPaths.X_EYE_R, ink, sw)
                    oval(0f, -41f, 3.2f, 3.9f, ink)
                }
            }
        }
    }
}

// ground x of the poo piles; the third hides slightly behind the first
private val POO_X = floatArrayOf(176f, 24f, 189f)

private fun DrawScope.drawPoo() {
    val dark = Color(0xFF6E5636)
    val light = Color(0xFF8B6F47)
    oval(0f, -4.5f, 11.5f, 5.2f, dark)
    oval(0f, -5.6f, 10.3f, 4.4f, light)
    oval(0f, -11f, 8.4f, 4.4f, dark)
    oval(-0.3f, -12f, 7.4f, 3.7f, light)
    oval(0f, -16.5f, 5.6f, 3.4f, dark)
    oval(-0.2f, -17.4f, 4.7f, 2.8f, light)
    shape(PetPaths.POO_TIP, light, dark, 1f)
    line(PetPaths.POO_SHINE, Color(0xFFC9A97A), 1.4f, 0.75f)
    dot(-3.8f, -4.6f, 1.2f, Ink)
    dot(3.8f, -4.6f, 1.2f, Ink)
    line(PetPaths.POO_SMILE, Ink, 1f)
}

private val BELLY = mapOf(
    "blob" to floatArrayOf(0f, -20f, 27f, 12.5f),
    "cat" to floatArrayOf(0f, -20f, 26f, 12f),
    "frog" to floatArrayOf(0f, -21f, 34f, 14f),
    "chick" to floatArrayOf(0f, -22f, 26f, 14f),
    "axolotl" to floatArrayOf(0f, -20f, 30f, 12.5f),
)
private val HIGHLIGHT = mapOf(
    "blob" to floatArrayOf(-27f, -68f),
    "cat" to floatArrayOf(-27f, -68f),
    "frog" to floatArrayOf(-30f, -56f),
    "chick" to floatArrayOf(-26f, -68f),
    "axolotl" to floatArrayOf(-28f, -60f),
)
private val SHADOW_RX = mapOf(
    "blob" to floatArrayOf(50f, 66f),
    "cat" to floatArrayOf(50f, 66f),
    "frog" to floatArrayOf(58f, 76f),
    "chick" to floatArrayOf(48f, 63f),
    "axolotl" to floatArrayOf(54f, 70f),
)

// axolotl gill fronds: attach point at the head edge + angle
private class Gill(val x: Float, val y: Float, val r: Float)

private val GILLS = listOf(Gill(37f, -63f, -56f), Gill(42f, -52f, -4f), Gill(41f, -41f, 42f))

// dirty: where the three stink lines rise from, and their delays (ms)
private val STINKS = listOf(Triple(-30f, -80f, 0), Triple(-16f, -86f, 900), Triple(30f, -80f, 1700))

/** Stroke width in the creature's local units (`--tam-sw`). */
internal fun petStrokeWidth(stage: String): Float = when (stage) {
    "adult" -> 1.9f
    "dead" -> 2.2f
    else -> 2.6f
}

/** The creature group's scale: adults are drawn 1.5x, babies 1.1x. */
internal fun petCreatureScale(stage: String): Float = if (stage == "adult") 1.5f else 1.1f

private fun DrawScope.drawCreature(pet: PetState, clock: PetClock) {
    val sp = pet.species
    val adult = pet.stage == "adult"
    val act = pet.activity
    val t = clock.t
    val pal = petPalette(sp)
    val sw = petStrokeWidth(pet.stage)

    // poop piles on the ground, beside the creature (3 first: it sits behind 1)
    for (n in intArrayOf(3, 2, 1)) {
        if (pet.poos < n) continue
        at(POO_X[n - 1], 173f) {
            scl(if (n == 3) 0.82f else 1f) {
                val p = cssProgress(t, 2600, delayMs = -900 * n)
                css(0f, 0f, sx = POO_SX.at(p, EaseInOut), sy = POO_SY.at(p, EaseInOut)) { drawPoo() }
            }
        }
    }

    at(100f, 172f) {
        val shadowRx = SHADOW_RX.getValue(sp)[if (adult) 1 else 0]
        if (act == "play") {
            val p = cssProgress(t, 1100)
            val s = SHADOW_PLAY_S.at(p, EaseInOut)
            css(0f, 2f, sx = s, sy = s) { oval(0f, 2f, shadowRx, 7f, Black, SHADOW_PLAY_A.at(p, EaseInOut)) }
        } else {
            oval(0f, 2f, shadowRx, 7f, Black, 0.3f)
        }
        enter(clock.enter) {
            scl(petCreatureScale(pet.stage)) {
                actor(act, t) { drawCreatureParts(pet, pal, sw, clock) }
            }
        }
    }
}

/** `.tam-actor`: the whole-body motion per activity. */
private fun DrawScope.actor(act: String, t: Long, block: DrawScope.() -> Unit) {
    when (act) {
        "eat" -> {
            val p = cssProgress(t, 600)
            css(
                0f, 0f, ty = MUNCH_Y.at(p, EaseInOut), rot = MUNCH_R.at(p, EaseInOut),
                sx = MUNCH_SX.at(p, EaseInOut), sy = MUNCH_SY.at(p, EaseInOut), block = block,
            )
        }
        "play" -> {
            val p = cssProgress(t, 1100)
            css(
                0f, 0f, ty = BOUNCE_Y.at(p, EaseInOut), rot = BOUNCE_R.at(p, EaseInOut),
                sx = BOUNCE_SX.at(p, EaseInOut), sy = BOUNCE_SY.at(p, EaseInOut), block = block,
            )
        }
        "pet" -> {
            val p = cssProgress(t, 1800)
            css(
                0f, 0f, rot = SWAY_R.at(p, EaseInOut),
                sx = SWAY_SX.at(p, EaseInOut), sy = SWAY_SY.at(p, EaseInOut), block = block,
            )
        }
        "sleep" -> {
            val p = cssProgress(t, 3600)
            css(0f, 0f, rot = -4f, sx = BREATHE_SX.at(p, EaseInOut), sy = BREATHE_SY.at(p, EaseInOut), block = block)
        }
        else -> {
            val p = cssProgress(t, 3000)
            css(
                0f, 0f, ty = BOB_Y.at(p, EaseInOut),
                sx = BOB_SX.at(p, EaseInOut), sy = BOB_SY.at(p, EaseInOut), block = block,
            )
        }
    }
}

private fun DrawScope.limb(cx: Float, cy: Float, rx: Float, ry: Float, pal: PetPalette, sw: Float) =
    ovalShape(cx, cy, rx, ry, pal.shade, pal.stroke, sw)

private fun DrawScope.drawCreatureParts(pet: PetState, pal: PetPalette, sw: Float, clock: PetClock) {
    val sp = pet.species
    val adult = pet.stage == "adult"
    val mood = pet.mood
    val act = pet.activity
    val t = clock.t
    val tailMs = when (act) {
        "play" -> 500
        "pet" -> 900
        "sleep" -> 6000
        else -> 2600
    }

    // ===== behind the body: per-species silhouette parts =====
    if (sp == "blob" && adult) {
        css(0f, -84f, rot = SPROUT_R.at(cssProgress(t, 3400, alternate = true), EaseInOut)) {
            line(PetPaths.SPROUT_STEM, Color(0xFF0E7A65), sw)
            shape(PetPaths.SPROUT_LEAF, Color(0xFF86E0A0), Color(0xFF0E7A65), sw)
            line(PetPaths.SPROUT_VEIN, Color(0xFF3FAE6A), sw * 0.5f)
        }
    }
    when (sp) {
        "cat" -> {
            css(36f, -12f, rot = TAIL_R.at(cssProgress(t, tailMs, alternate = true), EaseInOut)) {
                line(PetPaths.CAT_TAIL, pal.stroke, 11f)
                line(PetPaths.CAT_TAIL, pal.shade, 6.6f)
                line(PetPaths.CAT_TAIL_TIP, Color(0xFFF6EAD7), 6.6f)
            }
            val earIn = Color(0xFFF4B6C2)
            css(-27f, -76f, rot = EAR_L.at(cssProgress(t, 7000), EaseInOut)) {
                shape(PetPaths.CAT_EAR_L, pal.shade, pal.stroke, sw)
                shape(PetPaths.CAT_EAR_L_IN, earIn, earIn, 3f)
            }
            css(27f, -76f, rot = EAR_R.at(cssProgress(t, 5500, delayMs = 1200), EaseInOut)) {
                shape(PetPaths.CAT_EAR_R, pal.shade, pal.stroke, sw)
                shape(PetPaths.CAT_EAR_R_IN, earIn, earIn, 3f)
            }
        }
        "frog" -> {
            limb(-40f, -16f, 15f, 12f, pal, sw)
            limb(40f, -16f, 15f, 12f, pal, sw)
            limb(-32f, -3f, 15f, 4.4f, pal, sw)
            limb(32f, -3f, 15f, 4.4f, pal, sw)
        }
        "chick" -> {
            if (adult) {
                css(0f, -84f, rot = SPROUT_R.at(cssProgress(t, 3400, alternate = true), EaseInOut)) {
                    line(PetPaths.CHICK_CURL, Color(0xFFD9A521), 3.4f)
                }
            }
            for (s in intArrayOf(-1, 1)) {
                at(s * 17f, -1f) { shape(PetPaths.CHICK_FOOT, Color(0xFFF39C12), Color(0xFFC97A06), sw) }
            }
        }
        "axolotl" -> {
            val gillShade = Color(0xFFE06A92)
            val gillStroke = Color(0xFFB6527A)
            val gillHi = Color(0xFFF6A0BD)
            if (adult) {
                css(32f, -14f, rot = TAIL_R.at(cssProgress(t, tailMs, alternate = true), EaseInOut)) {
                    shape(PetPaths.AXO_TAIL, gillShade, gillStroke, sw)
                    line(PetPaths.AXO_FIN_RIB, gillHi, 3f)
                }
            }
            val gillMs = when (act) {
                "play" -> 600
                "pet" -> 1200
                "sleep" -> 5000
                else -> 2400
            }
            GILLS.forEachIndexed { i, g ->
                for (s in intArrayOf(-1, 1)) {
                    val delayMs = -500 * i - (if (s < 0) 800 else 0)
                    val p = cssProgress(t, gillMs, delayMs = delayMs, alternate = true)
                    at(s * g.x, g.y) {
                        scl(s.toFloat(), 1f) {
                            rot(g.r, 0f, 0f) {
                                css(
                                    0f, 0f, rot = GILL_R.at(p, EaseInOut),
                                    sx = GILL_SX.at(p, EaseInOut), sy = GILL_SY.at(p, EaseInOut),
                                ) {
                                    ovalShape(11f, 0f, 11.5f, 6f, gillShade, gillStroke, sw)
                                    oval(11.5f, -0.6f, 6.5f, 2f, gillHi)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // arms + feet sit behind the body (blob, cat, axolotl)
    if (sp == "blob" || sp == "cat" || sp == "axolotl") {
        rot(18f, -46f, -34f) { limb(-46f, -34f, 6f, 9.5f, pal, sw) }
        rot(-18f, 46f, -34f) { limb(46f, -34f, 6f, 9.5f, pal, sw) }
        limb(-21f, -3f, 11f, 4.2f, pal, sw)
        limb(21f, -3f, 11f, 4.2f, pal, sw)
    }

    // body: dark base + lighter, shifted copy = 2-tone shading
    val body = PetPaths.BODY.getValue(sp)
    shape(body, pal.shade, pal.stroke, sw)
    at(-3f, -2f) { scl(0.94f, 0.95f) { fill(body, pal.light) } }
    val belly = BELLY.getValue(sp)
    oval(belly[0], belly[1], belly[2], belly[3], pal.belly)
    val hi = HIGHLIGHT.getValue(sp)
    rot(-38f, hi[0], hi[1]) { oval(hi[0], hi[1], 9f, 4.6f, White, 0.28f) }

    // ===== on the body: per-species markings =====
    when (sp) {
        "cat" -> {
            if (adult) shape(PetPaths.CAT_RUFF, Color(0xFFFFFAF0), Color(0xFFE6D5B0), sw)
            oval(-6.5f, -41f, 9f, 7.5f, Color(0xFFF6EAD7))
            oval(6.5f, -41f, 9f, 7.5f, Color(0xFFF6EAD7))
            shape(PetPaths.CAT_NOSE, Color(0xFFE88EA0), Color(0xFFE88EA0), 2.4f)
        }
        "frog" -> {
            if (adult) {
                val spot = Color(0xFF3F6F22)
                drawCircle(spot, 5.5f, Offset(-38f, -44f), 0.45f)
                drawCircle(spot, 4f, Offset(-27f, -55f), 0.45f)
                drawCircle(spot, 6f, Offset(38f, -50f), 0.45f)
                drawCircle(spot, 3.6f, Offset(31f, -37f), 0.45f)
            }
            limb(-15f, -6f, 9f, 5f, pal, sw)
            limb(15f, -6f, 9f, 5f, pal, sw)
        }
        "chick" -> {
            for (s in intArrayOf(-1, 1)) {
                val left = s < 0
                val wingRot = when (act) {
                    "play" -> FLAP.at(cssProgress(t, 340, alternate = true), EaseInOut)
                    "pet" -> FLAP.at(cssProgress(t, 500, alternate = true), EaseInOut)
                    "sleep" -> 0f
                    else -> WING.at(cssProgress(t, 4000), EaseInOut)
                }
                css(if (left) -41f else 41f, -54f, rot = if (left) wingRot else -wingRot) {
                    scl(if (left) 1f else -1f, 1f) {
                        shape(PetPaths.CHICK_WING, pal.shade, pal.stroke, sw)
                        fill(PetPaths.CHICK_WING_PATCH, Color(0xFFFFFBE6))
                    }
                }
            }
        }
    }

    // frog: bump eyes on top of the head
    if (sp == "frog") {
        for (s in intArrayOf(-1, 1)) {
            drawCircle(Color(0xFF9CCC65), 13.5f, Offset(s * 18f, -76f))
            drawCircle(pal.stroke, 13.5f, Offset(s * 18f, -76f), style = Stroke(sw))
            drawCircle(Color(0xFFF4F8DA), 9.6f, Offset(s * 18f, -76f))
        }
    }

    // chick / axolotl: cheeks are always on
    if ((sp == "chick" || sp == "axolotl") && mood != "sick" && mood != "dead") {
        val a = if (sp == "axolotl") 0.55f else 0.75f
        val cx = if (sp == "chick") 30f else 34f
        oval(-cx, -45f, 6.5f, 3.8f, Blush, a)
        oval(cx, -45f, 6.5f, 3.8f, Blush, a)
    }

    // dirty: grime smudges on the body
    if (mood == "dirty") {
        val grime = Color(0xFF8B6F47)
        oval(-30f, -24f, 5f, 3.2f, grime, 0.5f)
        oval(31f, -60f, 4f, 2.6f, grime, 0.5f)
        oval(22f, -18f, 3f, 2f, grime, 0.5f)
    }

    // face, fading in after a mood change
    val fade = if (clock.face < 220) Ease(clock.face.coerceAtLeast(0) / 220f) else 1f
    group(fade) { drawFace(pet, pal, sw, t) }

    // cat: chunky whiskers poking past the silhouette
    if (sp == "cat") for (w in PetPaths.CAT_WHISKERS) line(w, Color(0xFF33475B), 3f)

    // dirty: stink lines rising
    if (mood == "dirty") {
        val stink = Color(0xFF8BC34A)
        for ((x, y, delay) in STINKS) {
            val p = cssProgress(t, 2600, delayMs = delay)
            at(x, y + STINK_TY.at(p, EaseInOut)) {
                line(PetPaths.STINK, stink, sw * 0.85f, STINK_A.at(p, EaseInOut))
            }
        }
    }

    // eat: crumbs pop near the mouth and fall
    if (act == "eat") {
        crumb(-9f, -41f, 3f, -7f, 0, Color(0xFFE8B36B), t)
        crumb(10f, -42f, 2.6f, 8f, 400, Color(0xFFC98F45), t)
        crumb(1f, -37f, 2.2f, 2f, 800, Color(0xFFE8B36B), t)
    }

    // pet: two hearts rise
    if (act == "pet") {
        heart(-38f, -78f, -4f, 0, t)
        heart(38f, -70f, 4f, 1100, t)
    }

    // sleep: drifting Z shapes
    if (act == "sleep") {
        for ((scale, delay) in listOf(1.7f to 0, 1.3f to 1200, 1f to 2400)) {
            val p = cssProgress(t, 3600, delayMs = delay)
            val s = scale * (0.6f + 0.4f * EaseOut(p))
            at(28f + Z_TX.at(p, EaseOut), -74f + Z_TY.at(p, EaseOut)) {
                scl(s) { line(PetPaths.Z, Color(0xFFCFE8F7), sw * 0.8f, Z_A.at(p, EaseOut)) }
            }
        }
    }
}

private fun DrawScope.crumb(x: Float, y: Float, r: Float, dx: Float, delay: Int, color: Color, t: Long) {
    val p = cssProgress(t, 1200, delayMs = delay)
    val tx = track(0f, 0f, 0.15f, dx * 0.4f, 1f, dx).at(p, EaseIn)
    at(x + tx, y + CRUMB_TY.at(p, EaseIn)) {
        scl(CRUMB_S.at(p, EaseIn)) {
            drawCircle(color, r, Offset.Zero, CRUMB_A.at(p, EaseIn).coerceIn(0f, 1f))
        }
    }
}

private fun DrawScope.heart(x: Float, y: Float, dx: Float, delay: Int, t: Long) {
    val p = cssProgress(t, 2200, delayMs = delay)
    val tx = track(0f, 0f, 0.2f, 0f, 1f, dx).at(p, EaseOut)
    at(x + tx, y + HEART_TY.at(p, EaseOut)) {
        scl(HEART_S.at(p, EaseOut) * 0.85f) {
            fill(PetPaths.HEART, Color(0xFFFF6B8B), HEART_A.at(p, EaseOut))
        }
    }
}

private fun DrawScope.drawFace(pet: PetState, pal: PetPalette, sw: Float, t: Long) {
    val sp = pet.species
    val mood = pet.mood
    val sleep = pet.activity == "sleep"
    val thin = sw * 0.7f
    val blink = BLINK.at(cssProgress(t, 4800), EaseInOut)
    val eyesY = if (sp == "frog") -22f else 0f

    // ---- eyes (frog: lifted onto the head bumps, with eyelids) ----
    at(0f, eyesY) {
        if (sp == "frog" && sleep && mood != "dead") {
            drawCircle(pal.shade, 12.6f, Offset(-18f, -54f))
            drawCircle(pal.shade, 12.6f, Offset(18f, -54f))
        }
        when {
            mood == "dead" -> {
                line(PetPaths.X_EYE_L, Ink, sw)
                line(PetPaths.X_EYE_R, Ink, sw)
            }
            sleep -> {
                line(PetPaths.SLEEP_EYE_L, Ink, sw)
                line(PetPaths.SLEEP_EYE_R, Ink, sw)
            }
            mood == "happy" -> {
                line(PetPaths.HAPPY_EYE_L, Ink, sw)
                line(PetPaths.HAPPY_EYE_R, Ink, sw)
            }
            mood == "sad" -> {
                css(0f, -54f, sy = blink) {
                    dot(-17f, -53f, 4.8f, Ink)
                    dot(17f, -53f, 4.8f, Ink)
                    dot(-15.4f, -54.6f, 1.5f, White)
                    dot(18.6f, -54.6f, 1.5f, White)
                }
                line(PetPaths.SAD_BROW_L, Ink, thin)
                line(PetPaths.SAD_BROW_R, Ink, thin)
            }
            mood == "hungry" -> {
                css(0f, -54f, sy = blink) {
                    fill(PetPaths.HUNGRY_EYE_L, Ink)
                    fill(PetPaths.HUNGRY_EYE_R, Ink)
                    dot(-14.8f, -55.2f, 2.1f, White)
                    dot(19.2f, -55.2f, 2.1f, White)
                    dot(-19.6f, -51.6f, 1f, White)
                    dot(14.4f, -51.6f, 1f, White)
                }
                line(PetPaths.HUNGRY_BROW_L, Ink, thin)
                line(PetPaths.HUNGRY_BROW_R, Ink, thin)
            }
            mood == "sleepy" -> {
                fill(PetPaths.SLEEPY_EYE_L, Ink)
                fill(PetPaths.SLEEPY_EYE_R, Ink)
                line(PetPaths.SLEEPY_LID_L, Ink, thin)
                line(PetPaths.SLEEPY_LID_R, Ink, thin)
            }
            mood == "dirty" -> {
                line(PetPaths.DIRTY_EYE_L, Ink, sw)
                line(PetPaths.DIRTY_EYE_R, Ink, sw)
            }
            mood == "sick" -> {
                val spin = 360f * cssProgress(t, 1600)
                at(-17f, -54f) { rot(spin, 0f, 0f) { line(PetPaths.SPIRAL, Ink, thin) } }
                at(17f, -54f) { rot(-spin, 0f, 0f) { line(PetPaths.SPIRAL, Ink, thin) } }
            }
            else -> css(0f, -54f, sy = blink) {
                dot(-17f, -54f, 5.2f, Ink)
                dot(17f, -54f, 5.2f, Ink)
                dot(-15.2f, -55.8f, 1.6f, White)
                dot(18.8f, -55.8f, 1.6f, White)
            }
        }
        // frog eyelids: blink by default, half shut when sleepy
        if (sp == "frog" && !sleep && mood != "dead") {
            val lid = if (mood == "sleepy") 0.5f else LID.at(cssProgress(t, 4800), EaseInOut)
            if (lid > 0.001f) {
                for (s in intArrayOf(-1, 1)) {
                    css(s * 18f, -66.6f, sy = lid) { drawCircle(pal.shade, 12.6f, Offset(s * 18f, -54f)) }
                }
            }
        }
    }

    // ---- mouth (scales open/shut while eating) ----
    val mouthY = when (sp) {
        "cat" -> 2f
        "frog" -> -2f
        else -> 0f
    }
    at(0f, mouthY) {
        if (pet.activity == "eat") {
            val p = cssProgress(t, 600)
            css(0f, -42f, sx = CHEW_SX.at(p, EaseInOut), sy = CHEW_SY.at(p, EaseInOut)) { drawMouth(sp, mood, sw) }
        } else {
            drawMouth(sp, mood, sw)
        }
    }

    // ---- mood extras ----
    when {
        mood == "happy" && sp != "chick" && sp != "axolotl" -> {
            oval(-31f, -45f, 6f, 3.5f, Blush, 0.75f)
            oval(31f, -45f, 6f, 3.5f, Blush, 0.75f)
        }
        mood == "sad" -> at(-24f, -47f + eyesY) { tear(t, 2400) }
        mood == "hungry" -> {
            at(5.2f, -39.5f) {
                scl(0.55f) {
                    val p = cssProgress(t, 1800)
                    at(0f, DROOL_TY.at(p, EaseIn)) {
                        scl(1f, DROOL_SY.at(p, EaseIn)) { fill(PetPaths.DROP, Drop, DROOL_A.at(p, EaseIn)) }
                    }
                }
            }
            at(RUMBLE_X.at(cssProgress(t, 450, alternate = true), EaseInOut), 0f) {
                line(PetPaths.RUMBLE, pal.shade, sw * 0.65f)
            }
        }
        mood == "sick" -> {
            val patch = Color(0xFFB8E08F)
            oval(29f, -46f, 11f, 8f, patch, 0.62f)
            oval(-30f, -44f, 6.5f, 4.5f, patch, 0.62f)
            at(33f, -70f) { scl(0.6f) { tear(t, 3000) } }
        }
    }
}

/** `.tam-drop-fall`: a drop swells, slides down and fades. */
private fun DrawScope.tear(t: Long, durMs: Int) {
    val p = cssProgress(t, durMs)
    at(0f, TEAR_TY.at(p, EaseIn)) {
        scl(TEAR_S.at(p, EaseIn)) { fill(PetPaths.DROP, Drop, TEAR_A.at(p, EaseIn)) }
    }
}

private fun DrawScope.drawMouth(sp: String, mood: String, sw: Float) {
    val tongue = Blush
    when (sp) {
        // chick: the beak IS the mouth (opens when happy / hungry)
        "chick" -> {
            val beak = Color(0xFFF39C12)
            val beakLine = Color(0xFFC97A06)
            if (mood == "happy" || mood == "hungry") {
                oval(0f, -43.4f, 3.8f, 2.6f, Ink)
                shape(PetPaths.BEAK_TOP, beak, beakLine, 2f)
                shape(PetPaths.BEAK_BOTTOM, beak, beakLine, 2f)
            } else {
                shape(PetPaths.BEAK_SHUT, beak, beakLine, 2f)
            }
        }
        // frog: wide mouth line across the whole head
        "frog" -> when (mood) {
            "dead" -> line(PetPaths.FROG_DEAD, Ink, sw)
            "happy" -> {
                fill(PetPaths.FROG_HAPPY, Ink)
                oval(0f, -39.4f, 5.5f, 2.4f, tongue)
            }
            "sad" -> line(PetPaths.FROG_SAD, Ink, sw)
            "hungry" -> oval(0f, -41f, 9f, 5f, Ink)
            "sleepy" -> line(PetPaths.FROG_SLEEPY, Ink, sw)
            "dirty" -> line(PetPaths.FROG_DIRTY, Ink, sw)
            "sick" -> line(PetPaths.FROG_SICK, Ink, sw)
            else -> line(PetPaths.FROG_NEUTRAL, Ink, sw)
        }
        // blob, cat, axolotl
        else -> when {
            mood == "dead" -> line(PetPaths.MOUTH_DEAD, Ink, sw)
            mood == "happy" && sp == "axolotl" -> {
                fill(PetPaths.AXO_HAPPY, Ink)
                oval(0f, -40f, 5f, 2.3f, tongue)
            }
            mood == "happy" -> {
                fill(PetPaths.MOUTH_HAPPY, Ink)
                oval(0f, -41.2f, 3.6f, 1.8f, tongue)
            }
            mood == "sad" -> line(PetPaths.MOUTH_SAD, Ink, sw)
            mood == "hungry" -> oval(0f, -42f, 3.4f, 3.2f, Ink)
            mood == "sleepy" && sp == "axolotl" -> line(PetPaths.AXO_SLEEPY, Ink, sw)
            mood == "sleepy" -> oval(0f, -42f, 3f, 2.6f, Ink)
            mood == "dirty" -> line(PetPaths.MOUTH_DIRTY, Ink, sw)
            mood == "sick" -> line(PetPaths.MOUTH_SICK, Ink, sw)
            // neutral: axolotls always look pleased
            sp == "axolotl" -> line(PetPaths.AXO_NEUTRAL, Ink, sw)
            sp == "cat" -> line(PetPaths.CAT_NEUTRAL, Ink, sw)
            else -> line(PetPaths.MOUTH_NEUTRAL, Ink, sw)
        }
    }
}
