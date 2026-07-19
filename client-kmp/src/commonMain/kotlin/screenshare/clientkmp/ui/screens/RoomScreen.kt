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
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import screenshare.clientkmp.state.RoomState
import screenshare.clientkmp.ui.components.ChatPanel
import screenshare.clientkmp.ui.components.ScreenShareArea
import screenshare.clientkmp.ui.components.Toolbar
import screenshare.clientkmp.ui.components.UserListPanel

@Composable
fun RoomScreen(
    state: RoomState,
    videoTrack: Any?,
    onSendMessage: (String) -> Unit,
    onToggleMic: () -> Unit,
    onStartScreenShare: () -> Unit,
    onStopScreenShare: () -> Unit,
) {
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
                    isSharing = state.currentSharerSocketId != null,
                    onToggleMic = onToggleMic,
                    onStartScreenShare = onStartScreenShare,
                    onStopScreenShare = onStopScreenShare,
                )
            }
        }
    }
}
