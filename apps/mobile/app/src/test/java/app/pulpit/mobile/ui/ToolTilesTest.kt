package app.pulpit.mobile.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure extrapolation/format helpers of the tool tiles (ToolTiles.kt). */
class ToolTilesTest {

    @Test
    fun timerRemainingExtrapolatesWhileRunning() {
        // 60 s countdown, 15 s accumulated, running for 10 s more
        assertEquals(
            35_000L,
            timerExtrapolateRemaining(
                durationMs = 60_000L,
                elapsedMs = 15_000L,
                startedAtMs = 1_000L,
                running = true,
                finished = false,
                nowMs = 11_000L,
            ),
        )
    }

    @Test
    fun timerRemainingFreezesWhilePausedAndAtFinish() {
        assertEquals(
            45_000L,
            timerExtrapolateRemaining(60_000L, 15_000L, 1_000L, running = false, finished = false, nowMs = 999_999L),
        )
        assertEquals(
            0L,
            timerExtrapolateRemaining(60_000L, 0L, 0L, running = false, finished = true, nowMs = 5L),
        )
        // clock behind the start stamp must not produce negative time
        assertEquals(
            60_000L,
            timerExtrapolateRemaining(60_000L, 0L, 5_000L, running = true, finished = false, nowMs = 1_000L),
        )
    }

    @Test
    fun stopwatchElapsedExtrapolatesOnlyWhileRunning() {
        assertEquals(
            25_000L,
            stopwatchExtrapolateElapsed(elapsedMs = 5_000L, startedAtMs = 1_000L, running = true, nowMs = 21_000L),
        )
        assertEquals(
            5_000L,
            stopwatchExtrapolateElapsed(elapsedMs = 5_000L, startedAtMs = 1_000L, running = false, nowMs = 999_999L),
        )
    }

    @Test
    fun mmssFormatsWithAndWithoutHours() {
        assertEquals("00:05", formatMmSs(5_000L))
        // ceil so a running timer never shows one second too many
        assertEquals("01:00", formatMmSs(59_001L))
        assertEquals("01:05", formatMmSs(65_000L))
        assertEquals("01:00:00", formatMmSs(3_600_000L))
    }

    @Test
    fun durationStringsParseLikeTheServer() {
        assertEquals(300_000L, parseDurationString("05:00"))
        assertEquals(90_000L, parseDurationString("1:30"))
        assertEquals(45_000L, parseDurationString("45"))
        assertEquals(3_600_000L, parseDurationString("1:00:00"))
        // unusable input falls back to the 5 min default, like the server
        assertEquals(300_000L, parseDurationString("abc"))
        assertEquals(300_000L, parseDurationString(null))
    }

    @Test
    fun timerTickDelayLandsOnTheNextDisplayChange() {
        // k*1000 waits a full second, k*1000 + 1 waits 1 ms (floored)
        assertEquals(1_000L, timerTickDelayMs(5_000L))
        assertEquals(500L, timerTickDelayMs(1_500L))
        assertEquals(TICK_MIN_DELAY_MS, timerTickDelayMs(1_001L))
        // the last step lands on exactly 0 (the finished instant)
        assertEquals(1_000L, timerTickDelayMs(1_000L))
        assertEquals(TICK_MIN_DELAY_MS, timerTickDelayMs(1L))
        // nothing left to tick for: bounded, never zero or negative
        assertEquals(TICK_MAX_DELAY_MS, timerTickDelayMs(0L))
        assertEquals(TICK_MAX_DELAY_MS, timerTickDelayMs(-5L))
    }

    @Test
    fun timerTickDelayMatchesFormatMmSsBruteForce() {
        for (remaining in 1L..3_500L) {
            val delay = timerTickDelayMs(remaining)
            assertTrue("clamped range for $remaining", delay in TICK_MIN_DELAY_MS..TICK_MAX_DELAY_MS)
            val before = formatMmSs(remaining)
            // unclamped delays are exact: same text right before, new text on the dot
            if (delay > TICK_MIN_DELAY_MS) {
                assertEquals("same at $remaining - ${delay - 1}", before, formatMmSs(remaining - (delay - 1)))
                assertNotEquals("flips at $remaining - $delay", before, formatMmSs(maxOf(0L, remaining - delay)))
            }
        }
    }

    @Test
    fun stopwatchTickDelayLandsOnTheNextDisplayChange() {
        // 1500 reads 2; it reads 3 from 2001, i.e. 501 ms later
        assertEquals(501L, stopwatchTickDelayMs(1_500L))
        assertEquals(1_000L, stopwatchTickDelayMs(1_001L))
        // exact multiples read k until 1 ms later: wait the floor, not a second
        assertEquals(TICK_MIN_DELAY_MS, stopwatchTickDelayMs(2_000L))
        assertEquals(TICK_MIN_DELAY_MS, stopwatchTickDelayMs(0L))
    }

    @Test
    fun stopwatchTickDelayMatchesFormatMmSsBruteForce() {
        for (elapsed in 1L..3_500L) {
            val delay = stopwatchTickDelayMs(elapsed)
            assertTrue("clamped range for $elapsed", delay in TICK_MIN_DELAY_MS..TICK_MAX_DELAY_MS)
            val before = formatMmSs(elapsed)
            if (delay > TICK_MIN_DELAY_MS) {
                assertEquals("same at $elapsed + ${delay - 1}", before, formatMmSs(elapsed + delay - 1))
                assertNotEquals("flips at $elapsed + $delay", before, formatMmSs(elapsed + delay))
            }
        }
    }
}
