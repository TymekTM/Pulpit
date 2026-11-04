//! First-run / disconnected screen: server address entry plus pairing.
//! A paired device (token stored) just hits Connect. A new device enters
//! the one-time code the desktop prints next to its QR (POST /v2/pair)
//! together with a name shown in the desktop's trust prompt.

package app.pulpit.mobile.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Link
import androidx.compose.material.icons.rounded.WifiFind
import androidx.compose.material3.Icon
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.draw.clip
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import app.pulpit.mobile.R
import app.pulpit.mobile.net.AddressError
import app.pulpit.mobile.net.ConnState
import app.pulpit.mobile.net.DiscoveredDesktop
import app.pulpit.mobile.net.NsdDiscovery
import app.pulpit.mobile.state.PulpitViewModel

@Composable
fun ConnectScreen(vm: PulpitViewModel) {
    val cfg by vm.config.collectAsState()
    val conn by vm.connState.collectAsState()
    val context = LocalContext.current

    var host by remember(cfg.host) { mutableStateOf(cfg.host) }
    var port by remember(cfg.port) { mutableStateOf(cfg.port.toString()) }
    var name by remember(cfg.name) { mutableStateOf(cfg.name) }
    var pairCode by remember { mutableStateOf("") }
    // MOB-01: set when the typed host/port cannot become a URL - shown
    // instead of connecting (and before saveConfig persists the bad pair)
    var addressError by remember { mutableStateOf<AddressError?>(null) }
    val paired = !cfg.token.isNullOrBlank()

    // M8 pairing modes: Auto (pick a desktop discovered over mDNS, both
    // screens compare the verification code) and Manual (type the code
    // the desktop minted) - the original flow stays for when discovery
    // cannot see the desktop (firewall, other subnet).
    var autoMode by remember { mutableStateOf(true) }
    val discovered = remember { mutableStateListOf<DiscoveredDesktop>() }
    val pairReq by vm.pairRequest.collectAsState()
    DisposableEffect(autoMode, paired) {
        val nsd = if (autoMode && !paired) {
            NsdDiscovery(context) { found ->
                if (discovered.none { it.host == found.host && it.port == found.port }) {
                    discovered.add(found)
                }
            }.also { it.start() }
        } else {
            null
        }
        onDispose { nsd?.stop() }
    }

    val hapticConfig by vm.hapticConfig.collectAsState()
    val scrollState = rememberScrollState()

    // the pairing / connection card; on a tablet it sits beside the
    // brand column, on a phone below it
    val form: @Composable () -> Unit = {
        Column(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(20.dp))
                .background(PulpitColors.tile)
                .border(1.dp, Color.White.copy(alpha = 0.05f), RoundedCornerShape(20.dp))
                .padding(24.dp),
        ) {
            SectionLabel(stringResource(R.string.connect_section_desktop))
            OutlinedTextField(
                value = host,
                onValueChange = { host = it },
                label = { Text(stringResource(R.string.main_field_host)) },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier.fillMaxWidth(),
            )
            Row(Modifier.fillMaxWidth().padding(top = 10.dp)) {
                OutlinedTextField(
                    value = port,
                    onValueChange = { port = it.filter(Char::isDigit).take(5) },
                    label = { Text(stringResource(R.string.main_field_port)) },
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = Modifier.weight(1f),
                )
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.main_field_device_name)) },
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    modifier = Modifier.weight(1.4f).padding(start = 10.dp),
                )
            }

            if (paired) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    modifier = Modifier
                        .padding(top = 16.dp)
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(12.dp))
                        .background(PulpitColors.accentSoft)
                        .padding(horizontal = 14.dp, vertical = 12.dp),
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Link,
                        contentDescription = null,
                        tint = PulpitColors.accent,
                        modifier = Modifier.size(18.dp),
                    )
                    Text(
                        text = stringResource(R.string.main_paired_hint),
                        color = PulpitColors.ink,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
                // The keep-alive service needs to escape Doze on aggressive
                // ROMs; stock Android keeps foreground services running, but
                // the OEM grid (Lineage included) may not. One tap, once.
                val pm = context.getSystemService(PowerManager::class.java)
                val pkg = context.packageName
                Row(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (pm != null && !pm.isIgnoringBatteryOptimizations(pkg)) {
                        TextButton(
                            text = stringResource(R.string.main_battery_whitelist),
                            onClick = {
                                runCatching {
                                    context.startActivity(
                                        Intent(
                                            Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                                            Uri.parse("package:$pkg"),
                                        ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                                    )
                                }
                            },
                        )
                    }
                    TextButton(
                        text = stringResource(R.string.main_forget_pairing),
                        onClick = { vm.forgetPairing() },
                    )
                }
            } else {
                // M8 pairing modes: Auto discovers desktops on the network and
                // finishes with the shared verification code; Manual keeps the
                // desktop-minted code entry.
                SectionLabel(
                    stringResource(R.string.connect_section_pairing),
                    Modifier.padding(top = 22.dp),
                )
                Segmented(
                    options = listOf(
                        stringResource(R.string.pair_mode_auto),
                        stringResource(R.string.pair_mode_manual),
                    ),
                    selected = if (autoMode) 0 else 1,
                    onSelect = { autoMode = it == 0 },
                )

                val waiting = pairReq
                if (autoMode) {
                    if (waiting != null) {
                        // both screens show this number; pairing completes only
                        // when the desktop operator confirms the match
                        Column(
                            horizontalAlignment = Alignment.CenterHorizontally,
                            modifier = Modifier
                                .padding(top = 14.dp)
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(14.dp))
                                .background(PulpitColors.well)
                                .padding(vertical = 18.dp, horizontal = 16.dp),
                        ) {
                            Text(
                                text = waiting.code,
                                style = MaterialTheme.typography.displayMedium.copy(
                                    fontWeight = FontWeight.SemiBold,
                                    letterSpacing = 8.sp,
                                ),
                                color = PulpitColors.ink,
                            )
                            Text(
                                text = stringResource(R.string.pair_confirm_match),
                                color = PulpitColors.ink2,
                                style = MaterialTheme.typography.bodyMedium,
                                textAlign = TextAlign.Center,
                                modifier = Modifier.padding(top = 6.dp),
                            )
                            OutlinedButton(
                                onClick = { vm.cancelPairRequest() },
                                shape = RoundedCornerShape(12.dp),
                                modifier = Modifier.padding(top = 12.dp),
                            ) { Text(stringResource(R.string.pair_cancel)) }
                        }
                    } else if (discovered.isEmpty()) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            modifier = Modifier
                                .padding(top = 14.dp)
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(14.dp))
                                .background(PulpitColors.well)
                                .padding(16.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.WifiFind,
                                contentDescription = null,
                                tint = PulpitColors.ink3,
                                modifier = Modifier.size(22.dp),
                            )
                            Text(
                                text = stringResource(R.string.pair_searching),
                                color = PulpitColors.ink2,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    } else {
                        Column(
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.padding(top = 14.dp),
                        ) {
                            discovered.forEach { desktop ->
                                DesktopRow(
                                    desktop = desktop,
                                    onClick = {
                                        vm.startPairRequest(
                                            desktop.host,
                                            desktop.port,
                                            name.ifBlank { Build.MODEL },
                                        )
                                    },
                                )
                            }
                        }
                    }
                } else {
                    OutlinedTextField(
                        value = pairCode,
                        onValueChange = { pairCode = it.uppercase().filter { c -> c.isLetterOrDigit() }.take(8) },
                        label = { Text(stringResource(R.string.pair_code_field)) },
                        singleLine = true,
                        shape = RoundedCornerShape(12.dp),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Text),
                        modifier = Modifier.fillMaxWidth().padding(top = 14.dp),
                    )
                    // USB path (ROADMAP M4): with the desktop reachable over adb
                    // reverse, the server answers on the phone's own loopback.
                    TextButton(
                        text = stringResource(R.string.pair_usb_button),
                        onClick = { host = "127.0.0.1" },
                    )
                    Text(
                        text = stringResource(R.string.pair_usb_command),
                        style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                        color = PulpitColors.ink3,
                        modifier = Modifier.padding(start = 12.dp),
                    )
                }
            }

            when (conn) {
                is ConnState.Connecting -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                        modifier = Modifier.padding(top = 18.dp),
                    ) {
                        // the same fixed-arc ring as the board overlay: the
                        // deck's animator scale is off, a Material
                        // indeterminate spinner would freeze into a dot
                        RingSpinner()
                        Text(
                            text = stringResource(R.string.main_connecting_to, cfg.host, cfg.port),
                            color = PulpitColors.ink2,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }
                is ConnState.Failed -> {
                    ErrorLine(failedText(conn as ConnState.Failed))
                }
                else -> {}
            }

            addressError?.let { err ->
                ErrorLine(
                    // resolved in composition: onClick handlers cannot call
                    // stringResource themselves
                    when (err) {
                        is AddressError.BlankHost -> stringResource(R.string.main_error_blank_host)
                        is AddressError.BadPort -> stringResource(R.string.main_error_bad_port)
                        is AddressError.BadAddress ->
                            stringResource(R.string.main_error_bad_address, err.host)
                    },
                )
            }

            if (!paired && autoMode) {
                // the auto flow is self-contained: picking a desktop starts
                // the request and approval connects on its own
            } else {
                Button(
                    onClick = {
                        // MOB-01: validate before saving - a persisted
                        // malformed host crashed (and re-crashed) the app
                        // on every paired launch
                        val portNum = port.toIntOrNull() ?: 0
                        val err = app.pulpit.mobile.net.addressError(host.trim(), portNum)
                        if (err != null) {
                            addressError = err
                            return@Button
                        }
                        addressError = null
                        vm.saveConfig(
                            cfg.copy(host = host.trim(), port = portNum, name = name.trim()),
                        )
                        if (paired) {
                            vm.connect()
                        } else {
                            vm.connectWithPairCode(pairCode)
                        }
                    },
                    shape = RoundedCornerShape(14.dp),
                    contentPadding = PaddingValues(vertical = 14.dp),
                    modifier = Modifier.padding(top = 20.dp).fillMaxWidth(),
                ) {
                    Text(
                        if (paired) stringResource(R.string.main_connect) else stringResource(R.string.main_pair),
                        style = MaterialTheme.typography.titleSmall,
                    )
                }
            }
        }
    }

    val haptics: @Composable () -> Unit = {
        HapticsSettings(
            config = hapticConfig,
            onEnabledChange = vm::setHapticsEnabled,
            onIntensityChange = vm::setHapticIntensity,
            modifier = Modifier.padding(top = 14.dp),
        )
    }

    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            .background(PulpitColors.bg),
    ) {
        val wide = maxWidth >= 720.dp
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(scrollState)
                .padding(horizontal = if (wide) 48.dp else 20.dp, vertical = 32.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            if (wide) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(56.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.widthIn(max = 1040.dp).fillMaxWidth(),
                ) {
                    BrandColumn(paired, Modifier.weight(0.85f))
                    Column(Modifier.weight(1f)) {
                        form()
                        haptics()
                    }
                }
            } else {
                Column(Modifier.widthIn(max = 560.dp).fillMaxWidth()) {
                    BrandColumn(paired, Modifier.padding(bottom = 24.dp))
                    form()
                    haptics()
                }
            }
        }
    }
}

/** Logo, name and the three steps of first contact. */
@Composable
private fun BrandColumn(paired: Boolean, modifier: Modifier = Modifier) {
    Column(modifier) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            BrandMark(Modifier.size(40.dp))
            Text(
                text = stringResource(R.string.app_name),
                style = MaterialTheme.typography.headlineMedium,
                color = PulpitColors.ink,
                modifier = Modifier.padding(start = 14.dp),
            )
        }
        Text(
            text = stringResource(R.string.main_connect_subtitle),
            style = MaterialTheme.typography.bodyLarge,
            color = PulpitColors.ink2,
            modifier = Modifier.padding(top = 10.dp),
        )
        if (!paired) {
            Column(
                verticalArrangement = Arrangement.spacedBy(14.dp),
                modifier = Modifier.padding(top = 28.dp),
            ) {
                Step(1, stringResource(R.string.connect_step_open))
                Step(2, stringResource(R.string.connect_step_pick))
                Step(3, stringResource(R.string.connect_step_confirm))
            }
        }
    }
}

/** The desktop's logo: a 2x2 tile grid in accent tints. */
@Composable
fun BrandMark(modifier: Modifier = Modifier) {
    val alphas = listOf(1f, 0.55f, 0.3f, 0.8f)
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        for (row in 0..1) {
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                for (col in 0..1) {
                    Box(
                        Modifier
                            .weight(1f)
                            .fillMaxSize()
                            .clip(RoundedCornerShape(5.dp))
                            .background(PulpitColors.accent.copy(alpha = alphas[row * 2 + col])),
                    )
                }
            }
        }
    }
}

@Composable
private fun Step(number: Int, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier
                .size(28.dp)
                .clip(CircleShape)
                .background(PulpitColors.tile2),
        ) {
            Text(
                text = number.toString(),
                style = MaterialTheme.typography.labelLarge,
                color = PulpitColors.accent,
            )
        }
        Text(
            text = text,
            style = MaterialTheme.typography.bodyMedium,
            color = PulpitColors.ink2,
            modifier = Modifier.padding(start = 14.dp),
        )
    }
}

@Composable
private fun SectionLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text = text.uppercase(),
        style = MaterialTheme.typography.labelSmall.copy(letterSpacing = 1.2.sp),
        color = PulpitColors.ink3,
        modifier = modifier.padding(bottom = 10.dp),
    )
}

/** Two-way segmented switch (pairing mode). */
@Composable
private fun Segmented(options: List<String>, selected: Int, onSelect: (Int) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(PulpitColors.well)
            .padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        options.forEachIndexed { i, label ->
            val on = i == selected
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(9.dp))
                    .background(if (on) PulpitColors.tile2 else Color.Transparent)
                    .clickable { onSelect(i) }
                    .padding(vertical = 10.dp),
            ) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.labelLarge,
                    color = if (on) PulpitColors.ink else PulpitColors.ink3,
                )
            }
        }
    }
}

/** One desktop found over mDNS: tap to start the pairing request. */
@Composable
private fun DesktopRow(desktop: DiscoveredDesktop, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(PulpitColors.tile2)
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 12.dp),
    ) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier
                .size(38.dp)
                .clip(RoundedCornerShape(10.dp))
                .background(PulpitColors.accentSoft),
        ) {
            Icon(
                imageVector = Icons.Rounded.Computer,
                contentDescription = null,
                tint = PulpitColors.accent,
                modifier = Modifier.size(20.dp),
            )
        }
        // payload-driven: the desktop's advertised name and socket, no
        // translatable words in the row
        Column(Modifier.weight(1f).padding(start = 14.dp)) {
            Text(
                text = desktop.name,
                style = MaterialTheme.typography.titleSmall,
                color = PulpitColors.ink,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = "${desktop.host}:${desktop.port}",
                style = MaterialTheme.typography.bodySmall,
                color = PulpitColors.ink3,
            )
        }
        Icon(
            imageVector = Icons.Rounded.ChevronRight,
            contentDescription = null,
            tint = PulpitColors.ink3,
        )
    }
}

@Composable
private fun ErrorLine(text: String) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        modifier = Modifier
            .padding(top = 14.dp)
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(Color(0x24E74C3C))
            .padding(horizontal = 14.dp, vertical = 10.dp),
    ) {
        Icon(
            imageVector = Icons.Rounded.ErrorOutline,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.error,
            modifier = Modifier.size(18.dp),
        )
        Text(
            text = text,
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

@Composable
private fun TextButton(text: String, onClick: () -> Unit) {
    androidx.compose.material3.TextButton(onClick = onClick) {
        Text(text, color = PulpitColors.ink2, style = MaterialTheme.typography.labelLarge)
    }
}

/** A Failed state's user-facing text: the localized resource form when
 *  one rides along (client-side chrome - V2Client, the ViewModel, the
 *  address oracle), else the raw reason as it arrived (server error
 *  messages and OkHttp text are wire content, shown as-is). */
@Composable
private fun failedText(failed: ConnState.Failed): String =
    failed.reasonRes?.let { res ->
        failed.reasonResArg?.let { arg -> stringResource(res, arg) } ?: stringResource(res)
    } ?: failed.reason

@Composable
fun HapticsSettings(
    config: HapticConfig,
    onEnabledChange: (Boolean) -> Unit,
    onIntensityChange: (HapticIntensity) -> Unit,
    modifier: Modifier = Modifier,
) {
    val haptics = LocalHaptics.current
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(20.dp))
            .background(PulpitColors.tile)
            .padding(horizontal = 24.dp, vertical = 18.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Column {
                Text(
                    text = stringResource(R.string.main_haptics_title),
                    style = MaterialTheme.typography.titleSmall,
                    color = PulpitColors.ink,
                )
                Text(
                    text = if (config.enabled) {
                        stringResource(
                            R.string.main_haptics_on,
                            stringResource(config.intensity.labelRes),
                        )
                    } else {
                        stringResource(R.string.main_haptics_off)
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = PulpitColors.ink3,
                )
            }
            Switch(
                checked = config.enabled,
                onCheckedChange = {
                    onEnabledChange(it)
                    if (it) haptics.click()
                },
            )
        }

        if (config.enabled) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                HapticIntensity.values().forEach { intensity ->
                    val selected = config.intensity == intensity
                    if (selected) {
                        Button(
                            onClick = {
                                onIntensityChange(intensity)
                                haptics.click()
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(
                                stringResource(intensity.labelRes),
                                style = MaterialTheme.typography.labelMedium,
                            )
                        }
                    } else {
                        OutlinedButton(
                            onClick = {
                                onIntensityChange(intensity)
                                haptics.click()
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(
                                stringResource(intensity.labelRes),
                                style = MaterialTheme.typography.labelMedium,
                            )
                        }
                    }
                }
            }
        }
    }
}