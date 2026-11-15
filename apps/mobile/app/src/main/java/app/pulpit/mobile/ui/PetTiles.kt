//! Desk pet tile (catalog type "pet", widget hint "pet"). The pet is global:
//! every pet tile renders the same animal from the compact snapshot pushed on
//! the `ext.pet` state channel. The rules live server-side, so this file only
//! draws what the payload says (see apps/desktop PetTile.vue for the contract)
//! and forwards gesture names. The animal itself is PetSprite.kt, a port of
//! the desktop sprite.

package app.pulpit.mobile.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.pulpit.mobile.proto.Tile
import kotlin.math.min
import kotlinx.coroutines.isActive
import androidx.compose.runtime.withFrameNanos
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull

/** The pet snapshot fields the renderer reads; everything else on the
 *  payload (`label`, `ageMs`, `generations`) is ignored on v2. */
data class PetState(
    val species: String = "blob",
    val stage: String = "egg",
    val mood: String = "neutral",
    val activity: String = "idle",
    val poos: Int = 0,
    val sleeping: Boolean = false,
    val food: Int = 100,
    val fun_: Int = 100,
    val rest: Int = 100,
    val clean: Int = 100,
)

private val PET_SPECIES = setOf("blob", "cat", "frog", "chick", "axolotl")
private val PET_STAGES = setOf("egg", "hatching", "baby", "adult", "dead")
private val PET_MOODS = setOf("happy", "neutral", "sad", "hungry", "sleepy", "dirty", "sick", "dead")
private val PET_ACTIVITIES = setOf("idle", "eat", "play", "pet", "sleep")

/** Parses the pushed snapshot. Null or a non-object (nothing pushed yet, a
 *  legacy plain-text label) yields the egg fallback; unknown enum values
 *  degrade the way the desktop sprite does (blob / egg / neutral / idle,
 *  or sleep when only `sleeping` says so) and numbers are clamped. */
fun parsePetState(live: JsonElement?): PetState {
    val obj = live as? JsonObject ?: return PetState()
    fun text(key: String): String? = (obj[key] as? JsonPrimitive)?.takeIf { it.isString }?.content
    fun num(key: String): Double? = (obj[key] as? JsonPrimitive)?.doubleOrNull
    fun stat(key: String): Int = (num(key) ?: 100.0).coerceIn(0.0, 100.0).toInt()
    val sleeping = (obj["sleeping"] as? JsonPrimitive)?.booleanOrNull ?: false
    return PetState(
        species = text("species")?.takeIf { it in PET_SPECIES } ?: "blob",
        stage = text("stage")?.takeIf { it in PET_STAGES } ?: "egg",
        mood = text("mood")?.takeIf { it in PET_MOODS } ?: "neutral",
        activity = text("activity")?.takeIf { it in PET_ACTIVITIES } ?: if (sleeping) "sleep" else "idle",
        poos = (num("poos") ?: 0.0).coerceIn(0.0, 3.0).toInt(),
        sleeping = sleeping,
        food = stat("food"),
        fun_ = stat("fun"),
        rest = stat("rest"),
        clean = stat("clean"),
    )
}

/** A stat bar tints warning below this value. */
internal const val PET_LOW_STAT = 20

fun petStatLow(value: Int): Boolean = value < PET_LOW_STAT

/** Whether the drifting Z's show: only a sleeping creature has them, like
 *  the desktop sprite (an egg, a ghost or a merely sleepy face does not). */
fun petShowsZzz(pet: PetState): Boolean =
    petSpriteBranch(pet.stage) == "creature" && pet.activity == "sleep"

private val PetBarOk = Color(0xFF1ABC9C)
private val PetBarLow = Color(0xFFF1C40F)
private val PetBarTrack = Color.Black.copy(alpha = 0.28f)

/** Native desk pet tile (widget "pet"). Same gesture wiring as the tool
 *  tiles: tap, double-tap and long-press go to the server by name, which
 *  decides what they mean (feed / play / clean or a new egg). Layout
 *  follows the desktop PetTile.vue: the sprite in a 78% wide screen above
 *  four stat bars, "R.I.P." over a ghost. */
@Composable
fun PetTile(
    tile: Tile,
    live: JsonElement?,
    onPress: () -> Unit,
    onGesture: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val pet = parsePetState(live)
    val dead = pet.stage == "dead"

    // one frame clock for every keyframe; it is read only while drawing,
    // so each frame redraws the canvas without recomposing the tile
    val born = remember { System.nanoTime() }
    val frame = remember { mutableLongStateOf(born) }
    LaunchedEffect(Unit) {
        while (isActive) withFrameNanos { frame.longValue = it }
    }
    // the pop-in replays when the egg / hatching / ghost / creature branch
    // changes, the face fades in when the mood or sleep changes
    val enteredAt = remember(petSpriteBranch(pet.stage)) { System.nanoTime() }
    val faceAt = remember(pet.mood, pet.activity == "sleep") { System.nanoTime() }

    BoxWithConstraints(
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
        // the desktop pads 8% / 10% / 6% of the tile width
        val w = maxWidth
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp, Alignment.Bottom),
            modifier = Modifier
                .fillMaxSize()
                .padding(start = w * 0.1f, end = w * 0.1f, top = w * 0.08f, bottom = w * 0.06f),
        ) {
            BoxWithConstraints(Modifier.weight(1f).fillMaxWidth(0.78f)) {
                Canvas(Modifier.fillMaxSize()) {
                    val now = frame.longValue
                    val clock = PetClock(
                        t = (now - born).coerceAtLeast(0) / 1_000_000,
                        enter = (now - enteredAt).coerceAtLeast(0) / 1_000_000,
                        face = (now - faceAt).coerceAtLeast(0) / 1_000_000,
                    )
                    // viewBox 0 0 200 200, square, bottom-centered
                    val side = min(size.width, size.height)
                    withTransform({
                        translate((size.width - side) / 2f, size.height - side)
                        scale(side / 200f, side / 200f, Offset.Zero)
                    }) { drawPetSprite(pet, clock) }
                }
                if (dead) {
                    val ripSp = (maxWidth.value * 0.16f).coerceIn(10f, 18f)
                    val shadowPx = with(LocalDensity.current) { 1.dp.toPx() }
                    Text(
                        text = "R.I.P.",
                        style = TextStyle(
                            color = Color.White.copy(alpha = 0.75f),
                            fontSize = ripSp.sp,
                            letterSpacing = 0.12.em,
                            shadow = Shadow(Color.Black.copy(alpha = 0.4f), Offset(0f, shadowPx), 2f * shadowPx),
                        ),
                        modifier = Modifier.align(Alignment.TopCenter).padding(top = maxHeight * 0.06f),
                    )
                }
            }
            if (!dead) {
                Row(
                    modifier = Modifier.fillMaxWidth().height(4.dp),
                    horizontalArrangement = Arrangement.spacedBy(w * 0.8f * 0.06f),
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
