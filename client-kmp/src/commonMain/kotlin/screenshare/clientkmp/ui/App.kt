package screenshare.clientkmp.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import screenshare.clientkmp.services.SessionManager
import screenshare.clientkmp.state.AppState
import screenshare.clientkmp.state.Screen
import screenshare.clientkmp.ui.screens.JoinScreen
import screenshare.clientkmp.ui.screens.RoomScreen
import screenshare.clientkmp.ui.theme.ScreenShareTheme

@Composable
fun App(
    sessionManager: SessionManager,
    appState: AppState,
) {
    val screen by appState.screen.collectAsState()
    val videoTrack by sessionManager.currentVideoTrack.collectAsState()

    ScreenShareTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            when (val s = screen) {
                is Screen.Join -> {
                    JoinScreen(
                        initialRoomId = appState.initialRoomId,
                        onJoin = { username, roomId ->
                            sessionManager.joinRoom(username, roomId)
                        },
                    )
                }

                is Screen.Room -> {
                    RoomScreen(
                        state = s.state,
                        videoTrack = videoTrack,
                        onSendMessage = { sessionManager.sendMessage(it) },
                        onToggleMic = { sessionManager.toggleMic() },
                        onStartScreenShare = { config -> sessionManager.startScreenShare(config) },
                        onStopScreenShare = { sessionManager.stopScreenShare() },
                        getScreenSources = { sessionManager.getScreenSources() },
                        getAudioDevices = { sessionManager.getAudioDevices() },
                        onApplySettings = { sessionManager.applyDeviceSettings(it) },
                    )
                }
            }
        }
    }
}
