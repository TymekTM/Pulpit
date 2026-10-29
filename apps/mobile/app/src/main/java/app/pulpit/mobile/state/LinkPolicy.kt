//! Deck screen policy (pure, unit-tested in LinkPolicyTest): which screen
//! the Activity shows, and when it may let the display sleep.

package app.pulpit.mobile.state

import android.content.Context
import app.pulpit.mobile.R
import app.pulpit.mobile.net.ConnState
import kotlinx.coroutines.flow.MutableStateFlow

/** How long the link may stay down, with the board on screen, before the
 *  deck stops holding the display awake. Long enough to ride out a Wi-Fi
 *  blip or a desktop restart; short enough that a sleeping PC does not
 *  keep the tablet lit all night. */
const val LINK_STANDBY_MS = 3 * 60_000L

/** Shared flags between the ViewModel (which owns the socket) and
 *  [app.pulpit.mobile.LinkService] (which owns the foreground state).
 *  The service flips [keepLinkInBackground]; the ViewModel mirrors the
 *  link line into [status] for the notification. */
object LinkBus {
    val status = MutableStateFlow<LinkStatus?>(null)

    /** True while the keep-alive service is up: background keeps the
     *  link open (see [closesLinkOnBackground]). */
    val keepLinkInBackground = MutableStateFlow(false)
}

/** Whether the board (not the connect or goodbye screen) is on screen. A
 *  snapshot keeps the board up through a transient drop; a refusal
 *  (revoked token, bad code) needs the connect screen and its "Forget
 *  pairing". [menuOpen] is the user's own back-button exit to the connect
 *  screen: it wins over any snapshot, so a deck that cannot reconnect can
 *  still reach the address fields. */
fun showsBoard(
    conn: ConnState,
    hasBoards: Boolean,
    serverDown: Boolean,
    menuOpen: Boolean = false,
): Boolean {
    if (serverDown || menuOpen) return false
    if (conn is ConnState.Failed && !conn.retryable) return false
    return conn is ConnState.Connected || hasBoards
}

/** FLAG_KEEP_SCREEN_ON is held only for a board that is live or still
 *  inside its reconnect grace; everything else follows the system
 *  screen timeout. */
fun keepsScreenOn(showsBoard: Boolean, linkStandby: Boolean): Boolean =
    showsBoard && !linkStandby

/** One line for the keep-alive service notification: what the link is
 *  doing, in deck terms. Resource-backed (with the optional host
 *  argument), so the notification follows the system locale; the
 *  service resolves it with getString at build time. */
data class LinkStatus(val resId: Int, val arg: String? = null)

fun linkStatus(conn: ConnState, host: String): LinkStatus = when (conn) {
    is ConnState.Connected -> LinkStatus(R.string.main_link_connected, host)
    is ConnState.Connecting -> LinkStatus(R.string.main_link_connecting, host)
    is ConnState.ServerDown -> LinkStatus(R.string.main_link_server_down)
    is ConnState.Failed ->
        if (conn.retryable) LinkStatus(R.string.main_link_unreachable)
        else LinkStatus(R.string.main_link_refused)
    is ConnState.Disconnected -> LinkStatus(R.string.main_link_disconnected)
}

/** Pure background decision (ROADMAP M4 vs plan 008): the socket closes
 *  on background - 008's battery win - unless the foreground keep-alive
 *  service is running. A final refusal or a pairing in flight always
 *  closes: the connect screen must keep its message, and a one-time
 *  code cannot be retried anyway. */
fun closesLinkOnBackground(keepAlive: Boolean, refused: Boolean, pairingInFlight: Boolean): Boolean =
    !(keepAlive && !refused && !pairingInFlight)
