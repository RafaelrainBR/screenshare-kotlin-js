package screenshare.clientkmp.ui.components

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.MicOff
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import screenshare.common.SocketUser

@Composable
fun UserListItem(
    user: SocketUser,
    isLocal: Boolean,
    isSpeaking: Boolean,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val borderColor =
                if (isSpeaking) {
                    MaterialTheme.colorScheme.tertiary
                } else {
                    MaterialTheme.colorScheme.outline
                }

            Box(
                modifier =
                    Modifier
                        .size(32.dp)
                        .border(width = 2.dp, color = borderColor, shape = CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                AvatarCircle(name = user.username, modifier = Modifier.size(28.dp))
            }

            Spacer(modifier = Modifier.width(8.dp))

            Text(
                text = if (isLocal) "${user.username} (Você)" else user.username,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }

        Icon(
            imageVector = if (user.isMuted) Icons.Filled.MicOff else Icons.Filled.Mic,
            contentDescription = if (user.isMuted) "Mudo" else "Com áudio",
            tint =
                if (user.isMuted) {
                    MaterialTheme.colorScheme.error
                } else {
                    MaterialTheme.colorScheme.tertiary
                },
            modifier = Modifier.size(18.dp),
        )
    }
}
