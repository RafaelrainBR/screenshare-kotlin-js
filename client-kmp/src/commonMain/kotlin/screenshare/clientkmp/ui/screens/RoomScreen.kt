package screenshare.clientkmp.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import screenshare.clientkmp.services.AudioDevice
import screenshare.clientkmp.services.DeviceSettings
import screenshare.clientkmp.services.ScreenShareConfig
import screenshare.clientkmp.services.ScreenSource
import screenshare.clientkmp.state.RoomState
import screenshare.clientkmp.ui.components.ChatPanel
import screenshare.clientkmp.ui.components.ScreenShareArea
import screenshare.clientkmp.ui.components.ScreenShareConfigDialog
import screenshare.clientkmp.ui.components.SettingsDialog
import screenshare.clientkmp.ui.components.Toolbar
import screenshare.clientkmp.ui.components.UserListPanel

@Composable
fun RoomScreen(
    state: RoomState,
    videoTrack: Any?,
    onSendMessage: (String) -> Unit,
    onToggleMic: () -> Unit,
    onStartScreenShare: (ScreenShareConfig) -> Unit,
    onStopScreenShare: () -> Unit,
    getScreenSources: suspend () -> List<ScreenSource>,
    getAudioDevices: suspend () -> Pair<List<AudioDevice>, List<AudioDevice>>,
    onApplySettings: (DeviceSettings) -> Unit,
) {
    var showScreenShareDialog by remember { mutableStateOf(false) }
    var showSettingsDialog by remember { mutableStateOf(false) }
    var screenSources by remember { mutableStateOf<List<ScreenSource>>(emptyList()) }
    var audioDevices by remember { mutableStateOf<Pair<List<AudioDevice>, List<AudioDevice>>>(Pair(emptyList(), emptyList())) }
    var isLocallySharing by remember { mutableStateOf(false) }

    // Load screen sources when the dialog is opened
    if (showScreenShareDialog) {
        LaunchedEffect(Unit) {
            screenSources = getScreenSources()
        }
        ScreenShareConfigDialog(
            screenSources = screenSources,
            onConfirm = { config ->
                showScreenShareDialog = false
                isLocallySharing = true
                onStartScreenShare(config)
            },
            onDismiss = { showScreenShareDialog = false },
        )
    }

    // Load audio devices when settings dialog is opened
    if (showSettingsDialog) {
        LaunchedEffect(Unit) {
            audioDevices = getAudioDevices()
        }
        SettingsDialog(
            audioInputs = audioDevices.first,
            audioOutputs = audioDevices.second,
            currentSettings = DeviceSettings(),
            onApply = { settings ->
                showSettingsDialog = false
                onApplySettings(settings)
            },
            onDismiss = { showSettingsDialog = false },
        )
    }

    Column(modifier = Modifier.fillMaxSize()) {
        // Header
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
        ) {
            Text(
                text = "Sala: ${state.roomId}",
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }

        HorizontalDivider(color = MaterialTheme.colorScheme.outline)

        // Main content
        Row(modifier = Modifier.fillMaxSize()) {
            // Left column (70%) — screen share + chat
            Column(
                modifier =
                    Modifier
                        .weight(0.7f)
                        .fillMaxHeight(),
            ) {
                Box(modifier = Modifier.weight(0.5f).fillMaxWidth()) {
                    ScreenShareArea(
                        sharerSocketId = state.currentSharerSocketId,
                        videoTrack = videoTrack,
                    )
                }

                HorizontalDivider(color = MaterialTheme.colorScheme.outline)

                Box(modifier = Modifier.weight(0.5f).fillMaxWidth()) {
                    ChatPanel(
                        messages = state.chatMessages,
                        localUsername = state.username,
                        onSendMessage = onSendMessage,
                    )
                }
            }

            VerticalDivider(color = MaterialTheme.colorScheme.outline)

            // Right column (30%) — user list + toolbar
            Column(
                modifier =
                    Modifier
                        .width(260.dp)
                        .fillMaxHeight(),
            ) {
                Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                    UserListPanel(
                        users = state.users,
                        localUsername = state.username,
                        speakingUsers = state.speakingUsers,
                    )
                }

                HorizontalDivider(color = MaterialTheme.colorScheme.outline)

                Toolbar(
                    isMicMuted = state.isMicMuted,
                    isSharing = isLocallySharing,
                    onToggleMic = onToggleMic,
                    onStartScreenShare = { showScreenShareDialog = true },
                    onStopScreenShare = {
                        isLocallySharing = false
                        onStopScreenShare()
                    },
                    onOpenSettings = { showSettingsDialog = true },
                )
            }
        }
    }
}
