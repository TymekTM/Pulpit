package app.pulpit.mobile.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.PathParser
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure helpers of the desk pet tile (PetTiles.kt) and its sprite port
 *  (PetSprite.kt). */
class PetTilesTest {

    @Test
    fun paletteMatchesTheDesktopSprite() {
        // TamagotchiSprite.vue: blob --c-light #1abc9c, cat #8fa6bd, ...
        assertEquals(Color(0xFF1ABC9C), petPalette("blob").light)
        assertEquals(Color(0xFF8FA6BD), petPalette("cat").light)
        assertEquals(Color(0xFF7CB342), petPalette("frog").light)
        assertEquals(Color(0xFFF7D154), petPalette("chick").light)
        assertEquals(Color(0xFFF19BB4), petPalette("axolotl").light)
        // unknown species degrade to the blob palette
        assertEquals(petPalette("blob").shade, petPalette("dragon").shade)
        assertEquals(Color(0xFF1ABC9C), petAccent("dragon")[0])
    }

    @Test
    fun strokeAndScaleFollowTheStage() {
        assertEquals(1.5f, petCreatureScale("adult"), 0f)
        assertEquals(1.1f, petCreatureScale("baby"), 0f)
        assertEquals(1.9f, petStrokeWidth("adult"), 0f)
        assertEquals(2.6f, petStrokeWidth("baby"), 0f)
        assertEquals(2.2f, petStrokeWidth("dead"), 0f)
    }

    @Test
    fun branchesMatchTheTemplate() {
        assertEquals("egg", petSpriteBranch("egg"))
        assertEquals("hatching", petSpriteBranch("hatching"))
        assertEquals("dead", petSpriteBranch("dead"))
        assertEquals("creature", petSpriteBranch("baby"))
        assertEquals("creature", petSpriteBranch("adult"))
    }

    @Test
    fun everySpritePathParses() {
        assertTrue(PetPaths.all.size > 60)
        for (d in PetPaths.all) {
            val nodes = PathParser().parsePathString(d).toNodes()
            assertTrue("no nodes for $d", nodes.isNotEmpty())
        }
    }

    @Test
    fun cubicBezierEndsAndMiddle() {
        assertEquals(0f, EaseInOut(0f), 0f)
        assertEquals(1f, EaseInOut(1f), 0f)
        assertEquals(0.5f, EaseInOut(0.5f), 0.002f)
        assertTrue(EaseIn(0.5f) < 0.5f)
        assertTrue(EaseOut(0.5f) > 0.5f)
        var last = 0f
        for (i in 1..20) {
            val v = Ease(i / 20f)
            assertTrue(v >= last)
            last = v
        }
    }

    @Test
    fun trackHitsKeyframesAndInterpolates() {
        val tr = track(0f, 0f, 0.5f, 10f, 1f, 0f)
        assertEquals(0f, tr.at(0f, Linear), 0f)
        assertEquals(10f, tr.at(0.5f, Linear), 0f)
        assertEquals(5f, tr.at(0.25f, Linear), 0.0001f)
        assertEquals(5f, tr.at(0.75f, Linear), 0.0001f)
        assertEquals(0f, tr.at(1f, Linear), 0f)
    }

    @Test
    fun cssProgressHandlesDelayAndDirection() {
        assertEquals(0.25f, cssProgress(250, 1000), 0f)
        assertEquals(0.25f, cssProgress(1250, 1000), 0f)
        // alternate: the second cycle runs backwards
        assertEquals(0.75f, cssProgress(1250, 1000, alternate = true), 0f)
        // a positive delay holds the first keyframe, a negative one skips ahead
        assertEquals(0f, cssProgress(100, 1000, delayMs = 500), 0f)
        assertEquals(0.5f, cssProgress(0, 1000, delayMs = -500), 0f)
        assertEquals(0.75f, cssProgress(250, 1000, reverse = true), 0f)
    }

    @Test
    fun statWarnsBelowTwenty() {
        assertTrue(petStatLow(19))
        assertFalse(petStatLow(20))
        assertFalse(petStatLow(100))
    }

    @Test
    fun missingOrNonObjectStateFallsBackToEgg() {
        assertEquals(PetState(), parsePetState(null))
        assertEquals("egg", parsePetState(JsonPrimitive("Pet: hungry")).stage)
    }

    @Test
    fun parsesPushedSnapshotAndClamps() {
        val live = Json.parseToJsonElement(
            """{"species":"axolotl","stage":"adult","mood":"hungry","activity":"idle",
               "poos":7,"sleeping":false,"food":12,"fun":150,"rest":-3,"clean":55,
               "ageMs":1000,"generations":2,"label":"x"}""",
        )
        val pet = parsePetState(live)
        assertEquals("axolotl", pet.species)
        assertEquals("adult", pet.stage)
        assertEquals("hungry", pet.mood)
        assertEquals("idle", pet.activity)
        assertEquals(3, pet.poos)
        assertEquals(12, pet.food)
        assertEquals(100, pet.fun_)
        assertEquals(0, pet.rest)
        assertEquals(55, pet.clean)
    }

    @Test
    fun unknownEnumsDegrade() {
        val pet = parsePetState(
            Json.parseToJsonElement("""{"species":"dog","stage":"teen","mood":"grumpy","activity":"fly"}"""),
        )
        assertEquals("blob", pet.species)
        assertEquals("egg", pet.stage)
        assertEquals("neutral", pet.mood)
        assertEquals("idle", pet.activity)
    }

    @Test
    fun activityFallsBackToTheSleepFlag() {
        val pet = parsePetState(Json.parseToJsonElement("""{"stage":"baby","sleeping":true}"""))
        assertEquals("sleep", pet.activity)
    }

    @Test
    fun zzzShowsOnlyForASleepingCreature() {
        assertTrue(petShowsZzz(PetState(stage = "adult", activity = "sleep")))
        assertTrue(petShowsZzz(PetState(stage = "baby", activity = "sleep")))
        assertFalse(petShowsZzz(PetState(stage = "baby", mood = "sleepy")))
        assertFalse(petShowsZzz(PetState(stage = "egg", activity = "sleep")))
        assertFalse(petShowsZzz(PetState(stage = "dead", activity = "sleep")))
    }
}
