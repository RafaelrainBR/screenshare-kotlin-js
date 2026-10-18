# Propostas de contribuições pequenas

Preparadas em **18/10/2026**. **Rascunhos de issues; nenhuma issue foi aberta.** Antes de publicar, verificar as [Issues existentes](https://github.com/RafaelrainBR/screenshare-kotlin-js/issues) para evitar duplicação, confirmar o reproducer no checkout e definir um responsável por revisar a contribuição. Os títulos e os critérios abaixo podem ser copiados para uma issue após revisão.

## 1. Tornar confiável o resultado do reporter de `:common:jsNodeTest`

**Problema observado nesta rodada:** o task `:common:jsNodeTest` imprime `TypeError: Class constructor Base cannot be invoked without new`, com `mocha-kotlin-reporter` / `TeamcityForWeb` na stack, mas o Gradle termina com `BUILD SUCCESSFUL`. O reporter e a integração de execução precisam ser investigados; esse sintoma não comprova falha nos testes do protocolo.

**Evidência fornecida pela execução desta rodada:** usando o reporter `spec` diretamente sobre o mesmo arquivo de testes compilado, foram observados **30 passing**. Os testes JVM do common tiveram **29** e os testes browser do cliente **24** aprovados. São suítes diferentes; não se espera que suas contagens sejam iguais. Esses resultados são o registro desta execução, não uma promessa de contagem futura nem um teste de captura em hardware.

**Reproducer**, na raiz do repositório, com JDK 21 e pré-requisitos do [guia de desenvolvimento](../development.md):

```powershell
.\gradlew.bat :common:jsNodeTest --rerun-tasks
```

Observar a stack e o resultado final separadamente. Depois da compilação, com `node` no PATH:

```powershell
node build/js/node_modules/mocha/bin/mocha.js --reporter spec common/build/compileSync/js/test/testDevelopmentExecutable/kotlin/screenshare-common-test.js
```

**Escopo proposto:** identificar a incompatibilidade na integração do reporter, corrigir o menor ponto necessário e preservar diagnóstico e sinalização de falhas. A configuração existente fixa Mocha **12.0.3** em [libs.versions.toml](../../libs.versions.toml) e a aplica em [build.gradle.kts](../../build.gradle.kts); a suíte é configurada em [common/build.gradle.kts](../../common/build.gradle.kts). Não adicionar uma dependência ou fazer downgrade apenas para ocultar a stack. Este documento não altera dependências e não prescreve uma correção antes do diagnóstico.

**Aceite:**

- `:common:jsNodeTest --rerun-tasks` executa os testes esperados e termina sem a exceção do reporter.
- A suíte direta com `--reporter spec` continua passando, sem exclusão de testes para satisfazer a integração.
- Uma asserção temporária deliberadamente falsa, em branch de teste, produz status de saída diferente de zero no Gradle e resultado de teste falho visível. Remover essa alteração deliberada antes do commit.
- Registrar Node, Kotlin, Mocha, sistema operacional e comandos usados; não tratar apenas `BUILD SUCCESSFUL` como evidência de execução integral.
- Reexecutar `:common:jvmTest` e os testes browser do cliente se a correção alcançar configuração compartilhada.

**Tamanho/risco:** investigação delimitada ao runner/reporter. Não rotular automaticamente como `good first issue`: pode exigir entendimento da integração Kotlin/JS–Mocha. Não usar o comando direto como substituto silencioso permanente de uma CI quebrada.

## 2. Documentar uma matriz real de captura e áudio dos navegadores

**Problema:** o [guia de uso](../usage.md) explica que captura e áudio variam conforme navegador, sistema e fonte, mas não apresenta resultados observados por combinação. Uma tabela baseada em suposições criaria promessas que o app não pode sustentar.

**Escopo proposto:** adicionar ao guia uma matriz pequena com resultados de sessões reais de dois peers; começar pelas máquinas disponíveis e marcar as outras combinações como **não testadas**. Não mudar o comportamento de captura nem confundir execução em headless com teste das permissões e áudio do dispositivo.

**Roteiro reproduzível:** abrir a mesma versão do app nos dois peers, entrar na mesma sala, compartilhar uma fonte com imagem e áudio de demonstração, e confirmar separadamente a imagem e o áudio recebidos. Registrar o que o seletor oferece para aba, janela e tela; uma opção ausente não deve virar sucesso inferido por outra fonte. Testar parar a captura e negar/cancelar o seletor sem publicar conteúdo. Repetir apenas nas combinações realmente disponíveis.

**Ficha a preencher para cada observação:**

| Data e versão/deploy do app | SO/versão real | Navegador/versão real | Fonte (aba/janela/tela) | Áudio oferecido pelo seletor | Imagem recebida | Áudio recebido | Permissões/limites |
|---|---|---|---|---|---|---|---|
| Pendente | Pendente | Pendente | Pendente | Não testado | Não testado | Não testado | Pendente |

**Aceite:**

- Ao menos uma combinação observada tem versão exata do SO e navegador, fonte usada, versão/deploy do app e resultado no receptor, incluindo teste de áudio quando o seletor o oferece.
- Campos sem observação estão marcados como `não testado` ou `indisponível no seletor`; não preencher versões ou resultados por conhecimento geral.
- Limitações são ligadas à combinação testada e à data; ausência de resultado não é tratada como incompatibilidade de toda uma plataforma.
- O texto não promete áudio de qualquer aplicativo por haver áudio de aba, nem confunde microfone captando alto-falantes com áudio da fonte.
- Capturas e registros não contêm códigos de sala, conversas, IPs ou conteúdo privado; seguir [CONTRIBUTING.md](../../CONTRIBUTING.md).

**Tamanho/risco:** contribuição documental apropriada para uma primeira participação com acesso ao hardware. Resultados macOS, móveis ou de outro navegador permanecem pendentes até serem observados. Este trabalho complementa o [teste com cinco pessoas](usability-test.md), sem substituir sua avaliação de usabilidade.
