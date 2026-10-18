# Página pública, mídia e publicação

As páginas PT-BR e inglês ficam em `/about` e `/about/en.html`. A entrada da sala continua em `/`, incluindo o upgrade WebSocket; não há um segundo frontend para as salas.

## Fontes e geração

- `site/page.html`: estrutura comum; `site/content.json`: textos PT/EN.
- `site/latest.json`: snapshot do manifesto público da release estável, ou marcador explícito de fallback.
- `server/src/jvmMain/resources/presentation/`: HTML gerado, CSS e mídia servidos somente pelo servidor JVM. O bundle Tauri não incorpora esses arquivos.
- A fonte local Barlow, o logotipo e a imagem do produto são os assets já existentes. O documento de [referência visual](../presentation-brand-spec.md) registra as escolhas.

Gerar e validar com Node.js, sem instalar dependências no app:

```bash
node scripts/build-presentation.mjs
node --test scripts/build-presentation.test.mjs
node scripts/build-presentation.mjs --check
```

Antes de publicar uma nova release estável na apresentação:

```bash
node scripts/build-presentation.mjs --refresh
```

O refresh consulta o `latest.json` existente uma vez, valida versão/plataforma/repositório e gera links estáticos. Não há consulta ao GitHub por visitante. Se a consulta falhar ou os dados forem inválidos, grava um fallback reproduzível para a página oficial da última release. Revise a diferença dos arquivos e publique o site após publicar o app; não anuncie uma versão antes de os assets estarem disponíveis.

## Atualizar os materiais visuais

Use uma instalação existente de Playwright e Google Chrome, seguindo `PLAYWRIGHT_MODULE_DIR` como nos outros scripts de QA. Para vídeo, defina também `FFMPEG_PATH` para um FFmpeg instalado com libx264 e libass; isso é ferramenta de produção de mídia, não dependência distribuída do app.

```powershell
$env:PLAYWRIGHT_MODULE_DIR = 'C:/caminho/para/node_modules'
$env:SCREENSHARE_DOCS_URL = 'http://localhost:8080'
$env:FFMPEG_PATH = 'C:/caminho/para/ffmpeg.exe'
node scripts/capture-social.mjs
node scripts/capture-presentation.mjs
```

O vídeo registra uma sessão local real com dois clientes Chrome, compartilhamento de aba WebRTC e mensagens de demonstração. Tem MP4 PT/EN, legendas abertas/arquivos VTT e transcrições. A amostragem do navegador é limitada a 10 fps: o vídeo explica o fluxo, não mede desempenho da transmissão. Não grava microfone, câmera ou áudio de aplicativo e não comprova captura nativa.

As capas usam a marca e uma captura real. Inspecione as duas imagens 1280 × 640 e o começo/meio/fim dos dois vídeos antes de publicar. Metadados de gravação e evidências ficam em `artifacts/qa/presence/`, ignorado pelo Git. Os arquivos finais ficam nos recursos `presentation/assets/` do servidor.

## Validação local

Com o Ktor atual em execução:

```powershell
$env:PLAYWRIGHT_MODULE_DIR = 'C:/caminho/para/node_modules'
$env:SCREENSHARE_DOCS_URL = 'http://localhost:8080'
node scripts/qa-presentation.mjs
```

O check cobre PT/EN sem JavaScript, cinco larguras de tela, macOS, links, imagens/fontes, reprodução/seek do MP4 e a entrada original com dois peers e entrega de chat. Os relatórios distinguem testes automatizados de hardware nativo e participantes humanos, que não são simulados.

## Deploy no Fly.io

Com Fly CLI autenticado e acesso ao app `screen-share`, execute da raiz:

```bash
node scripts/deploy-web.mjs
```

O comando atualiza o snapshot, compila o JAR, usa o builder remoto do Fly a partir de `server-java/` e verifica a página publicada. Ele faz deploy de produção. O servidor mantém salas/chat em memória; uma reinicialização pode encerrar as sessões existentes. Faça esse passo depois dos checks e da revisão do resultado concreto.

Depois, confira `/`, `/about`, `/about/en.html`, os arquivos sociais e os vídeos. Configure a homepage do GitHub como `https://screen-share.fly.dev/about` somente quando a página estiver respondendo. Para a imagem social do repositório, envie `presentation/assets/social-en.png` em Settings → Social preview; o site usa as capas localizadas por metadados Open Graph.

## Observação após divulgação

A [linha de base](baseline-2026-10-18.md), os [textos por canal](outreach-drafts.md) e o [protocolo com cinco pessoas](usability-test.md) registram o que falta. A janela D+7/D+14 começa na divulgação real, não na geração dos arquivos. Este procedimento não cria nem agenda publicações.
