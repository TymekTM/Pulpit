//! Desk pet tile (catalog type "pet", widget hint "pet"). The pet is global:
//! every pet tile renders the same animal from the compact snapshot pushed on
//! the `ext.pet` state channel. The rules live server-side, so this file only
//! draws what the payload says (see apps/desktop PetTile.vue for the contract)
//! and forwards gesture names.

package app.pulpit.mobile.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.pulpit.mobile.proto.Tile
import kotlin.math.min
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull

/** The pet snapshot fields the renderer reads; everything else on the
 *  payload (`label`, `ageMs`, `generations`, `activity`) is ignored on v2. */
data class PetState(
    val species: String = "blob",
    val stage: String = "egg",
    val mood: String = "neutral",
    val poos: Int = 0,
    val sleeping: Boolean = false,
    val food: Int = 100,
    val fun_: Int = 100,
    val rest: Int = 100,
    val clean: Int = 100,
)

private val PET_SPECIES = setOf("blob", "cat", "frog", "chick", "axolotl")
private val PET_STAGES = setOf("egg", "hatching", "baby", "adult", "dead")

/** Parses the pushed snapshot. Null or a non-object (nothing pushed yet, a
 *  legacy plain-text label) yields the egg fallback; unknown enum values
 *  degrade to blob / egg and numbers are clamped to their ranges. */
fun parsePetState(live: JsonElement?): PetState {
    val obj = live as? JsonObject ?: return PetState()
    fun text(key: String): String? = (obj[key] as? JsonPrimitive)?.takeIf { it.isString }?.content
    fun num(key: String): Double? = (obj[key] as? JsonPrimitive)?.doubleOrNull
    fun stat(key: String): Int = (num(key) ?: 100.0).coerceIn(0.0, 100.0).toInt()
    return PetState(
        species = text("species")?.takeIf { it in PET_SPECIES } ?: "blob",
        stage = text("stage")?.takeIf { it in PET_STAGES } ?: "egg",
        mood = text("mood") ?: "neutral",
        poos = (num("poos") ?: 0.0).coerceIn(0.0, 3.0).toInt(),
        sleeping = (obj["sleeping"] as? JsonPrimitive)?.booleanOrNull ?: false,
        food = stat("food"),
        fun_ = stat("fun"),
        rest = stat("rest"),
        clean = stat("clean"),
    )
}

/** Body color per species (ARGB); unknown species fall back to blob. */
fun petSpeciesColor(species: String): Long = when (species) {
    "cat" -> 0xFF9AA0A6
    "frog" -> 0xFF5CB85C
    "chick" -> 0xFFF4D03F
    "axolotl" -> 0xFFF5A3C0
    else -> 0xFF1ABC9C // blob
}

/** Desaturated body of a dead pet. */
internal const val PET_DEAD_COLOR = 0xFF7F858C

/** Size of the silhouette relative to an adult: a baby is ~60%, an egg and
 *  a hatching egg sit between baby and adult as plain ovals. */
fun petStageScale(stage: String): Float = when (stage) {
    "adult" -> 1f
    "baby" -> 0.6f
    "hatching" -> 0.8f
    "dead" -> 0.9f
    else -> 0.75f // egg
}

/** A stat bar tints warning below this value. */
internal const val PET_LOW_STAT = 20

fun petStatLow(value: Int): Boolean = value < PET_LOW_STAT

/** Whether the "Zzz" shows: asleep, or in the sleepy mood (not when dead). */
fun petShowsZzz(pet: PetState): Boolean =
    pet.stage != "dead" && (pet.sleeping || pet.mood == "sleepy")

private val PetBarOk = Color(0xFF1ABC9C)
private val PetBarLow = Color(0xFFF1C40F)
private val PetBarTrack = Color.Black.copy(alpha = 0.28f)
private val PetEgg = Color(0xFFF3EEDD)
private val PetEggSpot = Color(0xFFD9CFB4)
private val PetPoo = Color(0xFF7A4B28)
private val PetInk = Color(0xFF2B2B33)

/** Native desk pet tile (widget "pet"). Same gesture wiring as the tool
 *  tiles: tap, double-tap and long-press go to the server by name, which
 *  decides what they mean (feed / play / clean or a new egg). */
@Composable
fun PetTile(
    tile: Tile,
    live: JsonElement?,
    titleColor: Color,
    onPress: () -> Unit,
    onGesture: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val pet = parsePetState(live)
    val dead = pet.stage == "dead"

    Box(
        modifier = modifier
            .fillMaxSize()
            .pointerInput(tile.id) {
                detectTapGestures(
                    onTap = { onPress() },
                    onDoubleTap = { onGesture("double-tap") },
                    onLongPress = { onGesture("long-press") },
                )
            },
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier
                .fillMaxSize()
                .padding(start = 10.dp, end = 10.dp, top = 8.dp, bottom = 8.dp),
        ) {
            Box(Modifier.weight(1f).fillMaxWidth()) {
                Canvas(Modifier.fillMaxSize()) { drawPet(pet) }
                if (dead) {
                    Text(
                        text = "R.I.P.",
                        color = titleColor.copy(alpha = 0.8f),
                        fontSize = 14.sp,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.align(Alignment.TopCenter),
                    )
                }
                if (petShowsZzz(pet)) {
                    Text(
                        text = "Zzz",
                        color = titleColor,
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.align(Alignment.TopEnd),
                    )
                }
            }
            if (!dead) {
                Spacer(Modifier.height(4.dp))
                Row(
                    modifier = Modifier.fillMaxWidth().height(5.dp),
                    horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(4.dp),
                ) {
                    PetBar(pet.food, Modifier.weight(1f))
                    PetBar(pet.fun_, Modifier.weight(1f))
                    PetBar(pet.rest, Modifier.weight(1f))
                    PetBar(pet.clean, Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun PetBar(value: Int, modifier: Modifier) {
    val shape = RoundedCornerShape(50)
    Box(modifier.fillMaxHeight().background(PetBarTrack, shape)) {
        Box(
            Modifier
                .fillMaxHeight()
                .fillMaxWidth(value.coerceIn(0, 100) / 100f)
                .background(if (petStatLow(value)) PetBarLow else PetBarOk, shape),
        )
    }
}

/** Draws the animal bottom-aligned in the scope: silhouette per stage,
 *  species features, mood face, then the poos at the bottom corners. */
private fun DrawScope.drawPet(pet: PetState) {
    val w = size.width
    val h = size.height
    if (w <= 0f || h <= 0f) return
    val adultR = min(w, h) * 0.4f
    val r = adultR * petStageScale(pet.stage)
    val cx = w / 2f
    val baseY = h * 0.97f
    val dead = pet.stage == "dead"
    val isEgg = pet.stage == "egg" || pet.stage == "hatching"
    val body = Color(if (dead) PET_DEAD_COLOR else petSpeciesColor(pet.species))

    if (isEgg) {
        // oval, taller than wide
        val ew = r * 0.8f
        val eh = r * 1.05f
        val ecy = baseY - eh
        drawOval(PetEgg, Offset(cx - ew, ecy - eh), Size(ew * 2, eh * 2))
        drawCircle(PetEggSpot, r * 0.12f, Offset(cx - ew * 0.35f, ecy - eh * 0.2f))
        drawCircle(PetEggSpot, r * 0.09f, Offset(cx + ew * 0.4f, ecy + eh * 0.25f))
        if (pet.stage == "hatching") {
            val crack = Path().apply {
                moveTo(cx - ew * 0.7f, ecy - eh * 0.25f)
                lineTo(cx - ew * 0.3f, ecy - eh * 0.05f)
                lineTo(cx, ecy - eh * 0.3f)
                lineTo(cx + ew * 0.3f, ecy - eh * 0.05f)
                lineTo(cx + ew * 0.7f, ecy - eh * 0.25f)
                moveTo(cx, ecy - eh * 0.3f)
                lineTo(cx + ew * 0.1f, ecy - eh * 0.7f)
            }
            drawPath(crack, PetInk.copy(alpha = 0.7f), style = Stroke(r * 0.07f, cap = StrokeCap.Round))
        }
    } else {
        val bcy = baseY - r * 0.9f
        drawCreature(pet, body, cx, bcy, r)
        drawFace(pet, cx, bcy, r)
    }
    drawPoos(pet.poos, w, h, adultR)
}

private fun DrawScope.drawCreature(pet: PetState, body: Color, cx: Float, cy: Float, r: Float) {
    val dead = pet.stage == "dead"
    // appendages go behind the body
    if (!dead) {
        when (pet.species) {
            "cat" -> {
                for (dir in listOf(-1f, 1f)) {
                    val ear = Path().apply {
                        moveTo(cx + dir * r * 0.9f, cy - r * 0.35f)
                        lineTo(cx + dir * r * 0.75f, cy - r * 1.25f)
                        lineTo(cx + dir * r * 0.2f, cy - r * 0.85f)
                        close()
                    }
                    drawPath(ear, body)
                }
            }
            "frog" -> {
                for (dir in listOf(-1f, 1f)) {
                    drawCircle(body, r * 0.28f, Offset(cx + dir * r * 0.5f, cy - r * 0.85f))
                }
            }
            "axolotl" -> {
                val gill = Color(0xFFE0457B)
                for (dir in listOf(-1f, 1f)) {
                    for (i in 0..2) {
                        val y = cy - r * 0.35f + i * r * 0.22f
                        drawLine(
                            gill,
                            Offset(cx + dir * r * 0.85f, y),
                            Offset(cx + dir * r * 1.3f, y - r * 0.2f),
                            strokeWidth = r * 0.1f,
                            cap = StrokeCap.Round,
                        )
                    }
                }
            }
        }
    }
    drawOval(body, Offset(cx - r, cy - r * 0.9f), Size(r * 2, r * 1.8f))
    if (!dead && pet.species == "chick") {
        // beak: filled in drawFace's mouth slot, so it is drawn here
        val beak = Path().apply {
            moveTo(cx - r * 0.2f, cy + r * 0.05f)
            lineTo(cx + r * 0.2f, cy + r * 0.05f)
            lineTo(cx, cy + r * 0.3f)
            close()
        }
        drawPath(beak, Color(0xFFF08A24))
    }
}

private fun DrawScope.drawFace(pet: PetState, cx: Float, cy: Float, r: Float) {
    val eyeDx = r * 0.4f
    val eyeY = cy - r * 0.2f
    val eyeR = r * 0.1f
    val stroke = Stroke(r * 0.075f, cap = StrokeCap.Round)
    val dead = pet.stage == "dead"
    val closed = !dead && (pet.sleeping || pet.mood == "sleepy")
    for (dir in listOf(-1f, 1f)) {
        val ex = cx + dir * eyeDx
        when {
            dead || pet.mood == "dead" -> {
                val k = eyeR * 1.1f
                drawLine(PetInk, Offset(ex - k, eyeY - k), Offset(ex + k, eyeY + k), r * 0.075f, StrokeCap.Round)
                drawLine(PetInk, Offset(ex - k, eyeY + k), Offset(ex + k, eyeY - k), r * 0.075f, StrokeCap.Round)
            }
            closed -> drawLine(
                PetInk,
                Offset(ex - eyeR * 1.2f, eyeY),
                Offset(ex + eyeR * 1.2f, eyeY),
                r * 0.075f,
                StrokeCap.Round,
            )
            pet.mood == "happy" -> drawArc(
                PetInk,
                startAngle = 200f,
                sweepAngle = 140f,
                useCenter = false,
                topLeft = Offset(ex - eyeR * 1.4f, eyeY - eyeR * 1.2f),
                size = Size(eyeR * 2.8f, eyeR * 2.8f),
                style = stroke,
            )
            else -> drawCircle(PetInk, eyeR, Offset(ex, eyeY))
        }
    }
    // mouth (the chick's beak already sits in that spot)
    val mouthY = cy + r * 0.3f
    val mouthW = r * 0.5f
    when {
        dead || pet.mood == "dead" ->
            drawLine(PetInk, Offset(cx - mouthW * 0.5f, mouthY + r * 0.1f), Offset(cx + mouthW * 0.5f, mouthY + r * 0.1f), r * 0.075f, StrokeCap.Round)
        closed -> {}
        pet.species == "chick" -> {}
        pet.mood == "happy" -> drawArc(
            PetInk, 20f, 140f, false,
            Offset(cx - mouthW / 2, mouthY - mouthW * 0.35f), Size(mouthW, mouthW * 0.7f), style = stroke,
        )
        pet.mood == "sad" -> drawArc(
            PetInk, 200f, 140f, false,
            Offset(cx - mouthW / 2, mouthY), Size(mouthW, mouthW * 0.7f), style = stroke,
        )
        pet.mood == "hungry" ->
            drawOval(PetInk, Offset(cx - r * 0.1f, mouthY - r * 0.05f), Size(r * 0.2f, r * 0.26f))
        else ->
            drawLine(PetInk, Offset(cx - mouthW * 0.35f, mouthY + r * 0.05f), Offset(cx + mouthW * 0.35f, mouthY + r * 0.05f), r * 0.075f, StrokeCap.Round)
    }
}

/** Up to [count] small brown swirls along the bottom edge, outside the
 *  body's center so they read even on an adult. */
private fun DrawScope.drawPoos(count: Int, w: Float, h: Float, adultR: Float) {
    val xs = floatArrayOf(0.1f, 0.9f, 0.22f)
    val u = adultR * 0.16f
    for (i in 0 until count.coerceIn(0, xs.size)) {
        val x = w * xs[i]
        val y = h * 0.97f
        drawOval(PetPoo, Offset(x - u * 1.5f, y - u * 1.1f), Size(u * 3f, u * 1.1f))
        drawOval(PetPoo, Offset(x - u * 1.0f, y - u * 2.0f), Size(u * 2f, u * 1.0f))
        drawOval(PetPoo, Offset(x - u * 0.5f, y - u * 2.8f), Size(u * 1f, u * 0.9f))
    }
}
