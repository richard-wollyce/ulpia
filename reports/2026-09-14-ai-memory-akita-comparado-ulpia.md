# O que aproveitar do ai-memory na Ulpia

Análise para Richard, 14/09/2026. Repositório externo: [akitaonrails/ai-memory](https://github.com/akitaonrails/ai-memory), versão 2.2.1, commit [`74d2d31`](https://github.com/akitaonrails/ai-memory/tree/74d2d31ebd8cca656c49f31563fac53e0b61c5cf), de 12/09/2026. Ulpia: código do workspace, HEAD `9cdf2bf66114223b61fb42253f56f959cf701e73`, com alterações locais preexistentes.

Leitura de documentação, implementação e testes existentes. Não executei o ai-memory nem comparei desempenho dos sistemas. Reproduzi um caso de perda parcial da Ulpia usando o binário instalado em uma base temporária. As propostas abaixo são recomendações; nenhuma mudança funcional foi feita.

## Conclusão

**Sim. O principal aproveitamento é completar e tornar recuperável o ciclo entre sessões.** A Ulpia já possui vários fundamentos que aparecem no ai-memory: Markdown, índice derivado, captura sem modelo, memória curta identificada, promoção revisada, limites de contexto e isolamento do agente ativo por sessão. Não há motivo demonstrado nesta leitura para substituir nossa recuperação determinística.

O avanço mais útil seria registrar o estado real da tarefa, recuperar falhas de captura e aproveitar evidências de várias sessões. O ai-memory fornece mecanismos concretos para isso, mas também possui diferenças entre o que o README sugere, o comportamento padrão e as garantias efetivamente verificadas pelo código.

## Os dois fluxos observados

**ai-memory:** eventos do harness → transporte e observações → página de sessão sem LLM → consolidação e revisão opcionais com LLM → wiki → busca, briefing e handoff. Há também uma revisão opcional de várias sessões juntas. A [arquitetura](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/docs/ARCHITECTURE.md) descreve o fluxo e suas diferentes configurações.

**Ulpia:** `kb boot` consulta a memória e escolhe o agente → registra recusas e roteamentos em `.kb/sessions/` → `SessionEnd` chama `kb capture` → depósitos por agente em `inbox/` → `kb promote` propõe e consulta três lentes de revisão → notas aceitas entram na biblioteca. O boot seguinte consulta a biblioteca e injeta constituição e evidências conforme o contexto.

O detalhe decisivo: **nosso depósito automático não contém o trabalho inteiro da sessão**. Ele registra recusas e roteamentos. Prosa do usuário, respostas do agente e propostas de `remember` ainda não integram essa captura. Isso está explícito em [capture.rs:13](C:/Users/richa/Desktop/ulpia/tools/kb/src/capture.rs:13), inclusive suas exclusões nas linhas 27–30. A implementação atual divide o depósito entre os agentes que participaram; o trecho antigo do ADR-0035 que fala apenas no último agente não representa mais esse código.

## Aproveitamentos, em ordem de execução

### 1. Recuperação de captura antes de ampliar a captura

O ai-memory tem fila local, identificador estável de ingestão, tentativas posteriores, confirmação individual em lotes e mecanismos para retomar efeitos incompletos. O princípio útil é: **um item só está entregue depois que seu destino confirmou a gravação, e repetir a entrega precisa ser seguro**.

Encontrei uma falha correspondente na Ulpia. Em [capture.rs:254](C:/Users/richa/Desktop/ulpia/tools/kb/src/capture.rs:254), o processamento continua se um dos agentes não tiver base ou a escrita falhar. Porém, [capture.rs:275](C:/Users/richa/Desktop/ulpia/tools/kb/src/capture.rs:275) apaga o registro inteiro se qualquer depósito tiver sido escrito.

Reprodução isolada:

1. Registro com dois donos: `zed` e `missing-agent`, cada um com uma pergunta recusada.
2. Somente a base de `zed` existe.
3. `capture` salva o depósito de `zed`, informa que a outra base não existe e remove o arquivo `.events`.
4. A segunda chamada informa que não há nada para capturar. A pergunta do segundo dono perdeu sua fonte de recuperação nesse fluxo.

O binário usado reporta `kb 0.4.0 (unknown, x86_64 windows)`: o SHA da compilação não é identificável. O comportamento reproduzido coincide com a condição presente no código atual. Não equivale a uma execução de uma compilação recém-feita desse HEAD.

**Adaptação recomendada:** confirmação por destino, preservação dos itens incompletos e escrita atômica do depósito. Recuperar pendências numa próxima entrada ou encerramento de sessão. Isso pode continuar baseado em arquivos; a fila SQLite e o servidor do Akita não são pré-requisitos. Um checkpoint deve preservar a sessão ativa, enquanto o encerramento pode consumi-la; reutilizar uma operação que apaga tudo para os dois casos criaria outra perda.

Ressalva sobre a referência: a fila externa tem limites de idade, quantidade e tentativas. Além disso, o endpoint unitário responde antes de terminar o processamento; o caminho de lote oferece uma confirmação mais forte. Portanto, copiar o contrato de confirmação, sem prometer entrega infalível ou execução exatamente uma vez. Ver [hook_spool.rs](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-cli/src/commands/hook_spool.rs) e [router.rs:756](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-hooks/src/router.rs#L756). A distinção entre checkpoint e encerramento também está [implementada no router](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-hooks/src/router.rs#L2760).

### 2. Automatizar a passagem do estado da tarefa

O ai-memory representa handoff como um objeto: origem, projeto, sessão, resumo, perguntas abertas, próximos passos, arquivos e estado de aceitação. Isso permite distinguir conhecimento compartilhado de uma tarefa que outra sessão está assumindo. Os campos estão em [handoff.rs:60](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-core/src/handoff.rs#L60).

A Ulpia já tem orientação manual para registrar o que foi feito, decidido e deixado aberto. A lacuna é a geração e a retomada automáticas desse registro: [boot.rs:220](C:/Users/richa/Desktop/ulpia/tools/kb/src/boot.rs:220) trabalha a pergunta atual e o agente escolhido; `capture` produz outro tipo de evidência.

**Adaptação recomendada:** um registro curto de continuidade, vinculado a projeto, checkout e tarefa, com objetivo, decisões, tentativas que falharam, pendências e referências verificáveis. A próxima sessão recupera esse registro além de consultar conhecimento. Aceitação deve se referir à responsabilidade pela tarefa; não deve impedir outras sessões de ler conhecimento compartilhado.

O orçamento de contexto não é uma novidade a importar: já existe `HIT_BUDGET = 6000`, com indicação do material omitido, em [boot.rs:583](C:/Users/richa/Desktop/ulpia/tools/kb/src/boot.rs:583). Aproveitaríamos essa disciplina para o registro de continuidade.

Limite do Akita: o handoff automático por heurística é simples, baseado em prompts e ferramentas observados; a existência de campos ricos não significa que todos sejam automaticamente preenchidos com uma síntese completa do trabalho.

### 3. Capturar evidência útil com autoria e filtragem

Registrar uma pergunta sem resposta ensina onde a busca falhou. Para lembrar uma decisão, preferência ou solução, precisamos capturar também a evidência que a estabelece. Ampliar a entrada da memória é, portanto, uma mudança de conteúdo e autoria, além de uma mudança de integração com o harness.

O ai-memory aplica uma fronteira tipada de sanitização antes do armazenamento de observações: [sanitize.rs](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-core/src/sanitize.rs). A adaptação útil é exigir um tipo de entrada já filtrada para qualquer persistência ou envio a promotores, incluindo logs e arquivos de repetição de entrega.

**Adaptação recomendada:** começar com decisões explícitas, correções, resultado de tentativas e pendências; preservar quem disse, sessão, referência e natureza da evidência. Manter a separação entre afirmação do usuário, observação de ferramenta e interpretação do agente. Uma síntese gerada não deve adquirir autoria humana ao ser promovida.

Isso complementa nossa separação por pastas e proveniência. O `inbox/` não pertence ao conjunto privado padrão de [base.rs:54](C:/Users/richa/Desktop/ulpia/tools/kb/src/base.rs:54), logo ampliar a captura exige escolher também onde esse conteúdo poderá ser servido.

Ressalva importante: no ai-memory, o spool comum pode conter o evento JSON bruto. A fronteira tipada no store não comprova que nenhum conteúdo bruto foi gravado antes. Copiar a filtragem exige aplicá-la também antes da fila local; nenhum filtro por padrões garante encontrar todo segredo.

### 4. Consolidar padrões entre sessões

O [`experience` pass](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/docs/experience.md) lê resumos de várias sessões lado a lado e propõe procedimentos, preferências e padrões recorrentes. É opcional. O exemplo usa uma rodada após cinco novas sessões, consultando dez resumos.

Nosso [promote.rs:927](C:/Users/richa/Desktop/ulpia/tools/kb/src/promote.rs:927) propõe a partir de um arquivo de depósito por vez. Reler a biblioteca após uma escrita ajuda a revisão a enxergar notas recém-promovidas, mas não equivale a comparar várias experiências como entrada do primeiro promotor.

**Adaptação recomendada:** uma fase que produza candidatos a partir de registros de várias sessões e entregue esses candidatos ao nosso processo de revisão. Para padrões inferidos, exigir referências reais a pelo menos duas sessões diferentes, verificadas em código. Uma instrução explícita do usuário não precisa esperar repetição para ser guardada.

Há dois limites: os depósitos automáticos atuais permitem descobrir lacunas recorrentes, mas não procedimentos que o agente executou e nunca registrou; e `Proposal.source` hoje é singular. Seria necessário suportar várias referências sem perder rastreabilidade.

A exigência externa de duas sessões está no prompt, não numa checagem equivalente do validador. [experience.rs:238](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-consolidate/src/experience.rs#L238) pede isso; [auto_improve.rs:1571](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-consolidate/src/auto_improve.rs#L1571) verifica evidência preenchida, sem demonstrar esse requisito de duas origens distintas.

### 5. Fazer rejeições influenciarem novas tentativas

O ai-memory carrega rejeições anteriores, com motivo e identidade normalizada, para orientar revisões posteriores: [auto_improve.rs:971](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-consolidate/src/auto_improve.rs#L971).

Na Ulpia, [record_rejection](C:/Users/richa/Desktop/ulpia/tools/kb/src/promote.rs:701) lê o log para atualizar contagens; [proposal_prompt](C:/Users/richa/Desktop/ulpia/tools/kb/src/promote.rs:385) não recebe esse histórico. Temos o registro da recusa, mas essa parte do fluxo não aprende com ele.

**Adaptação recomendada:** evitar a repetição de uma proposta idêntica quando depósito, candidato e base não mudaram; permitir nova tentativa diante de evidência nova. Motivos detalhados podem ajudar a revisão. Não despejá-los indiscriminadamente no primeiro promotor: motivos que revelam conteúdo da biblioteca quebrariam sua premissa atual de não enxergar a base. A primeira aplicação pode ser um filtro determinístico de repetição, preservando a separação dos revisores.

### 6. Medir o efeito de uma promoção sobre a recuperação

O ai-memory oferece um avaliador executável antes de admitir propostas selecionadas. É opcional, desligado por padrão, e seu resultado depende do programa configurado. Ele não demonstra, sozinho, que uma proposta seja verdadeira ou que toda pergunta permaneça correta.

A Ulpia já tem uma versão mais específica dessa ideia para aliases: [gate.rs:94](C:/Users/richa/Desktop/ulpia/tools/kb/src/gate.rs:94) compara métricas de arquivo, agente e respostas indevidas. Porém, [promote.rs:981](C:/Users/richa/Desktop/ulpia/tools/kb/src/promote.rs:981) usa as três lentes e a validação de escrita, sem essa avaliação antes/depois das notas.

**Adaptação recomendada:** avaliar a nota candidata numa cópia temporária do corpus, antes de gravá-la, em perguntas mantidas para regressão e perguntas novas com destino esperado revisado. Comparar também cada pergunta; métricas agregadas podem esconder uma melhora que compensa numericamente outro erro. Esse gate complementa a revisão de conteúdo, sem provar veracidade factual. Ver o [contrato externo](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-consolidate/src/auto_improve.rs#L90).

## O que manter e o que deixar para depois

- **Manter nossa recuperação determinística e sua abstenção de resposta quando não há suporte.** Esta análise não executou uma comparação que justifique trocar o ranking. O ai-memory também funciona sem embeddings; seria incorreto tratá-lo como uma arquitetura obrigatoriamente vetorial.
- **Manter a revisão por contradição, duplicação e escopo.** O outro projeto não demonstra que seus resumos, confiança declarada pelo modelo ou aprovação automática substituam essas verificações.
- **Preservar Markdown e implantação local simples.** Compartilhamento por servidor, vários usuários, autenticação e gerenciamento de workstreams resolvem objetivos adicionais, com custo operacional adicional. Não são necessários para as melhorias acima.
- **Considerar relações explícitas `fixes` e `contradicts` depois.** Elas alimentam um linter determinístico no ai-memory e podem ajudar a reconciliar notas conflitantes. É uma hipótese útil para nossa manutenção, ainda sem necessidade medida aqui. [Implementação](https://github.com/akitaonrails/ai-memory/blob/74d2d31ebd8cca656c49f31563fac53e0b61c5cf/crates/ai-memory-consolidate/src/lint.rs#L216).
- **Adiar histórico temporal e esquecimento automático.** Consultas históricas do Akita registram quando o sistema soube de algo, não quando aquilo era verdadeiro no mundo. Exclusão por tempo exige uma decisão própria sobre retenção e evidência; não é consequência obrigatória de adotar memória entre sessões.

## Primeiro incremento recomendado

Corrigir a confirmação parcial de `capture`, adicionar uma forma recuperável de checkpoint e automatizar um registro curto de continuidade. Verificar quatro situações: repetição de entrega, falha em um dos destinos, interrupção antes de `SessionEnd` e retomada por outra sessão no mesmo checkout.

Com evidências de trabalho efetivamente chegando, experimentar consolidação entre sessões. Reaproveitar os promotores e instrumentos existentes, medindo propostas úteis, rejeições repetidas, rastreabilidade e regressões de recuperação. A leitura do Akita aponta onde experimentar; os resultados sobre a nossa base decidirão o que permanece.
