package screenshare.clientkmp.ui.components

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.foundation.ScrollbarStyle
import androidx.compose.foundation.VerticalScrollbar
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollbarAdapter
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import screenshare.clientkmp.services.DisplaySurface
import screenshare.clientkmp.services.FrameRate
import screenshare.clientkmp.services.Resolution
import screenshare.clientkmp.services.ScreenShareConfig
import screenshare.clientkmp.services.ScreenSource

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ScreenShareConfigDialog(
    screenSources: List<ScreenSource>,
    onConfirm: (ScreenShareConfig) -> Unit,
    onDismiss: () -> Unit,
) {
    var selectedSourceId by remember { mutableStateOf<String?>(null) }
    var selectedDisplaySurface by remember { mutableStateOf<DisplaySurface?>(null) }
    var selectedResolution by remember { mutableStateOf(Resolution.P1080) }
    var selectedFrameRate by remember { mutableStateOf(FrameRate.FPS30) }
    var captureAudio by remember { mutableStateOf(false) }
    var displaySurfaceExpanded by remember { mutableStateOf(false) }
    var selectedTab by remember { mutableStateOf(0) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Compartilhar tela") },
        text = {
            Column(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // ─── Source section ───────────────────────────────────────────
                if (screenSources.isNotEmpty()) {
                    val monitors = screenSources.filter { it.isMonitor }
                    val windows = screenSources.filter { !it.isMonitor }
                    TabRow(selectedTabIndex = selectedTab) {
                        Tab(
                            selected = selectedTab == 0,
                            onClick = { selectedTab = 0; selectedSourceId = null },
                            text = { Text("Telas (${monitors.size})") },
                        )
                        Tab(
                            selected = selectedTab == 1,
                            onClick = { selectedTab = 1; selectedSourceId = null },
                            text = { Text("Janelas (${windows.size})") },
                        )
                    }
                    val tabSources = if (selectedTab == 0) monitors else windows
                    val listState = rememberLazyListState()
                    Box(modifier = Modifier.heightIn(max = 200.dp).fillMaxWidth()) {
                        LazyColumn(
                            state = listState,
                            modifier = Modifier.fillMaxWidth().padding(end = 10.dp),
                            verticalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            items(tabSources) { source ->
                                Row(
                                    modifier =
                                        Modifier
                                            .fillMaxWidth()
                                            .clickable { selectedSourceId = source.id }
                                            .padding(vertical = 2.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                                ) {
                                    RadioButton(
                                        selected = selectedSourceId == source.id,
                                        onClick = { selectedSourceId = source.id },
                                    )
                                    Text(
                                        text = source.title,
                                        style = MaterialTheme.typography.bodyMedium,
                                    )
                                }
                            }
                        }
                        VerticalScrollbar(
                            modifier = Modifier.align(Alignment.CenterEnd).fillMaxHeight(),
                            adapter = rememberScrollbarAdapter(listState),
                            style = ScrollbarStyle(
                                minimalHeight = 32.dp,
                                thickness = 6.dp,
                                shape = RoundedCornerShape(3.dp),
                                hoverDurationMillis = 300,
                                unhoverColor = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.20f),
                                hoverColor = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.50f),
                            ),
                        )
                    }
                    HorizontalDivider()
                } else {
                    // Browser: show displaySurface hint dropdown
                    Text("Tipo de fonte (dica)", style = MaterialTheme.typography.labelLarge)
                    ExposedDropdownMenuBox(
                        expanded = displaySurfaceExpanded,
                        onExpandedChange = { displaySurfaceExpanded = it },
                    ) {
                        OutlinedTextField(
                            value = when (selectedDisplaySurface) {
                                DisplaySurface.MONITOR -> "Monitor"
                                DisplaySurface.WINDOW -> "Janela"
                                DisplaySurface.BROWSER_TAB -> "Aba do navegador"
                                null -> "Qualquer"
                            },
                            onValueChange = {},
                            readOnly = true,
                            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = displaySurfaceExpanded) },
                            modifier = Modifier.fillMaxWidth().menuAnchor(),
                        )
                        ExposedDropdownMenu(
                            expanded = displaySurfaceExpanded,
                            onDismissRequest = { displaySurfaceExpanded = false },
                        ) {
                            DropdownMenuItem(
                                text = { Text("Qualquer") },
                                onClick = { selectedDisplaySurface = null; displaySurfaceExpanded = false },
                            )
                            DropdownMenuItem(
                                text = { Text("Monitor") },
                                onClick = { selectedDisplaySurface = DisplaySurface.MONITOR; displaySurfaceExpanded = false },
                            )
                            DropdownMenuItem(
                                text = { Text("Janela") },
                                onClick = { selectedDisplaySurface = DisplaySurface.WINDOW; displaySurfaceExpanded = false },
                            )
                            DropdownMenuItem(
                                text = { Text("Aba do navegador") },
                                onClick = { selectedDisplaySurface = DisplaySurface.BROWSER_TAB; displaySurfaceExpanded = false },
                            )
                        }
                    }
                    HorizontalDivider()
                }

                // ─── Resolution ───────────────────────────────────────────────
                Text("Resolução", style = MaterialTheme.typography.labelLarge)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Resolution.entries.forEach { res ->
                        FilterChip(
                            selected = selectedResolution == res,
                            onClick = { selectedResolution = res },
                            label = { Text(res.label) },
                        )
                    }
                }

                // ─── Frame rate ───────────────────────────────────────────────
                Text("Taxa de quadros", style = MaterialTheme.typography.labelLarge)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    FrameRate.entries.forEach { fps ->
                        FilterChip(
                            selected = selectedFrameRate == fps,
                            onClick = { selectedFrameRate = fps },
                            label = { Text(fps.label) },
                        )
                    }
                }

                // ─── Audio capture ─────────────────────────────────────────────
                HorizontalDivider()
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = "Capturar áudio da tela\n(exclui áudio da chamada de voz)",
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.weight(1f),
                    )
                    Switch(
                        checked = captureAudio,
                        onCheckedChange = { captureAudio = it },
                    )
                }

                Spacer(modifier = Modifier.height(4.dp))
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    onConfirm(
                        ScreenShareConfig(
                            displaySurface = selectedDisplaySurface,
                            resolution = selectedResolution,
                            frameRate = selectedFrameRate,
                            captureAudio = captureAudio,
                            sourceId = selectedSourceId,
                        ),
                    )
                },
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.primary),
            ) {
                Text("Compartilhar")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancelar") }
        },
    )
}
