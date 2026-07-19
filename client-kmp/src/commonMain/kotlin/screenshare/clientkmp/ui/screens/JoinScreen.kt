package screenshare.clientkmp.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import screenshare.clientkmp.util.generateRandomRoomId

@Composable
fun JoinScreen(
    onJoin: (username: String, roomId: String) -> Unit,
    initialRoomId: String? = null,
) {
    var username by remember { mutableStateOf("") }
    var roomId by remember { mutableStateOf(initialRoomId ?: generateRandomRoomId()) }

    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = "ScreenShare",
            style = MaterialTheme.typography.headlineLarge,
            color = MaterialTheme.colorScheme.primary,
        )

        Spacer(modifier = Modifier.height(32.dp))

        Column(
            modifier = Modifier.widthIn(max = 400.dp).fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            OutlinedTextField(
                value = username,
                onValueChange = { username = it },
                label = { Text("Seu nome") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = roomId,
                onValueChange = { roomId = it },
                label = { Text("ID da sala") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            Button(
                onClick = {
                    val trimmedUsername = username.trim()
                    val trimmedRoomId = roomId.trim().ifEmpty { generateRandomRoomId() }
                    if (trimmedUsername.isNotEmpty()) {
                        onJoin(trimmedUsername, trimmedRoomId)
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                enabled = username.trim().isNotEmpty(),
            ) {
                Text("Entrar")
            }
        }
    }
}
