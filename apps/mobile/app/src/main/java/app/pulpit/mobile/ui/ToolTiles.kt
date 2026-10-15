package app.pulpit.mobile.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.pulpit.mobile.proto.Tile
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Locale
import java.util.TimeZone
import kotlinx.coroutines.delay
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

/** Pure extrapolation for countdown timer remaining milliseconds. */
fun timerExtrapolateRemaining(
    durationMs: Long,
    elapsedMs: Long,
    startedAtMs: Long,
    running: Boolean,
    finished: Boolean,
    nowMs: Long,
): Long {
    if (finished) return 0L
    val totalElapsed = if (running) {
        elapsedMs + kotlin.math.max(0L, nowMs - startedAtMs)
    } else {
        elapsedMs
    }
    return kotlin.math.max(0L, durationMs - totalElapsed)
}

/**
 * The tile's compact state object, keeping the last one seen while a
 * legacy stock client is connected: the 1 Hz lane then pushes a plain
 * formatted label string under the same channel (the stock client can
 * only show text), and falling back to tile params on every one of those
 * pushes would make the timer flicker between views.
 */
@Composable
fun rememberToolState(live: JsonElement?): JsonObject? {
    var last by remember { mutableStateOf<JsonObject?>(null) }
    (live as? JsonObject)?.let { last = it }
    return last
}

/** Pure extrapolation for stopwatch elapsed milliseconds. */
fun stopwatchExtrapolateElapsed(
    elapsedMs: Long,
    startedAtMs: Long,
    running: Boolean,
    nowMs: Long,
): Long {
    return if (running) {
        elapsedMs + kotlin.math.max(0L, nowMs - startedAtMs)
    } else {
        elapsedMs
    }
}

/** Formats milliseconds as mm:ss or hh:mm:ss. */
fun formatMmSs(ms: Long): String {
    val totalSecs = (ms + 999) / 1000
    val hours = totalSecs / 3600
    val mins = (totalSecs % 3600) / 60
    val secs = totalSecs % 60
    return if (hours > 0) {
        String.format(Locale.ROOT, "%02d:%02d:%02d", hours, mins, secs)
    } else {
        String.format(Locale.ROOT, "%02d:%02d", mins, secs)
    }
}

/** Floor and ceiling of the tick delays below. The floor stops a wake-up
 *  that lands a hair before the boundary (wall clock vs. uptime drift) from
 *  degenerating into a 1 ms busy loop; the ceiling keeps a stale
 *  extrapolation from sleeping more than one displayed second. */
internal const val TICK_MIN_DELAY_MS = 16L
internal const val TICK_MAX_DELAY_MS = 1_000L

/**
 * How long until the countdown's text can next change. [formatMmSs] shows
 * ceil(ms / 1000), so with [remainingMs] in ((k-1)*1000, k*1000] the text
 * reads k and flips to k-1 once remaining drops to (k-1)*1000, i.e. after
 * ((remaining - 1) % 1000) + 1 ms (remaining == k*1000 waits a full second;
 * remaining == (k-1)*1000 + 1 waits 1 ms). The last step lands on exactly
 * 0, which is also the instant the tile's finished state appears. Callers
 * stop ticking at 0 (nothing changes after it).
 */
internal fun timerTickDelayMs(remainingMs: Long): Long {
    if (remainingMs <= 0L) return TICK_MAX_DELAY_MS
    return (((remainingMs - 1L) % 1000L) + 1L).coerceIn(TICK_MIN_DELAY_MS, TICK_MAX_DELAY_MS)
}

/**
 * How long until the stopwatch's text can next change. The same ceil
 * display on an increasing value flips from k to k+1 one millisecond
 * after elapsed passes k*1000 (elapsed == 2000 still reads 2, 2001 reads
 * 3), so the wait is 1000 - ((elapsed - 1) % 1000); elapsed == 0 and
 * exact multiples of 1000 wait 1 ms (clamped up to the floor), never a
 * full second, otherwise the flip would be skipped for a whole second.
 */
internal fun stopwatchTickDelayMs(elapsedMs: Long): Long {
    if (elapsedMs <= 0L) return TICK_MIN_DELAY_MS
    return (1000L - ((elapsedMs - 1L) % 1000L)).coerceIn(TICK_MIN_DELAY_MS, TICK_MAX_DELAY_MS)
}

/** Parses duration string (e.g. "05:00", "5:00", "300") into milliseconds. */
fun parseDurationString(str: String?): Long {
    val s = str?.trim().orEmpty()
    if (s.isEmpty()) return 300_000L
    s.toLongOrNull()?.let { return it * 1000L }
    val parts = s.split(":")
    return when (parts.size) {
        2 -> {
            val m = parts[0].trim().toLongOrNull() ?: 0L
            val sec = parts[1].trim().toLongOrNull() ?: 0L
            (m * 60 + sec) * 1000L
        }
        3 -> {
            val h = parts[0].trim().toLongOrNull() ?: 0L
            val m = parts[1].trim().toLongOrNull() ?: 0L
            val sec = parts[2].trim().toLongOrNull() ?: 0L
            (h * 3600 + m * 60 + sec) * 1000L
        }
        else -> 300_000L
    }
}

/** Native clock tile (tool-clock). Rendered client-side from the device clock. */
@Composable
fun ToolClockTile(
    tile: Tile,
    titleColor: Color,
    modifier: Modifier = Modifier,
) {
    val twelveHour = tile.param("format") == "12h"
    val showSeconds = tile.param("seconds") in listOf("yes", "true", "1")
    val showDate = tile.param("date") in listOf("yes", "true", "1")
    val tzId = tile.param("timezone")

    var now by remember { mutableStateOf(Calendar.getInstance()) }

    LaunchedEffect(showSeconds, tzId) {
        while (true) {
            val cal = if (!tzId.isNullOrBlank()) {
                Calendar.getInstance(TimeZone.getTimeZone(tzId))
            } else {
                Calendar.getInstance()
            }
            now = cal
            if (showSeconds) {
                delay(1000L - (cal.get(Calendar.MILLISECOND)))
            } else {
                val sec = cal.get(Calendar.SECOND)
                val ms = cal.get(Calendar.MILLISECOND)
                delay(60_000L - (sec * 1000L + ms))
            }
        }
    }

    val timePattern = when {
        twelveHour && showSeconds -> "hh:mm:ss a"
        twelveHour -> "hh:mm a"
        showSeconds -> "HH:mm:ss"
        else -> "HH:mm"
    }

    val timeFormat = remember(timePattern, tzId) {
        SimpleDateFormat(timePattern, Locale.getDefault()).apply {
            if (!tzId.isNullOrBlank()) timeZone = TimeZone.getTimeZone(tzId)
        }
    }

    val dateFormat = remember(tzId) {
        SimpleDateFormat("EEE, d MMM", Locale.getDefault()).apply {
            if (!tzId.isNullOrBlank()) timeZone = TimeZone.getTimeZone(tzId)
        }
    }

    Box(
        modifier = modifier.fillMaxSize().padding(8.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = timeFormat.format(now.time),
                color = titleColor,
                fontSize = if (showSeconds) 20.sp else 26.sp,
                fontWeight = FontWeight.Bold,
                textAlign = TextAlign.Center,
            )
            if (showDate) {
                Spacer(Modifier.height(4.dp))
                Text(
                    text = dateFormat.format(now.time),
                    color = titleColor.copy(alpha = 0.8f),
                    fontSize = 12.sp,
                    textAlign = TextAlign.Center,
                )
            }
        }
    }
}

/** Native countdown timer tile (tool-timer). */
@Composable
fun ToolTimerTile(
    tile: Tile,
    live: JsonElement?,
    titleColor: Color,
    onPress: () -> Unit,
    onGesture: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val obj = rememberToolState(live)
    val running = obj?.get("running")?.jsonPrimitive?.booleanOrNull ?: false
    val finished = obj?.get("finished")?.jsonPrimitive?.booleanOrNull ?: false
    val startedAtMs = obj?.get("startedAtMs")?.jsonPrimitive?.longOrNull ?: 0L
    val elapsedMs = obj?.get("elapsedMs")?.jsonPrimitive?.longOrNull ?: 0L
    val durationMs = obj?.get("durationMs")?.jsonPrimitive?.longOrNull
        ?: parseDurationString(tile.param("duration"))

    var nowTick by remember { mutableLongStateOf(System.currentTimeMillis()) }

    // wakes only when the displayed second can change (see
    // timerTickDelayMs) instead of polling at 5 Hz. Keyed on every input
    // of the extrapolation, so a server push (pause/resume, new
    // startedAtMs/elapsedMs/durationMs) restarts the loop and re-aligns
    // rather than sleeping on a delay computed from the old state.
    LaunchedEffect(running, finished, startedAtMs, elapsedMs, durationMs) {
        nowTick = System.currentTimeMillis()
        if (running) {
            while (true) {
                val left = timerExtrapolateRemaining(
                    durationMs = durationMs,
                    elapsedMs = elapsedMs,
                    startedAtMs = startedAtMs,
                    running = true,
                    finished = finished,
                    nowMs = nowTick,
                )
                // at 0 the text and the finished state are final
                if (left <= 0L) break
                delay(timerTickDelayMs(left))
                nowTick = System.currentTimeMillis()
            }
        }
    }

    val remaining = timerExtrapolateRemaining(
        durationMs = durationMs,
        elapsedMs = elapsedMs,
        startedAtMs = startedAtMs,
        running = running,
        finished = finished,
        nowMs = nowTick,
    )

    val isFinished = finished || (remaining == 0L && durationMs > 0 && (running || elapsedMs >= durationMs))

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
        contentAlignment = Alignment.Center,
    ) {
        // the flashing effect only exists while finished: the infinite
        // transition lives inside TimerFlash, so an idle/running timer has
        // no per-frame work at all
        if (isFinished) {
            TimerFlash()
        }

        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(6.dp),
        ) {
            Text(
                text = formatMmSs(remaining),
                color = titleColor,
                fontSize = 24.sp,
                fontWeight = FontWeight.Bold,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(4.dp))
            val statusLabel = when {
                isFinished -> "KONIEC!"
                running -> "Działa"
                elapsedMs > 0 -> "Pauza"
                else -> "Gotowy"
            }
            Text(
                text = statusLabel,
                color = titleColor.copy(alpha = 0.75f),
                fontSize = 11.sp,
                textAlign = TextAlign.Center,
            )
        }
    }
}

/** Red pulse behind a finished timer's text. The infinite transition is
 *  created only while this is composed (the timer is finished), and the
 *  alpha is read in the draw phase so the pulse redraws a rect each frame
 *  instead of recomposing the tile. */
@Composable
private fun TimerFlash() {
    val infiniteTransition = rememberInfiniteTransition(label = "timerFlash")
    val flashAlpha = infiniteTransition.animateFloat(
        initialValue = 0f,
        targetValue = 0.85f,
        animationSpec = infiniteRepeatable(
            animation = tween(400),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "flashAlpha",
    )
    Box(
        Modifier
            .fillMaxSize()
            .drawBehind { drawRect(Color(0xFFE74C3C).copy(alpha = flashAlpha.value)) },
    )
}

/** Native stopwatch tile (tool-stopwatch). */
@Composable
fun ToolStopwatchTile(
    tile: Tile,
    live: JsonElement?,
    titleColor: Color,
    onPress: () -> Unit,
    onGesture: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val obj = rememberToolState(live)
    val running = obj?.get("running")?.jsonPrimitive?.booleanOrNull ?: false
    val startedAtMs = obj?.get("startedAtMs")?.jsonPrimitive?.longOrNull ?: 0L
    val elapsedMs = obj?.get("elapsedMs")?.jsonPrimitive?.longOrNull ?: 0L

    var nowTick by remember { mutableLongStateOf(System.currentTimeMillis()) }

    // wakes only when the displayed second can change (see
    // stopwatchTickDelayMs); keyed on the extrapolation inputs so a pushed
    // pause/resume/reset re-aligns instead of keeping a stale delay
    LaunchedEffect(running, startedAtMs, elapsedMs) {
        nowTick = System.currentTimeMillis()
        if (running) {
            while (true) {
                val shown = stopwatchExtrapolateElapsed(
                    elapsedMs = elapsedMs,
                    startedAtMs = startedAtMs,
                    running = true,
                    nowMs = nowTick,
                )
                delay(stopwatchTickDelayMs(shown))
                nowTick = System.currentTimeMillis()
            }
        }
    }

    val elapsed = stopwatchExtrapolateElapsed(
        elapsedMs = elapsedMs,
        startedAtMs = startedAtMs,
        running = running,
        nowMs = nowTick,
    )

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
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(6.dp),
        ) {
            Text(
                text = formatMmSs(elapsed),
                color = titleColor,
                fontSize = 24.sp,
                fontWeight = FontWeight.Bold,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(4.dp))
            val statusLabel = if (running) "Działa" else if (elapsed > 0) "Pauza" else "00:00"
            Text(
                text = statusLabel,
                color = titleColor.copy(alpha = 0.75f),
                fontSize = 11.sp,
                textAlign = TextAlign.Center,
            )
        }
    }
}

/** Native counter tile (tool-counter). */
@Composable
fun ToolCounterTile(
    tile: Tile,
    live: JsonElement?,
    titleColor: Color,
    onPress: () -> Unit,
    onGesture: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val obj = rememberToolState(live)
    val startVal = tile.param("start_value")?.toLongOrNull() ?: 0L
    val count = obj?.get("count")?.jsonPrimitive?.longOrNull ?: startVal
    val label = tile.param("label")?.ifBlank { null } ?: "Licznik"

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
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(6.dp),
        ) {
            Text(
                text = label,
                color = titleColor.copy(alpha = 0.8f),
                fontSize = 12.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(2.dp))
            Text(
                text = count.toString(),
                color = titleColor,
                fontSize = 32.sp,
                fontWeight = FontWeight.Bold,
                textAlign = TextAlign.Center,
            )
        }
    }
}
