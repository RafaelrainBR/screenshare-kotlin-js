# Teste da primeira experiência — cinco pessoas

**Status em 18/10/2026: protocolo preparado; 0/5 pessoas recrutadas, 0/5 sessões realizadas.** Não há resultados observados. As linhas abaixo são fichas vazias, não participantes ou depoimentos simulados.

Objetivo: descobrir se uma pessoa nova entende a proposta, escolhe uma versão compatível, entra numa sala e consegue compartilhar uma janela. Referências: [plano](../project-presence-plan.md), [guia de uso](../usage.md) e [v0.11.1 estável](https://github.com/RafaelrainBR/screenshare-kotlin-js/releases/tag/v0.11.1). Registrar separadamente o commit/deploy da página e a versão instalada; não chamar a interface local de release publicada.

## Preparação e recrutamento

O responsável pelo teste ainda precisa ser definido. Buscar cinco pessoas que não tenham participado do desenvolvimento nem conheçam o fluxo. Cobertura desejada: duas usando navegador, duas usando Windows x64 e uma usando macOS Apple Silicon. Registrar a plataforma real de cada uma; se não houver Mac, deixar sua cobertura pendente, mesmo que uma quinta pessoa teste outra plataforma. Não afirmar compatibilidade macOS validada por testes em Windows.

Os convites dependem de autorização explícita do usuário para os destinatários e a mensagem; nenhum foi enviado. Realizar as sessões após a página e o pacote de referência estarem disponíveis. Cada pessoa pode ter como par um facilitador que já esteja na sala, evitando que uma falha de outro participante invalide a tarefa.

Obter consentimento para observar a sessão e anotar tempos/dificuldades. Usar identificadores P1–P5 e dados de demonstração. Não registrar nome real, código real da sala, IP, SDP, ICE, conversas privadas ou conteúdo da tela. Não gravar áudio/vídeo por padrão. A pessoa pode interromper a sessão a qualquer momento. Não pedir que desative proteções do sistema para concluir a instalação; observar o aviso e o entendimento das orientações oficiais já disponíveis.

## Roteiro para a pessoa

Entregar só o link público e esta situação: **“Você vai revisar um trecho de código com outra pessoa. Use o Screenshare para entrar na sala dela e mostrar uma janela de demonstração.”** Não apontar botões nem explicar o código da sala antes da tarefa. Avisar que o produto está sendo avaliado e que dúvidas são úteis.

| Tarefa | Instrução neutra | Registro |
|---|---|---|
| 1. Entender | “O que você acha que consegue fazer aqui? Como começaria?” | Compreendeu a proposta? Identificou navegador/desktop? |
| 2. Escolher acesso | “Use a opção adequada para seu dispositivo.” | Caminho escolhido; pacote/plataforma certos; versão estável ou experimental; dúvidas de assinatura/permissão |
| 3. Instalar, quando aplicável | “Siga as instruções de instalação disponíveis na página.” | Tempo separado; pacote; avisos; permissão; resultado; ajuda |
| 4. Entrar | Entregar um código válido de uma sala já aberta: “Entre na sala desta outra pessoa.” | Início ao receber o código com app aberto e pronto; fim ao os dois serem visíveis na mesma sala; segundos; ajuda |
| 5. Compartilhar | “Mostre a janela de demonstração para a outra pessoa.” | Fonte correta; autorização; imagem recebida; tempo; bloqueio |
| 6. Conversar e alternar | “Converse com a pessoa e veja a transmissão dela.” | Microfone/chat; localizar foco/grade; parar a própria captura |
| 7. Encerrar | “Saia da sala e diga onde procuraria ajuda se algo falhasse.” | Saída e caminho de feedback encontrados; expectativa de privacidade |

Áudio da fonte é uma tarefa adicional só quando compatível com a plataforma/fonte e com conteúdo de demonstração. Registrar se o receptor ouviu, qual modo foi escolhido e se houve aviso. Não marcar falha de captura suportada como sucesso por o microfone transmitir o som do alto-falante.

Durante as tarefas, o facilitador observa sem orientar. Se a pessoa ficar bloqueada ou pedir ajuda, registrar o ponto e o tempo antes de intervir; qualquer instrução sobre o fluxo conta como ajuda. Permitir continuar com ajuda para observar as tarefas restantes. Não apagar o bloqueio da ficha após ajudar.

## Ficha de resultados pendentes

| ID | Dispositivo/versões | Escolha correta | Instalação: tempo/resultado | Entrada: segundos/sem ajuda | Captura recebida | Problemas/gravidade | Consentimento/data |
|---|---|---|---|---|---|---|---|
| P1 | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente |
| P2 | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente |
| P3 | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente |
| P4 | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente |
| P5 | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente | Pendente |

Preencher versões de SO, navegador/WebView, app e página. Para cada problema, registrar tarefa, ação tentada, resultado esperado/observado e ajuste proposto; usar citação apenas com fala realmente observada. Marcar `não aplicável` quando pertinente e `não testado` quando não foi possível observar. Nunca converter ausência de dado em sucesso.

Classificação simples: **bloqueante** impede entrar/compartilhar ou leva ao pacote errado; **relevante** exige ajuda ou induz decisão errada; **leve** causa dúvida sem impedir a tarefa. Priorizar bloqueantes antes da divulgação.

## Critérios e conclusão a preencher

- Pelo menos **4 de 5 pessoas** entram na sala em **até 120 segundos**, sem ajuda, depois de receberem o código com app pronto. O relógio de instalação fica separado.
- Ninguém é encaminhado à plataforma/versão errada.
- Downloads corretos e pelo menos uma captura real observada em cada plataforma declarada validada.
- Bloqueantes corrigidos e retestados com registro do resultado; cobertura ausente explicitamente anotada.

**Resultado atual:** pendente. **Tempo de entrada observado:** indisponível. **Taxa de sucesso:** indisponível, sem denominador observado. **Cobertura Windows/macOS/navegador:** pendente. **Decisão de divulgar:** pendente da validação e autorização.

Depois das sessões, resumir os achados e as correções na mesma ficha. Cinco pessoas dão sinais de usabilidade, não uma estimativa estatística de conversão ou confiabilidade. Não coletar analytics de sala para substituir observação direta.
