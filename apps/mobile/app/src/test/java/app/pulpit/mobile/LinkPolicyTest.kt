//! LinkPolicy tests: the pure background/keep-alive decisions and the
//! notification line (ROADMAP M4 vs plan 008).

package app.pulpit.mobile

import app.pulpit.mobile.net.ConnState
import app.pulpit.mobile.state.closesLinkOnBackground
import app.pulpit.mobile.state.keepsScreenOn
import app.pulpit.mobile.state.linkStatus
import app.pulpit.mobile.state.showsBoard
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LinkPolicyTest {
    @Test
    fun backgroundClosesTheLinkWithoutKeepAlive() {
        assertTrue(closesLinkOnBackground(keepAlive = false, refused = false, pairingInFlight = false))
    }

    @Test
    fun keepAliveHoldsTheLinkInBackground() {
        assertFalse(closesLinkOnBackground(keepAlive = true, refused = false, pairingInFlight = false))
    }

    @Test
    fun aRefusalOrPairingInFlightAlwaysCloses() {
        // a refused pairing must keep its message on the connect screen,
        // and a one-time code in flight cannot be retried anyway - the
        // keep-alive service must not override either
        assertTrue(closesLinkOnBackground(keepAlive = true, refused = true, pairingInFlight = false))
        assertTrue(closesLinkOnBackground(keepAlive = true, refused = false, pairingInFlight = true))
    }

    @Test
    fun statusLineNamesEveryState() {
        // the notification line is resource-backed (EN default +
        // values-pl override), so the mapping is pinned per state
        assertEquals(R.string.main_link_connected, linkStatus(ConnState.Connected("192.0.2.1", 8500), "192.0.2.1").resId)
        assertEquals("192.0.2.1", linkStatus(ConnState.Connected("192.0.2.1", 8500), "192.0.2.1").arg)
        assertEquals(R.string.main_link_connecting, linkStatus(ConnState.Connecting("192.0.2.1", 8500), "192.0.2.1").resId)
        assertEquals("192.0.2.1", linkStatus(ConnState.Connecting("192.0.2.1", 8500), "192.0.2.1").arg)
        assertEquals(R.string.main_link_server_down, linkStatus(ConnState.ServerDown, "h").resId)
        assertEquals(R.string.main_link_unreachable, linkStatus(ConnState.Failed("timeout"), "h").resId)
        assertEquals(R.string.main_link_refused, linkStatus(ConnState.Failed("bad code", retryable = false), "h").resId)
        assertEquals(R.string.main_link_disconnected, linkStatus(ConnState.Disconnected, "h").resId)
    }

    @Test
    fun screenPolicyStillSane() {
        // a live board keeps the screen awake; a board in standby does not
        assertTrue(showsBoard(ConnState.Connected("h", 8500), hasBoards = false, serverDown = false))
        assertFalse(keepsScreenOn(showsBoard = true, linkStandby = true))
        assertTrue(keepsScreenOn(showsBoard = true, linkStandby = false))
    }

    @Test
    fun menuOpenWinsOverSnapshotAndLiveLink() {
        // Back -> "yes" must reach the connect screen even with boards on
        // hand or a live link
        assertFalse(showsBoard(ConnState.Connected("h", 8500), hasBoards = true, serverDown = false, menuOpen = true))
        assertFalse(showsBoard(ConnState.Disconnected, hasBoards = true, serverDown = false, menuOpen = true))
        assertTrue(showsBoard(ConnState.Disconnected, hasBoards = true, serverDown = false, menuOpen = false))
    }
}
