# Plano de presença do Screenshare

Data: 18/10/2026. Status: execução autorizada. Acompanhamento, evidências e pendências ficam em [docs/presence](presence/README.md); o protocolo humano e a divulgação não devem ser marcados como concluídos por testes automatizados.

## Objetivo

Fazer uma pessoa que ainda não conhece o projeto entender a proposta, escolher navegador ou desktop e concluir sua primeira sessão. A divulgação deve levar a uma experiência que já foi validada.

Público inicial: pessoas que querem compartilhar tela e conversar com amigos ou colaborar em código, e desenvolvedores interessados em Kotlin/JS, WebRTC e captura nativa.

Mensagem inicial: **Compartilhe a tela e converse na mesma sala, pelo navegador ou pelo desktop.** A captura nativa e o áudio por aplicativo entram como diferenciais demonstráveis, com as limitações de cada plataforma.

## Ponto de partida

- O [PR #28](https://github.com/RafaelrainBR/screenshare-kotlin-js/pull/28) já renovou README em inglês/português, screenshots, guias e contribuição. Descrição, site e tópicos do repositório também foram preenchidos.
- O site público abre diretamente o formulário de entrada. Seu HTML ainda tem apenas o título genérico, sem descrição e metadados sociais.
- Existem uma release estável do desktop e uma pré-release nativa experimental. O visitante precisa entender qual é a recomendada.
- A release gera `latest.json` com os links versionados dos instaladores. Esse manifesto já pode orientar os downloads, sem criar outro serviço.
- A identidade visual, os logotipos, as fontes locais e o procedimento de screenshots já existem e devem ser reutilizados.

## Sequência e entregas

| Etapa | Entrega | Dependência | Conclusão |
|---|---|---|---|
| 0 | Mensagem, versão de referência e medição inicial | Nenhuma | Material coerente com a versão que será oferecida |
| 1 | Demonstração curta e imagens de compartilhamento | Etapa 0 | Vídeo legível e capas revisadas |
| 2 | Página pública, downloads e metadados | Mensagem e imagens; usar pôster até o vídeo ficar pronto | Página testada e links corretos |
| 3 | Teste com novos usuários e ajustes | Página e versão instalável | Problemas críticos da primeira sessão corrigidos |
| 4 | Divulgação e revisão dos resultados | Etapa 3 | Publicações autorizadas e avaliação após 14 dias |

Estimativa inicial: 4–6 dias de trabalho para preparar as entregas e corrigir problemas pequenos, mais a janela de observação. Acesso a hardware, disponibilidade dos participantes e eventuais bugs podem aumentar esse prazo. Não é um calendário agendado.

## Etapa 0 — Preparar a mensagem e a referência

- Escolher uma versão estável para demonstração e downloads. Se algo existir apenas na `master` ou na variante experimental, identificá-lo explicitamente.
- Usar o mesmo nome e a mesma proposta na página, no README, na capa e nas publicações.
- Registrar uma linha de base: visitantes únicos do GitHub, origens de tráfego disponíveis, estrelas e downloads dos instaladores por plataforma/tag.
- Separar downloads de instalação dos acessos a `latest.json`, assinaturas e checksums. Os acessos do atualizador não são novas instalações.

**Aceite:** versão de referência registrada; links disponíveis; lista dos recursos que realmente poderão aparecer no vídeo; linha de base com data e período de coleta.

## Etapa 1 — Mostrar o produto funcionando

Produzir um vídeo de 30–45 segundos com esta sequência:

1. Mostrar a proposta em uma frase e entrar numa sala.
2. Mostrar o segundo participante entrando com o código.
3. Compartilhar uma janela com conteúdo de demonstração.
4. Demonstrar áudio da fonte, caso validado na plataforma gravada, e alternar o foco da transmissão.
5. Encerrar com os caminhos para experimentar e baixar.

Usar uma sessão real, nomes e conteúdo de demonstração, legenda curta e indicação da plataforma. Cortes podem encurtar a gravação; não devem sugerir desempenho que não foi observado. O vídeo precisa ser compreensível sem som, com uma legenda explicando a demonstração de áudio.

Entregar MP4 com pôster estático, uma imagem social para o GitHub e uma para o site. Reutilizar a marca existente; a capa deve conter uma captura real, uma frase curta e texto legível no celular. Para o GitHub, preparar 1280 × 640, abaixo de 1 MB.

Hospedar vídeo e mídia pesada fora dos assets incorporados ao aplicativo Tauri. Reutilizar o mesmo vídeo no site, no README por link/pôster e nas publicações.

**Aceite:** sem dados privados; recursos compatíveis com a versão anunciada; texto legível; licença dos materiais conferida; vídeo e pôster abrem em desktop e celular.

## Etapa 2 — Criar uma entrada pública clara

Publicar uma página leve em uma rota própria, como `/about`, usando a infraestrutura atual. O botão **Experimentar no navegador** leva à aplicação existente em `/`. A rota de sinalização WebSocket, as salas e os URLs usados pelo desktop continuam funcionando.

Conteúdo, nesta ordem:

1. Proposta em uma frase, screenshot/pôster e botão de experimentar.
2. Demonstração e três benefícios concretos.
3. Comparação curta entre navegador, Windows e macOS.
4. Acesso ao download recomendado e às instruções da plataforma.
5. Limitações relevantes, código-fonte, documentação e feedback.

Preparar versões PT-BR e inglês com conteúdo equivalente. Indicar que a interface do app está em português. Os botões das duas plataformas permanecem visíveis, mesmo se houver sugestão baseada no sistema do visitante.

### Downloads e confiança

- Priorizar o instalador estável Windows x64 e o caminho documentado de instalação para macOS Apple Silicon.
- Usar o `latest.json` existente como fonte dos links na geração/publicação da página. Gerar links estáticos e registrar a versão; evitar uma consulta ao GitHub a cada visitante.
- Incluir a atualização desses links no procedimento de publicação de cada release estável. Se a resolução falhar, oferecer a página oficial da última release em vez de um botão quebrado.
- Tratar a variante nativa experimental como alternativa identificada, fora do botão principal recomendado.
- Explicar assinatura Windows, notarização macOS e permissões antes da instalação. No macOS, manter visível o guia do script, evitando prometer que o arquivo baixado abrirá sem restrições.
- Começar as próximas notas de release com benefícios para quem usa, instalação e problemas conhecidos. A lista técnica fica depois.

### Compartilhamento e busca

- Adicionar título descritivo, descrição, URL canônica, Open Graph e metadados de cartão social à página pública.
- Usar imagem pública com URL absoluta, conteúdo indexável em HTML, idioma declarado e relação entre as versões PT-BR/inglês.
- Aplicar os metadados adequados também à entrada do app, sem expor nome ou código de sala nas prévias.
- Configurar a imagem social do repositório e atualizar o site do GitHub para a página pública depois de ela estar disponível.

**Aceite:** navegação por teclado, foco visível, contraste e layout verificados em celular/desktop; mídia sem autoplay obrigatório; conteúdo principal disponível sem JavaScript; links de download conferidos; testes de entrar/criar sala, WebSocket, reconexão e desktop sem regressão. As prévias devem ser inspecionadas em serviços que consumam Open Graph, considerando possíveis caches.

### Onde a implementação deve chegar

| Área | Alcance previsto |
|---|---|
| Recursos estáticos e roteamento Ktor | Página, estilos, imagens, metadados e rota pública; preservar `/` e o upgrade WebSocket |
| Publicação/release | Ler o manifesto existente, validar links e manter a versão apresentada consistente |
| README e documentação | Apontar para a demonstração/página e explicar downloads; manter inglês e português alinhados |
| Configurações do GitHub | Imagem social e homepage |
| QA | Primeira sessão, navegação acessível, links, prévias e compatibilidade dos clientes existentes |

## Etapa 3 — Validar a primeira utilização

Convidar cinco pessoas que ainda não conheçam o fluxo, buscando cobrir navegador, Windows e macOS. Elas devem tentar sem instrução ao vivo: entender a proposta, escolher como acessar, entrar na mesma sala de outra pessoa e compartilhar tela.

Registrar tempo aproximado, pontos de dúvida e resultado, com consentimento e sem gravar conteúdo privado. Não confundir teste local de UI com captura nativa validada em hardware real.

Meta de usabilidade para esse teste pequeno: pelo menos quatro das cinco pessoas entram na sala em até dois minutos após receber o código, sem ajuda. A instalação desktop é avaliada separadamente; seu tempo não entra nessa meta. Isso é um critério de teste, não uma estimativa estatística de conversão.

**Aceite:** nenhuma pessoa direcionada à versão/plataforma errada; links funcionam; bloqueios críticos corrigidos; limitações por dispositivo documentadas. Se não houver Mac disponível, registrar essa lacuna e não tratar a validação macOS como concluída.

## Etapa 4 — Divulgar e aprender

Preparar três materiais reutilizando os mesmos assets:

- Publicação curta PT-BR: caso de uso, vídeo, link para experimentar e pedido específico de feedback.
- Versão em inglês para comunidades pertinentes ao público do app.
- Artigo técnico explicando Kotlin/JS + WebRTC + captura nativa, com decisões e limitações observadas.

Escolher inicialmente dois canais com público relevante. Conferir regras de autopromoção e formato antes de publicar. Usar links identificados por campanha quando houver medição capaz de interpretá-los; só acrescentar parâmetros não produz métricas automaticamente.

As peças podem ser preparadas durante o trabalho. Enviar convites ou publicar em contas/comunidades depende de autorização explícita do usuário para os canais e textos. Não automatizar divulgação recorrente.

Oferecer um caminho simples de feedback, com formulário de bug e algumas issues pequenas e bem descritas para contribuição. Começar pelas Issues existentes; abrir outro canal de comunidade quando houver demanda que o justifique.

**Aceite:** textos e destinos revisados; links conferidos; publicações autorizadas; responsável definido para acompanhar problemas após cada divulgação.

## Medir sem criar um projeto de analytics

| Pergunta | Medida inicial | Limite |
|---|---|---|
| Mais pessoas descobriram o repositório? | Visitantes únicos e origens disponíveis no GitHub | Não cobre toda a audiência do site |
| Há interesse no desktop? | Variação dos downloads dos instaladores por plataforma/tag | Download não equivale a instalação ou usuário único |
| A primeira experiência funciona? | Resultado dos cinco testes e problemas reportados | Amostra pequena; requer observação direta |
| O material gera interesse em contribuir? | Issues úteis, contribuições e dúvidas recebidas | Estrelas são um sinal secundário |

Coletar a linha de base antes de divulgar e comparar períodos equivalentes após 7 e 14 dias. Anotar o horário de cada publicação, a versão oferecida e o dia da coleta. O tráfego do GitHub tem janela de 14 dias; salvar os resultados antes de perder o histórico.

Sem analytics adicional, não afirmar taxa de conversão do site ou de conclusão de sessões. Se os dados iniciais forem insuficientes, propor depois uma medição agregada específica. Nenhum dado de chat, sala, captura, SDP ou ICE é necessário para este plano.

## Divisão sugerida de execução

1. **Material de demonstração:** mensagem, roteiro, gravação, pôster e capas.
2. **Página pública e downloads:** implementação, metadados, fluxo de publicação e testes.
3. **Apresentação e feedback:** integração no README, configuração social do GitHub, notas de release e formulário de bug.
4. **Validação e divulgação:** teste com pessoas novas, correções, revisão das publicações e comparação dos resultados.

Essas são entregas revisáveis, não tarefas ou automações já criadas. Publicação e deploy devem ocorrer depois de os respectivos materiais estarem concretos e validados.

## Fora desta primeira rodada

Anúncios pagos, novo framework de frontend, redesign do app, mudança de nome/domínio, contas de usuário, analytics por pessoa, fórum adicional e calendário automático de publicações. Assinatura/notarização completas podem reduzir atrito, mas exigem uma avaliação própria de custo, contas e manutenção.

## Referências

- [Imagem social de um repositório](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/customizing-your-repositorys-social-media-preview)
- [Links para releases e assets](https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases)
- [Tráfego do repositório e janela de 14 dias](https://docs.github.com/en/repositories/viewing-activity-and-data-for-your-repository/viewing-traffic-to-a-repository)
- [Workflow de release atual](../.github/workflows/release.yml)
- [Uso e limitações atuais](usage.md)
