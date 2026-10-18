# Kotlin/JS, WebRTC e captura nativa na mesma sala

**Rascunho para revisão — 18/10/2026. Não publicado.** Leitura do código no commit `5c8f4374dd85977477f99b3dffbc45272b0e0a4d`; os links de fonte estão fixados nesse commit. A referência de distribuição é a [release estável v0.11.1](https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/tag/v0.11.1). Este texto descreve a implementação; não é um benchmark nem uma validação de hardware.

O Screenshare permite entrar numa sala por código, compartilhar tela e conversar. O cliente é Kotlin/JS; no desktop, o mesmo cliente recebe tracks de captura nativa por uma ponte Tauri. A fronteira central é uma API que o WebRTC já entende: `MediaStreamTrack`.

## Sinalizar e transportar são trabalhos diferentes

O [protocolo compartilhado](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/common/src/commonMain/kotlin/screenshare/common/Packet.kt) define pacotes serializáveis para entrada, mensagens, SDP, candidatos ICE e estado de compartilhamento. O [Room no servidor](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/server/src/jvmMain/kotlin/screenshare/server/Room.kt) encaminha descrições e candidatos ao participante de destino e distribui chat e mudanças da sala.

Vídeo e áudio seguem pelas conexões WebRTC dos clientes. Quando a conectividade direta falha, um TURN configurado pode retransmitir a mídia. Isso não torna o Ktor um servidor de mídia. A arquitetura também não oferece o comportamento de um SFU: mais peers aumentam o trabalho e a banda dos clientes. O [guia de hospedagem](../self-hosting.md#stun-and-turn) explica a configuração e as limitações operacionais.

Essa separação evita tratar chat e mídia como se tivessem as mesmas garantias. O chat fica em memória do processo e não deve ser apresentado como arquivo durável. Conhecer o código permite entrar na sala; o aplicativo não implementa contas ou senha por sala.

## Uma track, duas origens

No navegador, [ScreenSharing](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/client/src/jsMain/kotlin/services/ScreenSharing.kt) pede a captura com `getDisplayMedia`. Com uma fonte nativa selecionada, ele chama `DesktopCaptureBridge.startVideoCapture` e monta um stream a partir da track recebida. A escolha da origem acontece antes de adicionar o stream à conexão.

No desktop, a [ponte TypeScript](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/desktop-tauri/src/desktopBridge.ts) adapta frames ao mecanismo disponível na WebView. O caminho Windows usa frames BGRA em memória compartilhada e `MediaStreamTrackGenerator`. O caminho WebKit usa um worker com `VideoTrackGenerator` para vídeo e um `AudioWorklet` ligado a um destino de áudio para gerar a track de áudio. As implementações de plataforma ficam no [módulo Rust de captura](https://github.com/RafaelrainBR/screenshare-kotlin-js/tree/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/desktop-capture/src).

A consequência prática é que [PeerConnections](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/client/src/jsMain/kotlin/services/PeerConnections.kt) pode adicionar as tracks de tela, microfone e câmera sem implementar uma conexão distinta para cada capturador. Isso não elimina as diferenças de plataforma: consentimento, seleção da fonte, APIs disponíveis e encerramento continuam precisando de tratamento próprio.

O código de captura nativa não usa o áudio do seletor do navegador como substituto silencioso de uma escolha desktop que falhou. O modo explícito de áudio determina a origem, e um erro recuperável mantém o vídeo sem publicar uma faixa de áudio não escolhida. Essa é uma decisão de comportamento observável, não uma garantia de que qualquer fonte de áudio funcionará.

## Compartilhar também muda uma conexão em andamento

Uma pessoa pode começar a transmitir depois que a conexão já existe. O gerenciamento de peers adiciona tracks ainda ausentes e dispara negociação; também mantém os estados `makingOffer` e `isPolite`. Na colisão entre ofertas, o lado educado faz rollback e responde, enquanto o outro ignora a oferta conflitante. Há [testes específicos de negociação](https://github.com/RafaelrainBR/screenshare-kotlin-js/blob/5c8f4374dd85977477f99b3dffbc45272b0e0a4d/client/src/jsTest/kotlin/services/NegotiationTest.kt) para esse comportamento. A presença dos testes não substitui uma sessão real em redes e WebViews diferentes; não foram executados para redigir este artigo.

Os perfis de transmissão aplicam taxa de quadros, bitrate e preferência de degradação aos senders. Configurar até 60 fps não comprova que o receptor os receberá. Fonte, encoder, rede e dispositivo continuam determinando o resultado. Uma demonstração deve informar a plataforma e mostrar o que realmente aconteceu.

## O que o desktop ainda exige

Os pacotes atuais são Windows x64 e macOS Apple Silicon. Os builds Windows são sem assinatura de publicador; o macOS usa assinatura ad hoc sem notarização, com permissões de captura que podem exigir recuperação após atualização. Áudio por aplicativo no macOS requer 14.2 ou superior. Essas condições precisam aparecer antes do download, conforme o [guia de uso](../usage.md).

A decisão de manter o cliente e as conexões comuns preserva uma fronteira pequena: entregar tracks à aplicação. O custo permanece na ponte entre as capacidades reais de WebView2, WebKit e captura do sistema. Feedback útil para esta arquitetura é sobre essa fronteira e seus testes, sem confundir portabilidade do código Kotlin com igualdade das APIs de cada plataforma.
