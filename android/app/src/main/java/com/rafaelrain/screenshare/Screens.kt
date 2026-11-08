package com.rafaelrain.screenshare

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import org.json.JSONObject
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The web's join card (index.html .join-form): logo, title, labelled fields, recent rooms, the two buttons. */
@Composable
fun JoinScreen(linked: String?, join: (String, String) -> Unit) {
    val context = LocalContext.current
    var name by remember { mutableStateOf(Settings.name(context)) }
    var code by remember(linked) { mutableStateOf(linked ?: "") }
    var favorites by remember { mutableStateOf(Settings.favorites(context)) }
    val error = RoomService.view.collectAsStateWithLifecycle().value.error
    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), contentAlignment = Alignment.Center) {
        Column(
            Modifier.widthIn(max = 430.dp).fillMaxWidth().background(Surface, RoundedCornerShape(14.dp)).border(1.dp, Line, RoundedCornerShape(14.dp)).padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Image(painterResource(R.drawable.logo), "Screenshare", Modifier.width(104.dp).padding(bottom = 14.dp))
            Text("Participar de uma sala", style = Body.copy(fontSize = 26.sp, lineHeight = 30.sp, fontWeight = FontWeight.Bold))
            Text("Use um código recebido ou deixe vazio para criar uma sala.", style = Body.copy(color = Dim), modifier = Modifier.padding(bottom = 14.dp))
            Label("Seu nome")
            IconField(name, { name = it.take(32) }, Glyphs.Person, "Como você aparece na sala")
            Spacer(Modifier.height(6.dp))
            Label("Código da sala")
            IconField(code, { code = it.trim().take(64) }, Glyphs.Hash, "ex: 4f8a2c1e", mono = true)
            Hint(if (code.isNotEmpty() && !validRoom(code)) "Use só letras, números, - e _." else "Código opcional ao criar uma sala.", if (code.isNotEmpty() && !validRoom(code)) Red else Faint)
            val recents = Settings.recents(context).sortedByDescending { it in favorites }
            if (recents.isNotEmpty()) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.padding(vertical = 4.dp)) {
                    recents.forEach { room ->
                        val favorite = room in favorites
                        Row(Modifier.background(Surface2, Small).border(1.dp, if (code == room) Accent else Line, Small), verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                room, style = Mono, maxLines = 1, overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.widthIn(max = 170.dp).clickable { code = room }.padding(start = 10.dp, end = 4.dp, top = 9.dp, bottom = 9.dp),
                            )
                            Text(
                                if (favorite) "★" else "☆", style = Body.copy(color = if (favorite) Amber else Faint),
                                modifier = Modifier.clickable(onClickLabel = if (favorite) "Tirar dos favoritos" else "Favoritar") {
                                    Settings.toggleFavorite(context, room)
                                    favorites = Settings.favorites(context)
                                }.padding(horizontal = 10.dp, vertical = 8.dp),
                            )
                        }
                    }
                }
            }
            if (error != null) Hint(error, Red)
            val ready = name.isNotBlank() && (code.isEmpty() || validRoom(code))
            ActionButton(
                "Entrar na sala", Kind.Primary, { join(code.ifEmpty { UUID.randomUUID().toString().take(8) }, name.trim()) },
                enabled = ready, trailing = Glyphs.Arrow, modifier = Modifier.fillMaxWidth().padding(top = 6.dp),
            )
            // A private room's code: all 128 random bits, like generatePrivateRoomId on the web.
            ActionButton("Criar sala privada", Kind.Ghost, { join(UUID.randomUUID().toString().replace("-", ""), name.trim()) }, enabled = name.isNotBlank(), modifier = Modifier.fillMaxWidth())
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RoomScreen(room: String, askShare: (ShareChoice) -> Unit) {
    val context = LocalContext.current
    val view by RoomService.view.collectAsStateWithLifecycle()
    val service = RoomService.instance
    var focused by remember { mutableStateOf<Pair<String, Boolean>?>(null) }
    var sheet by remember { mutableStateOf<String?>(null) }
    val videos = view.videos
    if (focused != null && focused !in videos) focused = null
    LaunchedEffect(videos.size) { (context as? MainActivity)?.let { it.setPictureInPictureParams(it.pipParams()) } }

    Column(Modifier.fillMaxSize()) {
        // The web's conversation header: SALA + the code in mono; a tap sends the link (WhatsApp and the like).
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(
                Modifier.weight(1f, fill = false).background(Surface, Small).border(1.dp, Line, Small)
                    .clickable(onClickLabel = "Convidar para a sala") {
                        val link = "https://screen-share.fly.dev/?roomId=$room"
                        context.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link), "Convidar para a sala"))
                    }.padding(horizontal = 10.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("SALA", style = Body.copy(fontSize = 10.sp, color = Faint, letterSpacing = 1.sp))
                Spacer(Modifier.width(8.dp))
                Text(room, style = Mono, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                Spacer(Modifier.width(8.dp))
                Icon(Glyphs.Copy, null, tint = Dim, modifier = Modifier.size(15.dp))
            }
            Spacer(Modifier.weight(0.01f).width(8.dp))
            var seen by remember { mutableStateOf(view.messageCount) }
            if (sheet == "chat") seen = view.messageCount
            Box(Modifier.clickable(onClickLabel = "Conversa") { sheet = "chat" }.padding(8.dp)) {
                Icon(Glyphs.Chat, "Conversa", tint = Dim, modifier = Modifier.size(20.dp))
                if (view.messageCount > seen) Box(Modifier.align(Alignment.TopEnd).size(8.dp).background(Accent, RoundedCornerShape(50)))
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.clickable { sheet = "people" }.padding(8.dp)) {
                Icon(Glyphs.People, null, tint = Dim, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(if (view.joined) "${view.people.size}" else "…", style = Body.copy(color = Dim))
            }
        }
        if (!view.joined) Hint("Conectando à sala…", Dim, Modifier.padding(horizontal = 14.dp))

        val notices = listOfNotNull(
            view.screenError,
            "Nenhum som do celular está chegando ao compartilhamento. Alguns apps não permitem capturar o áudio.".takeIf { view.sharing && service?.shareAudioSilent == true },
        )
        notices.forEach { GlassChip(it, Amber, AmberLine, Glyphs.Alert, Modifier.padding(horizontal = 12.dp, vertical = 2.dp)) }

        Box(Modifier.weight(1f).fillMaxWidth().padding(10.dp)) {
            val shown = focused?.let { listOf(it) } ?: videos
            if (shown.isEmpty()) {
                Column(
                    Modifier.fillMaxSize().background(VideoBg, Regular).border(1.dp, Line, Regular).padding(24.dp),
                    verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(if (view.joined) "Ninguém está compartilhando" else "Conectando…", style = Body.copy(fontWeight = FontWeight.Bold))
                    Spacer(Modifier.height(6.dp))
                    Text(
                        if (view.joined) "Toque em compartilhar para mostrar a tela do seu celular." else "Entrando na sala $room.",
                        style = Body.copy(color = Dim, textAlign = androidx.compose.ui.text.style.TextAlign.Center),
                    )
                }
            } else {
                val tile: @Composable (Pair<String, Boolean>) -> Unit = { key ->
                        val (peer, camera) = key
                        val local = peer == view.localId
                        val border = when {
                            peer in view.speaking -> Green
                            focused == key -> Accent
                            else -> Line
                        }
                        Box(
                            Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(VideoBg, Regular).border(1.dp, border, Regular)
                                .clickable(onClickLabel = if (focused == key) "Voltar à grade" else "Focar") { focused = if (focused == key) null else key },
                        ) {
                            VideoTile(peer, camera, fit = true, modifier = Modifier.fillMaxSize().padding(1.dp))
                            val name = if (local) "Você" else view.name(peer)
                            // "Por que está travando?": the sharer's own limit on our tile, theirs on theirs.
                            val problem = if (local && !camera) shareBadge(view.localShareProblem) else peerBadge(view.shareReports[peer], name).takeIf { !camera }
                            if (problem != null) GlassChip(problem, Amber, AmberLine, Glyphs.Alert, Modifier.align(Alignment.TopStart).padding(8.dp))
                            GlassChip(name + if (camera) " · câmera" else "", modifier = Modifier.align(Alignment.BottomStart).padding(8.dp))
                        }
                }
                // One video sits in the middle of the stage, like the web's focus mode.
                if (shown.size == 1) Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { tile(shown[0]) }
                else LazyVerticalGrid(GridCells.Adaptive(280.dp), verticalArrangement = Arrangement.spacedBy(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(shown, key = { "${it.first}:${it.second}" }) { tile(it) }
                }
            }
        }

        // The web dock: square controls on the bg, a line on top; leaving is the red arrow at the end.
        HorizontalDivider(color = Line)
        Row(Modifier.fillMaxWidth().background(Bg).padding(horizontal = 12.dp, vertical = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally)) {
            DockButton(if (view.microphoneEnabled) Glyphs.Mic else Glyphs.MicOff, if (view.microphoneEnabled) "Desligar microfone" else "Ligar microfone", muted = !view.microphoneEnabled) {
                if (context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) service?.setMicrophone(!view.microphoneEnabled)
                else (context as? ComponentActivity)?.requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), 1)
            }
            DockButton(if (view.sharing) Glyphs.StopScreen else Glyphs.Screen, if (view.sharing) "Parar compartilhamento" else "Compartilhar a tela", active = view.sharing) {
                if (view.sharing) service?.stopShare() else sheet = "share"
            }
            val cameraOn = view.cameraState == "active" || view.cameraState == "pending"
            DockButton(if (cameraOn) Glyphs.Camera else Glyphs.CameraOff, if (cameraOn) "Desligar câmera" else "Ligar câmera", active = cameraOn) {
                if (cameraOn) service?.stopCamera() else sheet = "camera"
            }
            var speaker by remember { mutableStateOf(service?.speakerWanted != false) }
            DockButton(if (speaker) Glyphs.Speaker else Glyphs.Earpiece, if (speaker) "Usar o fone do celular" else "Usar o alto-falante") {
                service?.toggleSpeaker()
                speaker = service?.speakerWanted != false
            }
            DockButton(Glyphs.People, "Pessoas e ajustes") { sheet = "people" }
            DockButton(Glyphs.Leave, "Sair da sala", danger = true) { service?.leave() }
        }
    }

    if (sheet != null) {
        ModalBottomSheet({ sheet = null }, containerColor = Surface, contentColor = TextColor) {
            when (sheet) {
                "share" -> ShareSheet { sheet = null; askShare(it) }
                "camera" -> CameraSheet { lens ->
                    sheet = null
                    if (context.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) service?.startCamera(lens)
                    else (context as? ComponentActivity)?.requestPermissions(arrayOf(Manifest.permission.CAMERA), 2)
                }
                "chat" -> ChatSheet(view)
                else -> PeopleSheet(view)
            }
            Spacer(Modifier.navigationBarsPadding().height(16.dp))
        }
    }
}

@Composable
fun SheetTitle(text: String) = Text(text, style = Body.copy(fontSize = 20.sp, fontWeight = FontWeight.Bold), modifier = Modifier.padding(bottom = 4.dp))

/** The web's share menu: what you will show (presets), quality and the phone's audio. */
@Composable
fun ShareSheet(start: (ShareChoice) -> Unit) {
    val context = LocalContext.current
    var choice by remember { mutableStateOf(Settings.lastShare(context)) }
    Column(Modifier.padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        SheetTitle("Compartilhar a tela")
        Label("O que você vai mostrar")
        Segmented(listOf("game" to "Jogo", "video" to "Vídeo", "text" to "Texto"), choice.preset) { choice = choice.copy(preset = it) }
        Hint(
            when (choice.preset) {
                "game" -> "60 qps: movimento mais fluido."
                "text" -> "Prioriza a nitidez das letras."
                else -> "30 qps, equilíbrio entre nitidez e fluidez."
            },
        )
        Label("Qualidade")
        Segmented(listOf(720 to "720p", 1080 to "1080p"), choice.shortSide) { choice = choice.copy(shortSide = it) }
        Row(Modifier.fillMaxWidth().clickable { choice = choice.copy(audio = !choice.audio) }, verticalAlignment = Alignment.CenterVertically) {
            Checkbox(choice.audio, { choice = choice.copy(audio = it) }, colors = CheckboxDefaults.colors(checkedColor = Accent, checkmarkColor = OnAccent, uncheckedColor = Line2))
            Text("Incluir o áudio do celular", style = Body)
        }
        if (choice.audio) Hint("Use fone para o som do jogo não vazar no microfone.")
        // share.rs share_bitrate: up to 2.5 Mb/s at 720p and 6 Mb/s at 1080p, per viewer.
        val mbPerMinute = ((if (choice.shortSide <= 720) 2.5 else 6.0) * 60 / 8).toInt()
        Hint("Nos dados móveis, cada pessoa assistindo gasta até cerca de $mbPerMinute MB por minuto da sua franquia.")
        ActionButton("Compartilhar", Kind.Primary, { start(choice) }, trailing = Glyphs.Arrow, modifier = Modifier.fillMaxWidth().padding(top = 6.dp))
    }
}

@Composable
fun CameraSheet(start: (String) -> Unit) {
    Column(Modifier.padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        SheetTitle("Câmera")
        ActionButton("Câmera frontal", Kind.Primary, { start("front") }, modifier = Modifier.fillMaxWidth())
        ActionButton("Câmera traseira", Kind.Secondary, { start("back") }, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
fun SettingRow(text: String, on: Boolean, change: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).clickable { change(!on) }, verticalAlignment = Alignment.CenterVertically) {
        Text(text, style = Body, modifier = Modifier.weight(1f))
        Switch(on, change, colors = SwitchDefaults.colors(checkedTrackColor = Accent, checkedThumbColor = OnAccent, uncheckedTrackColor = Bg, uncheckedBorderColor = Line2, uncheckedThumbColor = Dim))
    }
}

/** "Na sala" with each person's voice and screen volume ("Volume para você"), then the settings. */
@Composable
fun PeopleSheet(view: RoomView) {
    val context = LocalContext.current
    val service = RoomService.instance
    val volumes = remember { mutableStateMapOf<String, Float>() }
    val muted = remember { mutableStateMapOf<String, Boolean>() }
    var sounds by remember { mutableStateOf(Settings.sounds(context)) }
    var metrics by remember { mutableStateOf(Settings.metrics(context)) }
    val slider = SliderDefaults.colors(thumbColor = Accent, activeTrackColor = Accent, inactiveTrackColor = Line)
    Column(Modifier.padding(horizontal = 20.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SheetTitle("Na sala · ${view.people.size}")
        view.people.forEach { person ->
            val local = person.id == view.localId
            Column(Modifier.fillMaxWidth().background(Surface2, Regular).border(1.dp, Line, Regular).padding(12.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Avatar(person.name, person.id in view.speaking)
                    Spacer(Modifier.width(10.dp))
                    Text(person.name + if (local) " (você)" else "", style = Body, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (person.id in view.screenSharers || (local && view.sharing)) Icon(Glyphs.Screen, "Compartilhando a tela", tint = Dim, modifier = Modifier.size(17.dp))
                    Spacer(Modifier.width(6.dp))
                    val micOff = if (local) !view.microphoneEnabled else person.muted
                    Icon(if (micOff) Glyphs.MicOff else Glyphs.Mic, if (micOff) "Microfone desligado" else "Microfone ligado", tint = if (micOff) Red else Dim, modifier = Modifier.size(17.dp))
                }
                if (!local) {
                    SettingRow("Silenciar para você", muted[person.id] == true) {
                        muted[person.id] = it
                        service?.send(JSONObject().put("type", "peerMuted").put("peerId", person.id).put("muted", it))
                    }
                    Hint("Volume da voz")
                    Slider(volumes[person.id] ?: 1f, {
                        volumes[person.id] = it
                        service?.send(JSONObject().put("type", "peerVolume").put("peerId", person.id).put("volume", it.toDouble()).put("screen", false))
                    }, colors = slider)
                    if (person.id in view.screenSharers) {
                        Hint("Volume da tela")
                        Slider(volumes[person.id + ":screen"] ?: 1f, {
                            volumes[person.id + ":screen"] = it
                            service?.send(JSONObject().put("type", "peerVolume").put("peerId", person.id).put("volume", it.toDouble()).put("screen", true))
                        }, colors = slider)
                    }
                }
            }
        }
        SheetTitle("Ajustes")
        SettingRow("Sons de entrar, sair e compartilhar", sounds) { sounds = it; Settings.setSounds(context, it) }
        SettingRow("Enviar estatísticas de uso anônimas", metrics) {
            metrics = it
            Settings.setMetrics(context, it)
            com.rafaelrain.screenshare.engine.Engine.send("""{"metrics":$it}""")
        }
        UpdateRow()
    }
}

/** "Versão X" and, when GitHub has a newer final release, the button that installs it. */
@Composable
fun UpdateRow() {
    val context = LocalContext.current
    var update by remember { mutableStateOf<Update?>(null) }
    var status by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) { update = withContext(Dispatchers.IO) { Updater.check() } }
    Hint("Versão ${BuildConfig.VERSION_NAME}" + (update?.let { " · nova versão ${it.version}" } ?: ""))
    update?.let { found ->
        Hint("Na primeira atualização, o Android pede para liberar \"instalar apps desconhecidos\" para o Screenshare.")
        ActionButton("Atualizar para ${found.version}", Kind.Primary, {
            status = "Baixando…"
            scope.launch {
                runCatching { withContext(Dispatchers.IO) { Updater.download(context, found) } }
                    .onSuccess { status = null; context.startActivity(it) }
                    .onFailure { status = "Não deu para atualizar: ${it.message}" }
            }
        }, modifier = Modifier.fillMaxWidth())
    }
    status?.let { Hint(it, Dim) }
}

/** The room's conversation: history, a field and Enviar (the web's chat, last in priority). */
@Composable
fun ChatSheet(view: RoomView) {
    var text by remember { mutableStateOf("") }
    Column(Modifier.padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SheetTitle("Conversa")
        Column(Modifier.heightIn(max = 360.dp).verticalScroll(rememberScrollState(Int.MAX_VALUE)), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            if (view.messages.isEmpty()) Hint("Nenhuma mensagem ainda.")
            view.messages.forEach { message ->
                Column(Modifier.fillMaxWidth(), horizontalAlignment = if (message.mine) Alignment.End else Alignment.Start) {
                    if (!message.mine) Hint(message.name.ifEmpty { "Sistema" })
                    Text(
                        message.text, style = Body.copy(color = if (message.mine) androidx.compose.ui.graphics.Color.White else TextColor),
                        modifier = Modifier.widthIn(max = 300.dp).background(if (message.mine) androidx.compose.ui.graphics.Color(0xFF2649CC) else Surface2, Regular).padding(horizontal = 12.dp, vertical = 8.dp),
                    )
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            IconField(text, { text = it.take(4000) }, Glyphs.Chat, "Mensagem", modifier = Modifier.weight(1f))
            ActionButton("Enviar", Kind.Primary, {
                if (text.isNotBlank() && RoomService.instance?.send(JSONObject().put("type", "chat").put("text", text.trim())) == null) text = ""
            }, enabled = text.isNotBlank())
        }
    }
}
