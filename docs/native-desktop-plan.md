# Desktop nativo — plano da branch nova

Data: 17/10/2026. Base: `master` em `143123d` (0.11.0).

## Objetivo

Um cliente desktop Windows em Rust puro — UI nativa (iced 0.14, decidido no spike F-1) + sessão + libwebrtc no mesmo processo, sem WebView, JVM ou JS — que entra nas salas do servidor atual e conversa com o cliente web. Linha paralela ao Tauri: o Tauri continua sendo a versão oficial até a nativa ser promovida.

Fora de escopo até a promoção: auto-updater, macOS/Linux, migração automática de usuários Tauri.

## Decisões já tomadas

- Sem auto-updater nas versões nativas. Nem a primeira estável nativa terá; um updater próprio, se vier, começa a partir dela, não do Tauri. Ninguém é migrado automaticamente do Tauri.
- Builds nativos saem como **pre-release** (tags `native-v*`), nunca como a release "latest" — o updater do Tauri lê `releases/latest/download/latest.json`. Se esse arquivo sumir, o app Tauri só deixa de mostrar o banner (`runCatching` em `client/src/jsMain/kotlin/Main.kt:141`).
- O motor de compartilhamento do master (captura Rust + ajustes de encoder) é estável e **não é descartado** — ver "Origem de cada peça".
- O protótipo GPUI (`codex/pause-native-gpui-20261005`) era instável; dele só vem o que o master não tem, e a negociação WebRTC e o protocolo são reescritos seguindo o master.

## Framework de UI: iced 0.14 (decidido em 17/10/2026 pelo spike F-1)

Os dois desenham os próprios widgets via GPU (nenhum usa controles do Windows); os dois são "nativos" no sentido de não ter WebView. A troca afeta só a camada de UI: engine, sessão, captura e ajustes de encoder ficam iguais.

| Ponto | iced 0.14 (dez/2025) | GPUI 0.2.2 (pin do protótipo) |
|---|---|---|
| Maturidade/API | "Experimental", pré-1.0; muda a cada versão, com notas de migração | Pré-1.0, publicado esporadicamente, guiado pelas necessidades do Zed |
| Windows | Suportado há anos via winit, wgpu (DX12/Vulkan) + fallback por software | Mais recente (Zed para Windows em 2025) |
| Texto/chat | `text_input`/`text_editor` prontos, IME (preedit/commit do winit) e colar no próprio widget; acento e emoji recebidos renderizam (emoji colorido) | O protótipo precisou de `input.rs` próprio (708 linhas) |
| Vídeo remoto | Spike: `image` + `image::allocate` dá 1080p/30 com ~64% de um núcleo no processo todo (ver F-1); `Shader` fica como otimização, não foi preciso | Protótipo copia cada frame para `RenderImage` na CPU |
| Modelo | Elm; `Subscription` recebe eventos da sessão por canal | Entidades/contexto |
| Tray | Não tem (crate `tray-icon`); API `Daemon` permite viver sem janela | Não tem |
| Acessibilidade | Nenhuma, nem no `master` (0.15.0-dev, `08eb93a`, 17/10/2026): sem `accesskit` no `Cargo.lock`; PRs #1849 e #3111 abertos como rascunho, #3281 fechado sem merge | UIA ausente no Windows (registrado no protótipo) |

Decisão: **iced 0.14**, aprovada em 17/10/2026. Passou nos quatro critérios do spike. Acessibilidade não desempata: nenhum dos dois tem.

### F-1 — Spike do iced (antes da F0)

Branch descartável a partir do master. Objetivo: responder se o iced aguenta este app, não construir o produto.

- Trazer só `native-engine/src/{lib,session,render}.rs`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` do protótipo (`git checkout codex/pause-native-gpui-20261005 -- <caminhos>`). JNI pode ficar compilando ou ser cortado, o que for mais rápido.
- App iced 0.14 mínimo: campo de sala/nome → entra; lista de participantes; chat; exibe o vídeo remoto escolhido.
- O protocolo do protótipo é anterior ao master (sem `room-joined`, manda `supportsIceServers`, espera `ice-servers`). Fazer só o mínimo para entrar numa sala do servidor atual; a reescrita completa é a F2.
- Critérios:
  1. Vídeo de tela do browser em 1080p/30 fluido no iced, com CPU do processo medida (comparar caminho `image` × `Shader` se o `image` não bastar).
  2. Chat aceita acentos por tecla morta (`´` + `a`), `ç`, emoji e colar; Enter envia.
  3. Fechar a janela encerra sessão e engine sem travar.
  4. Conferir no repositório do iced se há AccessKit/acessibilidade no `master` depois da 0.14.
- Resultado: registrar números e decisão neste arquivo e atualizar as tabelas (GPUI → iced, ou manter GPUI). Spike não vira código de produto sem passar pelas fases abaixo.

#### Resultado do spike (17/10/2026, branch `claude/iced-ui-spike-600508`)

Montagem: `native-engine/` com `lib.rs`, `session.rs` e `render.rs` do protótipo, um `capture.rs` stub (o spike só recebe vídeo) e `src/bin/iced-spike.rs` (~250 linhas). O JNI continua compilando. Servidor Ktor local (`:server-java:run`) e o cliente web do master num browser na mesma máquina. A fonte é um canvas 1920×1080/30 com texto rolando, entregue pelo `getDisplayMedia` do browser e enviado com o perfil `SHARPNESS` (`maintain-resolution`) para o browser não reduzir a resolução. Máquina: Xeon E5-2640 v3 (16 threads, 2014), RTX 3060, Windows 10. Havia um jogo aberto durante as medições.

| Critério | Resultado |
|---|---|
| 1. Vídeo 1080p/30 | **Passa.** 1920×1080 decodificado e exibido a 29–31 fps, estável por vários minutos. CPU do processo inteiro: **~64% de um núcleo (4% da máquina)**, 215–225 MB de RAM. A maior thread (~23%) é a decodificação VP8 em software do libwebrtc; a thread de UI do iced fica em ~12%; o resto é I420→RGBA e o runtime. Em 720p/30: ~44% de um núcleo. |
| 2. Teclado no chat | **Passa** (teste manual do usuário: tecla morta, `ç`, emoji pelo `Win + .`, Ctrl+V e Enter). Pelo código, o iced 0.14 converte `Ime::Preedit/Commit` do winit em eventos do `text_input`, trata tecla morta pelo texto composto do `KeyPressed` e cola com Ctrl+V no próprio widget. Mensagens com `ç ã é 😀` vindas do browser chegam e aparecem certas (emoji colorido). Simular teclas com `PostMessage` não serve: o iced não dá foco ao campo sem clique real. E `SendInput` exigiria tirar o foco da janela em uso. |
| 3. Fechar a janela | **Passa.** 9 de 9 fechamentos com vídeo ativo: `session ended: Ok(())`; sessão e engine param em 19–84 ms; o processo sai em 170–510 ms. Caminho: `exit_on_close_request(false)` → `close_requests()` → flag de stop → `join` da thread da sessão → `window::close` + `iced::exit()`. |
| 4. AccessKit no `master` | **Não tem** (ver tabela acima). |

Lições para as fases seguintes:

- **Imagem no iced 0.14 é carregada de forma assíncrona.** Um `image::Handle::from_rgba` novo a cada quadro nunca chega a ser desenhado. O certo é `image::allocate(handle)` e trocar o quadro exibido só quando a alocação volta. Com uma alocação por vez, a tela fica em ~20 fps. Deixando as alocações se sobreporem e descartando as que voltam fora de ordem, chega a 30 fps. O conversor do `render.rs` passou para `i420_to_abgr` (RGBA em memória), então o iced não precisa trocar canais.
- **A negociação travava pelos papéis, não pelo WebRTC.** O protótipo escolhia quem oferece e quem é polite pela comparação dos socket IDs. O master escolhe pela chegada: quem recebe `user-connected` oferece e é impolite; quem recebe `user-list` é polite e espera (`SessionHandler.kt`, `PeerConnections.kt:155`). Com a regra do protótipo, os dois lados às vezes ficavam impolite e ignoravam a oferta um do outro: nada conectava. Copiar a regra do master resolveu (`session.rs`, `ensure`). Isso confirma a reescrita da negociação prevista para a F2.
- Protocolo mínimo para entrar: `clientVersion: 2` no `join-room` e tratar `room-joined`. O resto da F2 continua pendente.
- O browser negociou **VP8**, porque o engine ainda não aplica a ordem de codecs do master. Isso fica para a F3.
- Para medir: janela coberta por outra não é redesenhada pelo Windows, e aba de browser oculta limita `setInterval` a 1 Hz. As duas coisas derrubam o fps e enganam a medição. Gerar os quadros da fonte de teste a partir de um Worker resolve o lado do browser.

Como repetir o teste manual: rodar `iced-spike.exe` sem `SPIKE_AUTO` (essa variável só serve para entrar e assistir sem cliques), entrar na sala `spike` com o browser aberto em `http://localhost:8080/?roomId=spike`, focar o campo de mensagem e digitar `´` + `a`, `ç`, um emoji pelo painel `Win + .` e colar com Ctrl+V. Depois apertar Enter e ver a mensagem chegar no browser.

## O que existe

| Fonte | O que tem | Estado |
|---|---|---|
| `codex/pause-native-gpui-20261005` (base `7061ce1`, 29/09) | `native-engine/`: engine libwebrtc (`lib.rs`, ~2.6k linhas), `session.rs` (WebSocket/protocolo, ~1.8k), `render.rs`, `capture.rs` (WGC/WASAPI), `bin/gpui-smoke.rs` (UI GPUI 0.2.2, ~2k + `input.rs`) | Sessão + mídia sintética aprovadas contra Ktor e browser reais; UI funcional; gates físicos (áudio, soak, UIA/IME, MSI) abertos |
| mesma branch | `desktop-native/` (Kotlin/JNI), `scripts/native/*.ps1`, mudanças em client/server/common, ~4k linhas de docs | **Não reaproveitar** — superado pelo master ou específico do caminho JNI |
| `master` | `desktop-tauri/src-tauri/src/capture/`: DXGI de tela, PrintWindow de janela, loopback do sistema excluindo Discord, áudio por processo, enumeração de fontes, thumbnails | Em produção, mais novo que o `capture.rs` do protótipo |
| `master` | Protocolo e features novas (abaixo) | Fonte da verdade |

### O que o master ganhou depois do protótipo

- Protocolo: `room-joined` (servidor informa o próprio `sid`), `clientVersion` no `join-room`, `clientMessageId` no chat, `request-media-state` / `room-media-state`, câmera (`start/stop-camera-share`). O pacote `ice-servers` do protótipo **não existe** no master.
- Compartilhamento: wizard único de telas/janelas com thumbnails, presets de conteúdo, configurações avançadas de encoder, troca de configuração ao vivo.
- UI v4 "Sala noturna" (Barlow, dock com status/sair, rail espelhado, Enter-para-enviar), painel de tray, autostart, reentrar pela URL, recuperação de permissões.

## Estratégia

1. **Não fazer merge nem rebase da branch antiga.** Ela diverge em 63 arquivos e mexe em client/server/common de um jeito que o master já resolveu diferente. Trazer só os arquivos úteis com `git checkout codex/pause-native-gpui-20261005 -- <caminho>`.
2. **Captura compartilhada, não copiada.** Extrair `capture/` do Tauri para um crate próprio usado pelos dois apps. Assim correções de captura (Discord, DXGI, janelas) valem para as duas linhas. Essa extração é um PR pequeno **direto no master**, antes da branch nativa crescer.
3. **Uma branch longa `native/desktop`** a partir do master, com merges periódicos do master (não rebase) para acompanhar protocolo e captura.

## Origem de cada peça

Regra: **o que já é estável no master manda.** Do protótipo só vem o que o master não tem (engine libwebrtc, transporte WebSocket em Rust, UI do protótipo), e as partes que o log de validação mostrou instáveis são reescritas seguindo o código do master.

Legenda: **Manter** = mesmo código, só muda de lugar · **Portar 1:1** = traduzir para Rust com os mesmos valores e regras · **Copiar** = trazer do protótipo quase sem mudança · **Copiar e revisar** = trazer e ajustar ao master · **Reescrever** = escrever de novo usando o master como referência · **Descartar**.

### Motor de compartilhamento de tela

O motor do Tauri tem três camadas. Só a do meio deixa de existir, porque é a ponte com a WebView:

```
Tauri:   captura Rust ──► ring/IPC ──► WebView (VideoFrame/AudioWorklet) ──► WebRTC do Chromium + ajustes Kotlin
Nativo:  captura Rust ───────────────► libwebrtc (NativeVideoSource/NativeAudioSource) + mesmos ajustes em Rust
```

| Peça | Onde está | Ação | Observação |
|---|---|---|---|
| Captura de tela DXGI, janela PrintWindow (`capture/video.rs`) | master | **Manter** | Vai para `desktop-capture`, sem mudar lógica; o Tauri usa o mesmo crate |
| Áudio do PC sem Discord (`loopback.rs`, `process.rs`) | master | **Manter** | Idem |
| Áudio do app da janela (`process.rs`) | master | **Manter** | Idem |
| Lista de fontes e thumbnails (`sources.rs`, `thumb.rs`) | master | **Manter** | Alimentam o wizard de share |
| Captura macOS (`mac*.rs`) | master | **Manter** | Vai para o crate; o nativo não usa até ter macOS |
| Ring de memória compartilhada (`shared.rs`), `videoWorker.ts`, `audioWorklet.ts`, partes de captura do `desktopBridge.ts` | master | **Descartar no nativo** | São a ponte com a WebView; continuam no Tauri |
| Presets e prioridades (`SharePreset.kt`: GAME/VIDEO/TEXT, `contentHint`, `degradationPreference`, bitrate por altura) | master | **Portar 1:1** | Mesmos números; teste de tabela comparando valores |
| Aplicação do perfil ao vivo (`PeerConnections.kt:64-80`, `setParameters` com `maxBitrate`/`maxFramerate`/`degradationPreference`) | master | **Portar 1:1** | libwebrtc tem as mesmas `RtpEncodingParameters` |
| Ordem de codecs VP9 › H264 › VP8 › AV1 mantendo RTX/RED/ULPFEC (`RTCPeerConnectionDecorator.kt:125-150`) | master | **Portar 1:1** | Substitui o VP8 forçado do protótipo |
| Ajuste Opus no SDP (`AudioQuality.kt`: estéreo, 192 kbps, FEC, DTX) | master | **Portar 1:1** | Função de string; testar com o mesmo SDP de entrada |
| Encoder e transporte WebRTC | WebView2 (Chromium) | **Trocado** por libwebrtc da LiveKit | Mesma biblioteca upstream, build diferente; ver Riscos |
| Captura WGC/WASAPI do protótipo (`native-engine/src/capture.rs`) | protótipo | **Descartar** | Substituída pela captura do master |
| Ponte captura → libwebrtc (`media.rs`) | — | **Escrever** | Pequena: BGRA → I420 com o `yuv_helper` do libwebrtc; f32 → i16 em blocos de 10 ms |

### Sessão, rede e WebRTC

| Peça | Onde está | Ação | Observação |
|---|---|---|---|
| Pin do libwebrtc, dependências e toolchain (`Cargo.toml`, `rust-toolchain.toml`) | protótipo | **Copiar** | Pins mantidos; `windows` alinhado ao do crate de captura |
| Engine: criação de peers, senders/transceivers, fila de comandos, close e ownership (`lib.rs`) | protótipo | **Copiar e revisar** | Tirar JNI; manter a API `RustEngine` |
| Microfone e reprodução pelo ADM, seleção de dispositivos | protótipo | **Copiar** | Comportamento (mute, mic começa desligado) revisado contra `VoiceChat.kt` |
| Configuração/validação de ICE, `ice_generation_matches`, stats | protótipo | **Copiar** | |
| Negociação (perfect negotiation: glare, rollback, renegociação, restart de ICE) | protótipo | **Reescrever** | Foi a parte instável no log (sem bytes enviados após rollback do lado polite, sem oferta após `user-list`, answers fora de ordem). Reescrever seguindo `PeerConnections.kt:240-340` do master, que é estável. O spike achou uma causa: papéis polite/iniciador escolhidos por socket ID em vez de por chegada (ver F-1) |
| Testes de glare/rollback/ICE adiantado da engine | protótipo | **Copiar** | Viram regressão da negociação reescrita |
| Fontes sintéticas (`synthetic-test`) | protótipo | **Copiar** | Para testar sem captura real |
| Transporte WebSocket (`tokio-tungstenite`, TLS, fila limitada, backoff 1/2/4/8/15 s, gerações) | protótipo | **Copiar** | Já aprovado contra Ktor real |
| Tratamento do protocolo em `session.rs` | protótipo | **Reescrever** | Seguindo `Session.kt`/`SessionHandler.kt` do master: `room-joined`, `clientVersion`, `clientMessageId`, estado de mídia, câmera. A heurística de nome único e o pacote `ice-servers` somem |
| Fixtures de protocolo | master (`MessageSpec.kt`) | **Portar 1:1** | JSON literais nos testes Rust |
| Servidor Ktor e cliente web | master | **Manter** | Não mudam. As mudanças de server/client/common da branch antiga são descartadas |

### Interface e app

Tabela ajustada para o iced, decidido no spike F-1.

| Peça | Onde está | Ação | Observação |
|---|---|---|---|
| Campo de texto, foco/Tab, rolagem de histórico, clipboard (`gpui-smoke/input.rs` e helpers) | protótipo | **Descartar** | O iced já tem `text_input`, `scrollable` com `anchor_bottom` e clipboard |
| Superfície de vídeo (`render.rs`: track → I420 → RGBA → `VideoSurface`) | protótipo | **Copiar e revisar** | O spike já usa `i420_to_abgr`. Revisar: tirar a janela Win32 separada e os ramos não embutidos |
| Exibição do vídeo na UI | spike F-1 (`iced-spike.rs`) | **Copiar** | `image::allocate` com alocações sobrepostas, descartando as fora de ordem. `Shader` só se a CPU incomodar |
| Telas e layout | `gpui-smoke.rs` (só referência de fluxo) | **Reescrever** em iced | Módulos pequenos; layout segue a UI v4 |
| Visual e comportamento de referência (`client/src/jsMain/resources/`, `ui/`) | master | **Portar** | Cores, Barlow, dock, rail, Enter-para-enviar |
| Ícones | master | **Copiar** | Os do redesign v4, não os do protótipo |
| Tray, autostart, instância única | master (Tauri) | **Depois do beta** | O iced não tem tray; usar o crate `tray-icon` com a API `Daemon` |
| Ponte Kotlin/JNI (`desktop-native/` Kotlin), `scripts/native/*.ps1`, `browser-fixture.js` | protótipo | **Descartar** | Caminho JNI abandonado; harness PowerShell pesado demais |
| Docs do protótipo (~4k linhas) | protótipo | **Descartar** | Consultar quando precisar; não trazer para a branch |

## Estrutura de crates

```
desktop-capture/            # novo, extraído de desktop-tauri/src-tauri/src/capture
desktop-tauri/src-tauri/    # passa a depender de desktop-capture; shared.rs (ring com WebView) fica aqui
desktop-native/             # novo: engine + sessão + UI (vindo de native-engine/)
  src/engine.rs             # lib.rs do protótipo sem JNI
  src/session.rs
  src/render.rs
  src/media.rs              # ponte capture -> NativeVideoSource/NativeAudioSource
  src/ui/                   # UI iced em módulos (fluxo de referência: gpui-smoke.rs)
  src/main.rs
```

`desktop-native/` substitui o diretório `desktop-native/` Kotlin da branch antiga (que não vem junto). Um `Cargo.toml` de workspace na raiz só se o build ficar mais simples com ele; senão, crates independentes com dependência por `path`.

## Fases

### F0 — Extrair captura (PR no master)

- Mover `capture/{mod,loopback,process,sources,thumb,video}.rs` e `mac*.rs` para `desktop-capture/`. `shared.rs` usa `tauri::webview::PlatformWebview`, então fica no Tauri.
- O Tauri passa a usar `desktop_capture::…`; nenhum comportamento muda.
- Pronto quando: `cargo test` do Tauri e do crate novo passam e o release do Tauri builda.

### F1 — Branch e engine sem JNI

- Criar `native/desktop` do master. Trazer `native-engine/src/{lib,session,render}.rs`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` para `desktop-native/`. Tirar `gpui` e `image` do `Cargo.toml`; o `gpui-smoke` não vem.
- Remover `jni`, `EventSink::Jvm` e os `Java_*`; `RustEngine` vira a única API. Manter a feature `synthetic-test`.
- Remover `capture.rs` do protótipo (substituído por `desktop-capture`).
- Alinhar `windows` para a versão do `desktop-capture` (0.62; o protótipo usa 0.61).
- Pins: iced `0.14` (o spike resolveu `iced 0.14.0`, `iced_widget 0.14.2`, `iced_winit 0.14.1`) e libwebrtc LiveKit `2ff180e`. Atualizar só se não compilar.
- Listar os codecs de vídeo e encoders de hardware que o libwebrtc oferece nesta máquina (VP9, H264, AV1).
- Pronto quando: `cargo test --lib` passa, o binário abre a janela iced e a lista de codecs está registrada.

**Feito (17/10/2026, `native/desktop`).** O crate chama `desktop-native` e o `lib.rs` continua sendo a engine (não virou `engine.rs`: renomear não resolvia nada). Saíram o JNI, o registro de handles, a contagem de tarefas e o `crate-type = cdylib`. O `render.rs` ficou só com o caminho embutido no iced: a thread de decodificação escreve RGBA direto no `VideoSurface`, sem a janela Win32 nem o preview local. Com isso o crate deixou de usar o `windows`. O `capture.rs` virou um stub até a F3. Até lá a UI é a do spike (`main.rs`).
- Codecs de vídeo deste libwebrtc (`get_rtp_sender_capabilities`, coberto por teste): VP9 perfis 0–3, H264 (perfis 42001f, 42e01f, 4d001f, f4001f, modos de pacote 0 e 1), VP8 e AV1, além de RTX, RED, ULPFEC e FlexFEC. Dá para portar 1:1 a ordem VP9 › H264 › VP8 › AV1.
- **Sem encoder de hardware no Windows:** o `webrtc-sys` só compila NVENC/VAAPI no Linux (o bloco VAAPI do Windows está comentado no `build.rs`). Tudo é software (libvpx, OpenH264, AV1). Medir CPU de envio na F3.
- Build: o `target/` dentro deste worktree falha no `webrtc-sys` porque os includes do abseil passam do MAX_PATH. Usar um `CARGO_TARGET_DIR` curto. No CI o caminho já é curto.

### F2 — Protocolo atual em `session.rs`

- `join-room` com `clientVersion` em vez de `supportsIceServers`.
- Identidade por `room-joined` em vez da heurística de nome único com sufixo e retry — apaga código.
- Chat com `clientMessageId` e confirmação, como o cliente web.
- `request-media-state` ao entrar; aplicar `room-media-state`.
- Receber câmera dos outros (`start/stop-camera-share`).
- ICE: sem pacote `ice-servers`; configuração local fora do Git, como o protótipo já fazia com `artifacts/native/private/`.
- Testes: fixtures JSON copiadas de `MessageSpec.kt`; falham se tag ou campo mudar.
- Pronto quando: entra numa sala com o browser, troca chat e vídeo sintético nos dois sentidos.

**Feito (17/10/2026).**
- O que entrou:
  - identidade só por `room-joined`, e por ela os papéis polite/iniciador da negociação;
  - `request-media-state` logo depois do `room-joined`;
  - `room-media-state`, `screen/camera-share-*`, `user-muted/unmuted` e o `isMuted` do `user-list` aplicados na view;
  - chat com `clientMessageId` e confirmação pelo eco com `senderSocketId`; sem eco em 10 s, a view mostra erro;
  - tela × câmera do mesmo peer escolhidas pela regra do `PeerConnections.onTrack`: 1º track de vídeo é a tela, o seguinte é a câmera.
- O que saiu: o pacote `ice-servers`, a heurística de nome único com retry e o modo de teste do protótipo (troca de sala, mídia e chat automáticos). ICE local continua por `SCREENSHARE_ENGINE_CONFIG`, um arquivo fora do Git.
- Testes: fixtures do `MessageSpec.kt` em `session.rs`, mais os testes de integração com servidor fake e engine sintética. Total: 30 testes (34 com `synthetic-test`).
- Fluxo real (Ktor local + browser): entra, recebe tela 1080p e câmera 640×480 do browser a ~30 fps, chat do browser chega. O envio de chat do nativo para o servidor real só foi coberto pelo servidor fake: a UI do spike não tinha como digitar de forma automatizada. Enviar vídeo do nativo fica para a F3, com captura real.

### F3 — Mídia real

- `media.rs` liga `capture_video_loop` (BGRA) ao `NativeVideoSource` do libwebrtc (conversão para I420 pelo próprio libwebrtc) e `AudioChunk` (f32 estéreo, 480 frames = 10 ms) ao `NativeAudioSource` (i16).
- Modos de áudio do share iguais ao Tauri: sem áudio; áudio do aplicativo da janela (`capture_process_loop` incluindo a árvore); áudio do PC sem Discord (`capture_loop` com o PID do Discord excluído).
- Presets, codecs, Opus e configuração ao vivo portados 1:1 do master (tabela acima). O `fps` da captura já é `AtomicU32` lido a cada tick.
- Microfone e reprodução pelo ADM do libwebrtc (já no protótipo).
- Pronto quando: tela, janela com áudio do app e áudio do PC sem Discord chegam ao browser; parar e reiniciar 5 vezes sem vazamento.

**Feito (17/10/2026), exceto o modo áudio do PC.**
- O que entrou:
  - `share.rs` porta `SharePreset.kt`, a ordem de codecs VP9 > H264 > VP8 > AV1 (+rtx/red/ulpfec) e o Opus de `AudioQuality.kt`;
  - `capture.rs` liga o `desktop-capture` ao libwebrtc: BGRA → I420 (`argb_to_i420`) e f32 → i16;
  - `StartScreen{source, profile, height, audioMode}` e `SetShareProfile` ao vivo (fps atômico + `set_parameters`);
  - `ready_to_negotiate` exige só signaling `stable`. Antes exigia `Connected` e travava quando o 1º offer vinha sem mídia.
- Achados do libwebrtc:
  - `sender.track()` dá 0xC0000005 num sender cujo track foi removido (wrapper da LiveKit). A tela entra por `add_transceiver`, que devolve o transceiver, e nada chama `.track()`.
  - Pela JSEP, um transceiver que já enviou mantém MSID e SSRCs em toda oferta até o `stop()`. Só com `remove_track`, o send stream e o encoder VP9 continuavam vivos: **+67 MB, ~+20 handles e ~+3 threads por share**. `active=false` nas encodings não libera nada.
  - Reusar o transceiver (`set_track` + `set_direction`) acaba com o vazamento, mas o browser devolve o mesmo track de recepção, que o cliente web já parou (`ended`), e a tela não reaparece.
  - Solução: `stop()` nos transceivers da tela no stop. As m-lines paradas são recicladas e a oferta fica com tamanho fixo.
  - **Depende do cliente web:** `setVideoReception` atribuía `direction` a transceiver parado, lançava erro e o answer nunca saía. A guarda em `RTCPeerConnectionDecorator.setVideoReception` precisa estar no master (web e Tauri, que embute o cliente) antes de o nativo sair. Cliente sem a guarda trava a negociação com o nativo no 1º stop do share.
- Medido (Ktor local + browser pane, janela de teste 960×572 com áudio do app, 6 ciclos de 10 s):
  - os 6 starts mostram a tela, o áudio chega (nível 0,0183) e o browser volta a `stable` em todos os ciclos;
  - memória privada ~293 MB compartilhando e ~225 MB parado, sem crescer; handles 905–950, threads 90–100;
  - antes da correção: 205 → 605 MB em 6 ciclos;
  - sem peer, a captura sozinha fica estável (~217 MB), então a captura não vazava.
- CPU: retângulo 1000×600 a 30 fps em VP9 usa ~67% de um núcleo. O build Windows não tem encoder de hardware; 1080p é risco no Xeon E5-2640 v3.
- Não testado: modo áudio do PC (capturaria áudio pessoal; precisa de validação manual). O modo tela (retângulo) usa o mesmo caminho de anexar e parar, mas os ciclos foram medidos com janela.
- Testes: 33 (37 com `synthetic-test`).

### F4 — UI

Escrever a UI em iced, em módulos, usando o spike F-1 como ponto de partida e o `gpui-smoke.rs` como referência de fluxo. Visual e comportamento seguem a UI v4 (`client/src/jsMain/resources/`). Escopo do primeiro beta:

- Entrar/sair, participantes com estado (mudo, compartilhando, falando), chat.
- Wizard de share com thumbnails (`enumerate_sources` + `capture_thumbnail`), modo de áudio e preset.
- Dock: microfone, mute, dispositivos, share, sair.
- Receber tela e câmera; escolher qual vídeo ver.

Depois do primeiro beta: enviar câmera, tray/painel, autostart, reentrar pelo link, configurações avançadas de encoder.

Teclado e foco básicos entram no beta. UIA/IME completos são gate de promoção, não de beta.

**Feito (17/10/2026).**
- Módulos em `desktop-native/src/ui/`:
  - `theme.rs`: tokens da v4 como `Theme::custom` e fonte Barlow embutida;
  - `room.rs`: participantes, palco, conversa e dock;
  - `wizard.rs`: assistente de share.
  - O `main.rs` só guarda estado e mensagens. O driver `SPIKE_*` saiu.
- Entrar e sair:
  - foco inicial no nome; o nome padrão é o usuário do Windows;
  - código vazio cria uma sala com 8 hex, como no web;
  - sair derruba a sessão (o `Drop` de `Running`) e volta à tela de entrada.
- Participantes: nome, "(você)", mudo, tela e câmera, com botões "Ver tela"/"Ver câmera". A tela de quem compartilha abre sozinha, como no web, e volta ao vazio quando o share acaba.
- Dock:
  - microfone (F2) e som (F3), cada um com seletor de dispositivo; trocar o dispositivo com o áudio ligado desliga e religa com o novo;
  - compartilhar e parar (F4);
  - Sair com a seta `⇤`. O `↪` caía na fonte de emoji.
- Assistente:
  - miniaturas de `enumerate_sources` + `capture_thumbnail` (carregadas fora da thread da UI), sem as janelas do próprio app;
  - áudio: nenhum, do aplicativo (só para janela) ou do PC sem Discord;
  - presets com os textos do web;
  - teclado: setas, A, P, Enter e Esc.
- Fora do beta, por enquanto: o indicador "falando" (o `SessionView` não tem nível de voz) e a UIA.
- Testes: 2 testes de unidade do assistente (mapeamento fonte → `ScreenOptions`, regra do áudio do app e navegação). Fluxo real com Ktor local + browser:
  - entrar pelo teclado;
  - chat do nativo chega ao browser;
  - tela do browser aparece sozinha no palco;
  - F3 liga e desliga o som;
  - F4 abre o assistente; Esc fecha.
- Não testado:
  - iniciar o share pelo assistente: as miniaturas mostram a tela real do usuário, então não se escolhe fonte às cegas; o caminho do share em si é o da F3;
  - F2: ligaria o microfone de verdade;
  - o mouse: clique postado não funciona no iced, porque o `WM_MOUSELEAVE` do `TrackMouseEvent` fecha o lote de eventos com o cursor fora.

### F5 — Distribuição de teste

- Workflow separado (`release-native.yml`), tags `native-v*`, publicado como **pre-release** para nunca virar a "latest" que o updater do Tauri lê.
- Artefatos: exe portátil e, se quiser, setup NSIS via `cargo-packager`, com identificador diferente do Tauri para os dois poderem estar instalados juntos.
- Sem updater.

**Feito (17/10/2026), sem rodar no CI.**
- `.github/workflows/release-native.yml`:
  - tag `native-vX.Y.Z[-sufixo]`;
  - roda os testes do `desktop-capture` e do `desktop-native` no `windows-latest`;
  - compila e publica `Screenshare-native_<versão>_x64-portable.exe` + `SHA256SUMS` com `--prerelease --latest=false`, e confere que saiu como pre-release.
- O `CARGO_TARGET_DIR` fica em `runner.temp`, pelo MAX_PATH do abseil.
- Config ICE/TURN: o build embute o secret `NATIVE_ENGINE_CONFIG` (`option_env!`) e falha sem ele. O `SCREENSHARE_ENGINE_CONFIG` local continua tendo prioridade.
- O servidor padrão é `wss://screen-share.fly.dev/` (`SCREENSHARE_WS` sobrepõe).
- Antes da primeira tag:
  - criar o secret `NATIVE_ENGINE_CONFIG` com o JSON do engine (`schemaVersion`, `iceServers`, `iceTransportPolicy`);
  - fazer o merge do #26 e sair um release web/Tauri com a guarda.

### F6 — Promoção (decisão manual)

Critérios mínimos: duas betas usadas de verdade sem bug grave; paridade com o Tauri nas features do dia a dia; teclado/IME ok; instalação e desinstalação limpas. Ao promover, avisar usuários Tauri por uma última release Tauri com notas apontando para o nativo. Updater próprio, se houver, começa nessa versão.

## Riscos

- **Duas linhas de produto.** Toda feature nova do Tauri precisa ser portada. Manter o escopo do beta pequeno e fazer merge do master com frequência.
- **Protocolo em dois lugares** (Kotlin `common` e Rust `session.rs`). As fixtures da F2 pegam divergências; mudanças de protocolo no master devem atualizar as duas no mesmo PR quando a branch nativa já estiver ativa.
- **Framework de UI pré-1.0 (iced 0.14).** A API muda entre versões e não há acessibilidade, nem no `master`. Isso é risco para o gate de promoção (F6). Manter o pin; não atualizar sem motivo.
- **libwebrtc.** Build pesado; os binários pré-compilados da LiveKit cobrem Windows x64. Tamanho do exe maior que o do Tauri. O encoder deixa de ser o do Chromium: confirmar cedo (F1) quais codecs e encoders de hardware o build da LiveKit oferece no Windows. Se VP9/H264 por hardware faltarem, a qualidade do share pode ficar diferente da do Tauri mesmo com a mesma captura.
- **Lógica WebRTC duplicada.** A engine Rust reimplementa a negociação que o cliente web faz em `PeerConnections.kt`. O protótipo já cobre glare, ICE adiantado e restart com testes; manter esses testes.

## Próximo passo

F-1 a F5 feitos (F5 sem rodar no CI). Para o primeiro beta: merge do F0 (#25) e da guarda do cliente (#26) no master, release web/Tauri com a guarda, secret `NATIVE_ENGINE_CONFIG`, validar à mão o áudio do PC e o share pelo assistente com mouse, e então a tag `native-v0.1.0`.
