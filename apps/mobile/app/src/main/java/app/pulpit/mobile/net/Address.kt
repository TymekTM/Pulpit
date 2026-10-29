//! Address validation for the connect path (round 4, MOB-01): a host
//! OkHttp cannot parse used to escape as an IllegalArgumentException
//! from Request.Builder().url() straight through the click handler - and
//! since the bad host was already saved, as a launch crash loop on every
//! later start. The check uses the same parser OkHttp will use, so
//! whatever passes here cannot throw later.

package app.pulpit.mobile.net

import app.pulpit.mobile.R
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

/** Why `host:port` cannot become a socket URL. The UI localizes these
 *  through [resId]; [fallbackText] is the plain English form for logs
 *  and the non-resource fallback path. */
sealed interface AddressError {
    /** Empty or whitespace-only host field. */
    data object BlankHost : AddressError

    /** Port outside 1..65535 (0 stands for "no parseable number" from
     *  the text field). */
    data object BadPort : AddressError

    /** Parsing alone is not enough: "192.168.1.2/x" parses as host
     *  192.168.1.2 with the port swallowed into the path, so the parsed
     *  authority must be exactly what was typed. */
    data class BadAddress(val host: String) : AddressError
}

/** Why `host:port` cannot become a socket URL; null when it can.
 *  The check parses the `http` form of the URL because that is exactly
 *  what OkHttp does internally: Request.Builder.url() rewrites
 *  ws/wss to http/https before HttpUrl parses the authority. */
fun addressError(host: String, port: Int): AddressError? {
    if (host.isBlank()) return AddressError.BlankHost
    if (port !in 1..65535) return AddressError.BadPort
    val url = "http://$host:$port/v2/ws".toHttpUrlOrNull()
    val typed = host.trim().removePrefix("[").removeSuffix("]")
    return if (url == null || !url.host.equals(typed, ignoreCase = true) || url.port != port) {
        AddressError.BadAddress(host)
    } else {
        null
    }
}

/** English fallback wording (logs, non-resource contexts). */
fun AddressError.fallbackText(): String = when (this) {
    is AddressError.BlankHost -> "enter the PC address"
    is AddressError.BadPort -> "port must be between 1 and 65535"
    is AddressError.BadAddress -> "\"$host\" is not a valid address"
}

/** Localized display form. */
fun AddressError.resId(): Int = when (this) {
    is AddressError.BlankHost -> R.string.main_error_blank_host
    is AddressError.BadPort -> R.string.main_error_bad_port
    is AddressError.BadAddress -> R.string.main_error_bad_address
}

/** The single optional format argument of [resId]. */
fun AddressError.resArg(): String? = (this as? AddressError.BadAddress)?.host
