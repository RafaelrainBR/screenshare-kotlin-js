package screenshare.clientkmp.ui.components

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import screenshare.clientkmp.services.AudioDevice
import screenshare.clientkmp.services.DeviceSettings

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsDialog(
    audioInputs: List<AudioDevice>,
    audioOutputs: List<AudioDevice>,
    currentSettings: DeviceSettings,
    onApply: (DeviceSettings) -> Unit,
    onDismiss: () -> Unit,
) {
    var selectedMicId by remember { mutableStateOf(currentSettings.micDeviceId) }
    var selectedOutputId by remember { mutableStateOf(currentSettings.outputDeviceId) }
    var micVolume by remember { mutableFloatStateOf(currentSettings.micVolume) }
    var outputVolume by remember { mutableFloatStateOf(currentSettings.outputVolume) }
    var micMenuExpanded by remember { mutableStateOf(false) }
    var outputMenuExpanded by remember { mutableStateOf(false) }

    fun labelFor(id: String?, devices: List<AudioDevice>, placeholder: String): String =
        devices.firstOrNull { it.id == id }?.label ?: placeholder

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Configurações") },
        text = {
            Column(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // ─── Mic section ──────────────────────────────────────────────
                Text("Microfone", style = MaterialTheme.typography.labelLarge)

                if (audioInputs.isNotEmpty()) {
                    ExposedDropdownMenuBox(
                        expanded = micMenuExpanded,
                        onExpandedChange = { micMenuExpanded = it },
                    ) {
                        OutlinedTextField(
                            value = labelFor(selectedMicId, audioInputs, "Padrão do sistema"),
                            onValueChange = {},
                            readOnly = true,
                            label = { Text("Microfone") },
                            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = micMenuExpanded) },
                            modifier = Modifier.fillMaxWidth().menuAnchor(),
                        )
                        ExposedDropdownMenu(
                            expanded = micMenuExpanded,
                            onDismissRequest = { micMenuExpanded = false },
                        ) {
                            audioInputs.forEach { device ->
                                DropdownMenuItem(
                                    text = { Text(device.label) },
                                    onClick = { selectedMicId = device.id; micMenuExpanded = false },
                                )
                            }
                        }
                    }
                } else {
                    Text(
                        text = "Nenhum microfone encontrado",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = "Volume do microfone",
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                    Text(
                        text = "${(micVolume * 100).toInt()}%",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Slider(
                    value = micVolume,
                    onValueChange = { micVolume = it },
                    valueRange = 0f..1f,
                    modifier = Modifier.fillMaxWidth(),
                )

                HorizontalDivider()

                // ─── Audio output section ─────────────────────────────────────
                Text("Saída de áudio", style = MaterialTheme.typography.labelLarge)

                if (audioOutputs.isNotEmpty()) {
                    ExposedDropdownMenuBox(
                        expanded = outputMenuExpanded,
                        onExpandedChange = { outputMenuExpanded = it },
                    ) {
                        OutlinedTextField(
                            value = labelFor(selectedOutputId, audioOutputs, "Padrão do sistema"),
                            onValueChange = {},
                            readOnly = true,
                            label = { Text("Saída de áudio") },
                            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = outputMenuExpanded) },
                            modifier = Modifier.fillMaxWidth().menuAnchor(),
                        )
                        ExposedDropdownMenu(
                            expanded = outputMenuExpanded,
                            onDismissRequest = { outputMenuExpanded = false },
                        ) {
                            audioOutputs.forEach { device ->
                                DropdownMenuItem(
                                    text = { Text(device.label) },
                                    onClick = { selectedOutputId = device.id; outputMenuExpanded = false },
                                )
                            }
                        }
                    }
                } else {
                    Text(
                        text = "Nenhuma saída de áudio encontrada",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = "Volume de saída",
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                    Text(
                        text = "${(outputVolume * 100).toInt()}%",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Slider(
                    value = outputVolume,
                    onValueChange = { outputVolume = it },
                    valueRange = 0f..1f,
                    modifier = Modifier.fillMaxWidth(),
                )

                Spacer(modifier = Modifier.height(4.dp))
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    onApply(
                        DeviceSettings(
                            micDeviceId = selectedMicId,
                            outputDeviceId = selectedOutputId,
                            micVolume = micVolume,
                            outputVolume = outputVolume,
                        ),
                    )
                },
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.primary),
            ) {
                Text("Aplicar")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancelar") }
        },
    )
}
