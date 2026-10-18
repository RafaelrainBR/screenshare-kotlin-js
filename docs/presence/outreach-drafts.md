# Divulgação — canais e rascunhos

Pesquisa em **18/10/2026**. **Nada publicado ou enviado.** Os textos abaixo são propostas para revisão pelo autor. A publicação depende de autorização explícita para o canal e o texto final, da mídia real revisada e da validação da primeira experiência. Não usar resultados de testes ainda pendentes como argumento de divulgação.

## Dois canais iniciais

| Canal | Público e formato proposto | Regra confirmada e aplicação |
|---|---|---|
| LinkedIn, publicação do próprio autor | Desenvolvedores e pessoas que colaboram à distância; relato curto com vídeo legendado e uma pergunta específica | As [políticas oficiais](https://www.linkedin.com/legal/professional-community-policies) exigem conteúdo autêntico/profissional, proíbem spam e engajamento combinado. Publicar um relato do próprio projeto; não enviar convites promocionais em massa nem combinar curtidas. |
| [Kotlin Discussions — JavaScript](https://discuss.kotlinlang.org/c/javascript/13) | Desenvolvedores Kotlin/JS; explicar uma decisão técnica e pedir feedback sobre ela | A categoria é destinada a Kotlin/JS; as [diretrizes](https://discuss.kotlinlang.org/faq) pedem relevância, categoria correta, direitos sobre o conteúdo e vedam spam/cross-post repetido. Um tópico técnico nessa categoria é a recomendação editorial, **não autorização prévia da moderação**. |

Não encontrei nas diretrizes consultadas uma regra específica que garanta aceitação de autopromoção ou uma categoria dedicada a showcases. Reconfirmar as regras e os tópicos semelhantes no dia da publicação. Não postar a mesma peça em JavaScript, Web Development e Multiplatform. Escolher uma categoria; aqui, JavaScript pela discussão da ponte para as APIs do navegador.

A pesquisa usa somente as páginas dos próprios canais. Não há promessa de alcance, frequência ideal ou benefício de algoritmo. Para LinkedIn, escolher PT ou EN conforme a rede do autor; são alternativas, não dois posts duplicados. No fórum, preferir EN para discussão com a comunidade internacional; o PT fica como versão de revisão/tradução.

## LinkedIn — português

> Revisar código com alguém costuma exigir uma tela compartilhada e uma conversa ao mesmo tempo.
>
> Estou desenvolvendo o Screenshare para isso: entrar numa sala por código, compartilhar uma janela e conversar por voz, câmera ou chat, sem criar conta.
>
> O cliente usa Kotlin/JS e WebRTC. Ele roda no navegador e também no desktop com Tauri e captura nativa em Rust no Windows e macOS. A disponibilidade de áudio depende da plataforma e da fonte escolhida; a interface está em português brasileiro.
>
> Você consegue entender como entrar na mesma sala de outra pessoa só olhando a página? Quero especialmente feedback sobre essa primeira experiência.
>
> Conhecer e experimentar: https://screen-share.fly.dev/about
> Código e instalação: https://github.com/RafaelrainBR/screenshare-kotlin-js

Vídeos reais PT/EN de aproximadamente 31 segundos e a página pública foram publicados e validados tecnicamente em 18/10/2026. Anexar o arquivo correspondente ao idioma escolhido. O vídeo mostra uma sessão web local, sem áudio gravado; não apresentá-lo como evidência de captura nativa. O teste com cinco pessoas novas permanece pendente.

## LinkedIn — English

> Reviewing code with someone often means sharing a window while talking through it.
>
> I'm building Screenshare for that: join a room with a code, share a window, and talk using voice, camera, or chat. No account is required.
>
> The client uses Kotlin/JS and WebRTC. It runs in a browser and on desktop through Tauri, with Rust native capture on Windows and macOS. Audio support depends on the platform and source. The app interface is currently in Brazilian Portuguese.
>
> Can you work out how to join someone else's room from the page alone? I'd especially appreciate feedback on that first experience.
>
> Overview and demo: https://screen-share.fly.dev/about/en.html
> Source and installation: https://github.com/RafaelrainBR/screenshare-kotlin-js

## Kotlin Discussions — português para revisão

**Título:** Screenshare: Kotlin/JS e WebRTC com captura desktop convertida em MediaStreamTrack

> Estou desenvolvendo o Screenshare, um projeto aberto para compartilhar tela, conversar e entrar em salas por código.
>
> Uma decisão que gostaria de discutir é manter a conexão WebRTC em Kotlin/JS e adaptar a captura nativa à mesma interface de `MediaStreamTrack`. O servidor Ktor encaminha SDP/ICE, chat e estado da sala; o desktop Tauri fornece vídeo/áudio capturados em Rust. Assim, o código que adiciona tracks aos peers é compartilhado com o navegador.
>
> Os caminhos de captura ainda precisam respeitar APIs diferentes de WebView2 e WebKit. No navegador, áudio continua dependendo do seletor e da fonte; no desktop, a distribuição tem limitações de assinatura e permissões. Não estou apresentando números de desempenho sem uma medição reproduzível.
>
> Para quem integrou APIs recentes do navegador ao Kotlin/JS: como vocês delimitam as declarações externas e a ponte TypeScript para manter a compatibilidade entre WebViews? O projeto usa um decorator de RTCPeerConnection e uma ponte desktop para isso.
>
> Código: https://github.com/RafaelrainBR/screenshare-kotlin-js
> Página e demonstração: https://screen-share.fly.dev/about (interface do aplicativo em português brasileiro)
>
> Sou o autor do projeto e agradeço feedback técnico sobre essa fronteira.

## Kotlin Discussions — English, versão proposta para envio

**Title:** Screenshare: Kotlin/JS and WebRTC with native capture adapted to MediaStreamTrack

> I'm building Screenshare, an open-source project for screen sharing, conversation, and joining rooms by code.
>
> One decision I'd like to discuss is keeping the WebRTC connection code in Kotlin/JS while adapting native capture to the same `MediaStreamTrack` interface. A Ktor server forwards SDP/ICE, chat, and room state; a Tauri desktop shell supplies video and audio captured in Rust. The code that adds tracks to peers is shared with the browser client.
>
> Capture still has to respect the different APIs available in WebView2 and WebKit. Browser audio depends on the picker and source; desktop distribution has signing and permission limitations. I don't have reproducible performance results to claim here.
>
> For others integrating newer browser APIs with Kotlin/JS: how do you draw the boundary between external declarations and a TypeScript bridge to keep WebView compatibility manageable? This project uses an RTCPeerConnection decorator and a desktop bridge.
>
> Source: https://github.com/RafaelrainBR/screenshare-kotlin-js
> Overview and demo: https://screen-share.fly.dev/about/en.html (the app interface is currently in Brazilian Portuguese)
>
> I'm the project's author and would appreciate technical feedback on that boundary.

Fontes das afirmações técnicas: [artigo com links para o código](technical-article.md). No fórum, incorporar apenas o trecho que sustenta a pergunta; o artigo pode ser oferecido como leitura complementar após revisão, sem criar um post só de links.

## Registro de publicação e acompanhamento

| Campo | LinkedIn | Kotlin Discussions |
|---|---|---|
| Texto escolhido e aprovado | Pendente | Pendente |
| Autorização explícita de envio | Pendente | Pendente |
| Página/mídia: validação técnica | Concluída em 18/10/2026 | Concluída em 18/10/2026 |
| Teste com cinco pessoas novas | Pendente | Pendente |
| Responsável por responder | Pendente | Pendente |
| Data/hora e URL da publicação | Não publicado | Não publicado |
| D+7: coleta e dúvidas úteis | Pendente de publicação | Pendente de publicação |
| D+14: coleta e decisões | Pendente de publicação | Pendente de publicação |

Responder a dúvidas do uso e da arquitetura no contexto em que aparecerem, sem mensagens privadas não solicitadas. Relatos de bug devem usar as [Issues do repositório](https://github.com/RafaelrainBR/screenshare-kotlin-js/issues) e as orientações de [contribuição](../../CONTRIBUTING.md). Não inserir parâmetros de campanha até existir uma medição capaz de interpretá-los.
