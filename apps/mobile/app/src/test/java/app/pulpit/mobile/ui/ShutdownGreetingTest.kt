package app.pulpit.mobile.ui

import app.pulpit.mobile.R
import org.junit.Assert.assertEquals
import org.junit.Test

/** The shutdown standby screen greets by the tablet's local hour: night
 *  says goodnight, everything else goodbye. The greeting is a string
 *  resource (EN default, values-pl override), so the test pins the
 *  resource mapping rather than the English words. */
class ShutdownGreetingTest {

    @Test
    fun night_hours_say_goodnight() {
        for (hour in listOf(21, 22, 23, 0, 3, 5)) {
            assertEquals(R.string.main_goodnight, shutdownGreeting(hour))
        }
    }

    @Test
    fun daytime_hours_say_goodbye() {
        for (hour in listOf(6, 9, 12, 17, 20)) {
            assertEquals(R.string.main_goodbye, shutdownGreeting(hour))
        }
    }
}
