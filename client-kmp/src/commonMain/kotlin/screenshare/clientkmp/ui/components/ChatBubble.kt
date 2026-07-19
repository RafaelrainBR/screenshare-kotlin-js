package screenshare.clientkmp.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import screenshare.clientkmp.util.getUsernameInitials
import screenshare.common.ChatMessage

@Composable
fun ChatBubble(
    message: ChatMessage,
    localUsername: String,
) {
    val isOwn = message.username == localUsername
    val isSystem = message.username == "Sistema"
    val displayName =
        when {
            isSystem -> "Sistema"
            isOwn -> "Você"
            else -> message.username
        }

    if (isSystem) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
            horizontalArrangement = Arrangement.Center,
        ) {
            Text(
                text = message.content,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        return
    }

    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        horizontalArrangement = if (isOwn) Arrangement.End else Arrangement.Start,
        verticalAlignment = Alignment.Bottom,
    ) {
        if (!isOwn) {
            AvatarCircle(name = message.username)
            Spacer(modifier = Modifier.width(8.dp))
        }

        Column(
            horizontalAlignment = if (isOwn) Alignment.End else Alignment.Start,
        ) {
            Text(
                text = displayName,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 4.dp),
            )
            Box(
                modifier =
                    Modifier
                        .clip(
                            RoundedCornerShape(
                                topStart = 12.dp,
                                topEnd = 12.dp,
                                bottomStart = if (isOwn) 12.dp else 2.dp,
                                bottomEnd = if (isOwn) 2.dp else 12.dp,
                            ),
                        ).background(
                            if (isOwn) {
                                MaterialTheme.colorScheme.primaryContainer
                            } else {
                                MaterialTheme.colorScheme.surfaceVariant
                            },
                        ).padding(horizontal = 12.dp, vertical = 8.dp),
            ) {
                Text(
                    text = message.content,
                    color =
                        if (isOwn) {
                            MaterialTheme.colorScheme.onPrimaryContainer
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }

        if (isOwn) {
            Spacer(modifier = Modifier.width(8.dp))
            AvatarCircle(name = message.username)
        }
    }
}

@Composable
fun AvatarCircle(
    name: String,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier =
            modifier
                .size(28.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.secondaryContainer),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = name.getUsernameInitials(),
            fontSize = 10.sp,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onSecondaryContainer,
        )
    }
}
