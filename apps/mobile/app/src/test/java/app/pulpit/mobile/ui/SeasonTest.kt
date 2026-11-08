//! Seasonal themes on the tablet: the `ext.season` channel decides the
//! skin, and the particle/burst math stays inside the screen.

package app.pulpit.mobile.ui

import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SeasonTest {
    @Test
    fun everyWireNameMapsToItsSeason() {
        val on = { v: String -> seasonOf(mapOf(SEASON_CHANNEL to JsonPrimitive(v))) }
        assertEquals(Season.CHRISTMAS, on("christmas"))
        assertEquals(Season.HALLOWEEN, on("halloween"))
        assertEquals(Season.EASTER, on("easter"))
        assertEquals(Season.VALENTINES, on("valentines"))
        assertNull(on("off"))
        assertNull(on("ramadan"))
        assertNull(seasonOf(mapOf(SEASON_CHANNEL to JsonNull)))
        assertNull(seasonOf(mapOf(SEASON_CHANNEL to JsonPrimitive(true))))
        assertNull(seasonOf(emptyMap()))
        // the bare key (legacy lane name) is not the v2 channel
        assertNull(seasonOf(mapOf("season" to JsonPrimitive("christmas"))))
    }

    @Test
    fun theSkyIsFixedBySeed() {
        assertEquals(makeFlakes(20), makeFlakes(20))
        for (season in Season.entries) {
            val look = season.ambient
            val flakes = makeFlakes(34, look = look)
            assertEquals(Math.round(34 * look.share), flakes.size)
            for (f in flakes) {
                assertTrue(f.x in 0f..1f)
                assertTrue(f.speed > 0f)
                assertTrue(f.radiusDp in look.sizeDp)
                assertTrue(f.color in look.colors.indices)
            }
        }
    }

    @Test
    fun noSeasonAccentIsRed() {
        // red means "error" in the design system (PulpitColors.danger):
        // every accent keeps its hue well away from red
        for (s in Season.entries) {
            val c = s.accent
            val max = maxOf(c.red, c.green, c.blue)
            val min = minOf(c.red, c.green, c.blue)
            val d = max - min
            val hue = when (max) {
                c.red -> 60f * (((c.green - c.blue) / d) % 6f)
                c.green -> 60f * ((c.blue - c.red) / d + 2f)
                else -> 60f * ((c.red - c.green) / d + 4f)
            }.let { if (it < 0f) it + 360f else it }
            val fromRed = minOf(hue, 360f - hue)
            assertTrue("${s.name} hue $hue", fromRed >= 25f)
            assertTrue(s.name, c != PulpitColors.danger)
        }
    }

    @Test
    fun flakesWrapInsideTheBoard() {
        for (f in makeFlakes(30)) {
            for (t in listOf(0f, 1f, 7.5f, 120f, 86_400f)) {
                val y = flakeY(f, t)
                assertTrue("y=$y at t=$t", y >= 0f && y < 1f)
            }
        }
    }

    @Test
    fun burstRunsZeroToOneAndClamps() {
        val start = 1_000_000_000L
        assertEquals(0f, burstProgress(start, start), 1e-6f)
        assertEquals(0.5f, burstProgress(start, start + BURST_MS * 500_000L), 1e-3f)
        assertEquals(1f, burstProgress(start, start + BURST_MS * 2_000_000L), 1e-6f)
        // a burst stamped "later" than now (first frame) does not go negative
        assertEquals(0f, burstProgress(start, start - 5), 1e-6f)
    }
}
