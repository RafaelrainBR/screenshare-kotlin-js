<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="client/src/jsMain/resources/brand/screenshare-logo-on-dark.svg" />
  <img src="client/src/jsMain/resources/brand/screenshare-logo.svg" alt="Screenshare" width="144" />
</picture>

# Screenshare

**Compartilhe a tela e converse na mesma sala.**

Compartilhamento de tela, voz, câmera e chat pelo navegador ou pelo aplicativo nativo para Windows e macOS.

**[Abrir no navegador](https://screen-share.fly.dev/)** · **[Baixar o app](https://screen-share.fly.dev/about/download.html)** · [Site e demonstração](https://screen-share.fly.dev/about) · [English](README.md)

<a href="https://screen-share.fly.dev/about#demo"><img src="server/src/jvmMain/resources/presentation/assets/demo-poster.jpg" alt="Assistir à demonstração real: duas pessoas, código Kotlin compartilhado e conversa" width="1000" /></a>

</div>

## Experimente

Veja a **[demonstração curta](https://screen-share.fly.dev/about#demo)** ou [baixe o MP4 legendado](https://screen-share.fly.dev/about/assets/demo-pt.mp4). É uma sessão local real no navegador; não demonstra áudio nativo nem mede desempenho da transmissão.

1. Abra o [aplicativo web](https://screen-share.fly.dev/) ou instale o [app para Windows ou Mac](https://screen-share.fly.dev/about/download.html).
2. Digite seu nome. Deixe o código em branco para criar uma sala, cole o código de uma sala existente ou use **Criar sala privada** para um código longo que ninguém adivinha.
3. Clique no código no cabeçalho da conversa para copiar o link da sala e envie às pessoas que vão participar. O link abre a sala no navegador; no Windows, a página também oferece abrir no app.
4. Compartilhe a tela, ative o microfone ou a câmera e converse.

A interface está em português e não exige conta. Quem tiver o código ou o link pode entrar na sala; compartilhe-o com pessoas de confiança.

## Recursos

- **Tela e janelas:** perfis para jogo, vídeo e código/texto, com ajuste de quadros por segundo, bitrate e prioridade entre nitidez e fluidez. Quando a transmissão chega ao limite, um clique a deixa mais leve.
- **Voz e câmera:** controles independentes, seleção de dispositivos, silenciamento de entrada/saída, volume por participante e redução de ruído do microfone no navegador.
- **Várias transmissões:** grade ou tela em foco com faixa de prévias, nomes nas transmissões, janela flutuante e conversa lateral que pode ser ocultada.
- **Chat e quem está na sala:** mensagens da sala, incluindo o histórico enquanto ela continua ativa. Um som curto avisa quando alguém entra, sai ou começa a compartilhar.
- **Salas recentes e favoritas** na tela de entrada.
- **App nativo** (Rust e iced, sem WebView): miniaturas de telas e janelas, áudio do computador ou de um aplicativo só, codificação H.264 por hardware com NVENC ou Media Foundation no Windows, controles na bandeja, início opcional com o sistema e atualização com um clique.
- **Hospedagem própria:** servidor Kotlin/Ktor para salas, chat e sinalização. A mídia usa WebRTC direto quando possível e relay TURN quando necessário.

Resolução, áudio e desempenho dependem do sistema, navegador, fonte, rede e hardware. O ajuste permite até 60 fps, sem garantir essa taxa em todos os cenários.

## Navegador ou app?

O navegador não exige instalação e usa seu próprio seletor de compartilhamento. A disponibilidade de áudio depende da fonte escolhida e do suporte da plataforma.

O app é um cliente nativo separado, que entra nas mesmas salas do navegador. Ele oferece seleção nativa de telas/janelas e áudio por aplicativo. Os downloads incluem **Windows x64** (instalador por usuário e EXE portátil) e **macOS Apple Silicon** (app com script de instalação). Todos se atualizam pelo menu da bandeja. As versões Windows não têm assinatura de código; a versão macOS usa um certificado próprio, sem notarização.

O **app Android** (beta, Android 10 ou mais novo) compartilha a tela do celular com o som dos apps, fica na sala em segundo plano com uma notificação e assiste numa janela flutuante. Ele sai como APK no GitHub Releases e se atualiza sozinho.

Leia o [guia de instalação, permissões e limitações](docs/usage.md) (em inglês).

## Privacidade

A mídia vai direto entre os participantes, ou por um relay TURN quando a conexão direta falha. Chat e sinalização passam pelo servidor e ficam só na memória dele. O site oficial e o app publicado enviam ao Sentry relatórios de erro e estatísticas de uso anônimas: contagens e durações em faixas, nunca seu nome, o código da sala, seu IP ou o que você compartilha. Desligue as estatísticas em **Enviar estatísticas de uso anônimas**, no menu da saída de áudio (a seta ao lado do botão do alto-falante). Cópias hospedadas por terceiros e builds locais não enviam nada.

## Rodar localmente

Com o **JDK 21** instalado:

```bash
git clone https://github.com/RafaelrainBR/screenshare.git
cd screenshare
./gradlew :server-java:run
```

No PowerShell, use `.\gradlew.bat :server-java:run`.

Abra **[http://localhost:8080](http://localhost:8080)** em duas abas e entre na mesma sala. O comando compila o cliente e inicia o servidor; o Gradle Wrapper baixa as ferramentas e dependências necessárias. Fora de `localhost`, sirva a aplicação por HTTPS para usar a captura de tela.

## Documentação e contribuição

A documentação técnica está em inglês para facilitar a colaboração:

- [Índice da documentação](docs/README.md)
- [Uso, instalação e solução de problemas](docs/usage.md)
- [Desenvolvimento e testes](docs/development.md)
- [Hospedagem própria, Docker e STUN/TURN](docs/self-hosting.md)
- [Desenvolvimento e releases do app](desktop-native/README.md)
- [Como contribuir](CONTRIBUTING.md)

O projeto está em desenvolvimento ativo. A documentação acompanha o código atual; o app publicado pode estar atrás da `master`. Consulte as [releases](https://github.com/RafaelrainBR/screenshare/releases) e [issues](https://github.com/RafaelrainBR/screenshare/issues) para acompanhar mudanças e reportar problemas.

As capturas mostram o cliente web atual numa sala local de demonstração, com transmissão WebRTC real e uma conversa de exemplo. O código compartilhado pertence a este repositório.

Licença [MIT](LICENSE).
