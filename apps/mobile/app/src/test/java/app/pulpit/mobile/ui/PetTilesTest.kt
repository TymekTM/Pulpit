package app.pulpit.mobile.ui

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure helpers of the desk pet tile (PetTiles.kt). */
class PetTilesTest {

    @Test
    fun everySpeciesHasItsOwnColor() {
        val colors = listOf("blob", "cat", "frog", "chick", "axolotl").map { petSpeciesColor(it) }
        assertEquals(colors.size, colors.toSet().size)
        // unknown species degrade to the blob teal
        assertEquals(petSpeciesColor("blob"), petSpeciesColor("dragon"))
    }

    @Test
    fun stageScaleOrdersBabyBelowAdult() {
        assertEquals(1f, petStageScale("adult"), 0f)
        assertEquals(0.6f, petStageScale("baby"), 0f)
        assertTrue(petStageScale("egg") < petStageScale("adult"))
        assertTrue(petStageScale("hatching") > petStageScale("baby"))
        assertNotEquals(petStageScale("baby"), petStageScale("adult"))
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
        assertEquals(3, pet.poos)
        assertEquals(12, pet.food)
        assertEquals(100, pet.fun_)
        assertEquals(0, pet.rest)
        assertEquals(55, pet.clean)
    }

    @Test
    fun unknownEnumsDegrade() {
        val pet = parsePetState(Json.parseToJsonElement("""{"species":"dog","stage":"teen"}"""))
        assertEquals("blob", pet.species)
        assertEquals("egg", pet.stage)
    }

    @Test
    fun zzzShowsWhenAsleepOrSleepyButNotDead() {
        assertTrue(petShowsZzz(PetState(stage = "adult", sleeping = true)))
        assertTrue(petShowsZzz(PetState(stage = "baby", mood = "sleepy")))
        assertFalse(petShowsZzz(PetState(stage = "adult", mood = "happy")))
        assertFalse(petShowsZzz(PetState(stage = "dead", sleeping = true)))
    }
}
