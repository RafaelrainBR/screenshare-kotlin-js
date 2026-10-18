# Registro de execução da presença

Data: 18/10/2026. Referência de downloads: `v0.11.1`, linha estável Tauri. A variante nativa experimental permanece separada.

| Etapa | Resultado |
|---|---|
| Mensagem e linha de base | Concluídas; [snapshot e limites](baseline-2026-10-18.md) |
| Demonstração e capas | Gravadas/renderizadas; sessão real de aproximadamente 31 segundos, MP4 PT/EN, legendas e transcrições |
| Página e downloads | Implementados em PT-BR/inglês; validação final e deploy em andamento |
| Primeira utilização com pessoas | 0/5; [protocolo e fichas](usability-test.md) prontos, sem participantes simulados |
| Divulgação externa | [Textos PT/EN](outreach-drafts.md) e artigos [PT](technical-article.md)/[EN](technical-article.en.md) preparados; nada enviado |

## O que foi implementado

- Rotas `/about` e `/about/en.html`, mantendo a sala e o WebSocket em `/`.
- HTML indexável e utilizável sem JavaScript; fonte, identidade e screenshots existentes.
- Título, descrição, URL canônica, idiomas alternativos e cartões Open Graph/Twitter.
- Downloads estáticos derivados do manifesto da release, com validação de versão, plataforma e origem; fallback para a página oficial da última release.
- MP4 com legendas abertas PT/EN, arquivos VTT, transcrições HTML, pôster e capas 1280 × 640. Vídeo/amostras ficam apenas no servidor JVM, fora do pacote Tauri.
- Suporte HTTP parcial na rota da apresentação para permitir seek no vídeo, e HEAD para os arquivos públicos dessa rota.
- Formulário de bug, notas de release orientadas ao usuário e procedimento de atualizar/publicar a página após releases estáveis.
- [Backlog de contribuições](contribution-backlog.md) fundamentado em observações reais, ainda como propostas de issues.

## Origem e limites do vídeo

O cliente web foi gravado no Chrome com duas sessões locais, chat e captura real de uma aba via WebRTC. HTML, CSS e controles pertinentes foram comparados com `v0.11.1`. Uma faixa externa ao app contém as legendas; não foi desenhada uma interface substituta.

O gravador amostra o viewport em até 10 fps para evitar o corte de viewport observado no gravador embutido do Chrome. Os intervalos de captura preservam o tempo observado. Isso não demonstra taxa de quadros da transmissão ou desempenho de captura nativa. Não há áudio gravado, câmera física ou teste de WASAPI/Core Audio neste vídeo.

## Validação e publicação

Validação no checkout isolado:

- Build do servidor/JAR e `ktlintCheck`: concluídos. A dependência adicionada é o plugin oficial Ktor PartialContent, restrito à rota de apresentação.
- Cliente: 24 testes de navegador; common/JVM: 29 testes, sem falhas.
- O reporter Kotlin/JS–Mocha apresentou a incompatibilidade descrita no [backlog](contribution-backlog.md), apesar do exit code de sucesso do Gradle. Os testes compilados foram executados diretamente com o reporter `spec`: 30 passaram. Esse desvio não foi tratado como sucesso do reporter original.
- Gerador: quatro testes de versão/origem, fallback e escape de HTML, mais a conferência dos arquivos gerados.
- Browser QA: PT/EN sem JavaScript; larguras 320, 390, 768, 1280 e 1440 px; downloads e guia macOS; HEAD das capas e HTTP 206 do vídeo; reprodução e seek; rota original com dois peers e entrega de chat.
- As alterações de WebRTC e textos de ajuda observados em paralelo no checkout principal foram excluídos desta entrega.

PR e deploy serão registrados após a publicação. A validação acima não equivale a testes com cinco pessoas ou em hardware nativo.

## Pendências que exigem participação externa

- O usuário escolheu aplicar manualmente a capa social do GitHub. Arquivo: [`social-en.png`](../../server/src/jvmMain/resources/presentation/assets/social-en.png); alternativa PT: [`social-pt.png`](../../server/src/jvmMain/resources/presentation/assets/social-pt.png). Em Settings → Social preview, selecionar Edit → Upload an image.
- Recrutar e observar cinco pessoas, incluindo cobertura real de Windows/macOS. Nenhuma sessão humana foi realizada.
- Escolher os textos/canais e autorizar a publicação ou o envio dos convites. Os rascunhos não foram postados.
- Iniciar a comparação D+7/D+14 quando ocorrer a divulgação real. Nenhuma automação de postagem foi criada.
- Validar caches/prévias em serviços sociais externos após a publicação, sem tratar somente a inspeção de metadados como teste de todos os serviços.
