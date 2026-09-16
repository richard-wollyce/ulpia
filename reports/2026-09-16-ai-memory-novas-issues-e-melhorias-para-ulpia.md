# Análise Atualizada: ai-memory (Akita) vs Ulpia (Setembro/2026)

Inspeção direta do repositório [akitaonrails/ai-memory](https://github.com/akitaonrails/ai-memory), incluindo README atualizado, issues recentes, discussões de usuários e PRs submetidos entre 14 e 16 de setembro de 2026.

---

## 1. O que os Usuários e Colaboradores estão Reportando no ai-memory

A análise das issues e PRs mais recentes revelou dores reais dos usuários em produção, além de desafios arquiteturais enfrentados pelo projeto de Akita:

### A. Falha Crítica de Isolamento Multi-Usuário (Issue #708 e PR #746)
- **O Problema**: A versão 2.1+ do `ai-memory` adicionou autenticação (OIDC e tokens humanos), mas **não implementou autorização por projeto ou repositório**. Qualquer usuário autenticado no servidor consegue ler páginas e fazer buscas sobre projetos de outros usuários.
- **Caso Real**: Um usuário (`alice`) registrou tarifas confidenciais (`"ALICE CONFIDENTIAL: day rate is 900 EUR"`), e outro usuário comum recém-criado (`bob`) conseguiu extrair o texto completo via `memory_query`.
- **Resolução de Projetos Frágil**: O `ai-memory` resolve o nome do projeto pelo nome da pasta base (`basename`). Se dois desenvolvedores clonarem repositórios diferentes chamados `api/`, ambos colidem na mesma linha do banco.
- **Contraste com a Ulpia**: A Ulpia não adota um servidor compartilhado centralizado com tenancy cega. A Ulpia opera localmente por base de arquivos com camadas privadas explícitas (`private_layer` em `base.rs` / `ADR-0034`). Arquivos de `profile/`, `projects/` e `records/` nunca são indexados na camada pública nem vazam para consultas não autorizadas.

### B. Instabilidade e Flakiness de Testes por Uso de Servidor Daemon (Issue #745)
- **O Problema**: O `ai-memory` depende de um servidor daemon em background (`DEFAULT_BIND = 127.0.0.1:49374` ou portas dinâmicas). Rodar a suíte de testes em paralelo no macOS resulta em falhas constantes (`110 passed, 8 failed`), travamentos em portas de socket e contenção de shutdown signals (`SIGINT`/`SIGTERM`).
- **Contraste com a Ulpia**: A Ulpia opera **em processo** (in-process) como CLI e biblioteca Rust pura sobre SQLite FTS5 local. Todos os 484 testes rodam em paralelo em ~3 segundos com zero concorrência de portas, zero processos zumbis e zero flakiness.

### C. Pressão Corporativa por Garantias de Privacidade e Air-Gapped (PR #744)
- **O Problema**: Empresas e equipes na União Europeia (especialmente Alemanha) exigiram documentação formal sobre tratamento de dados corporativos (`DATA_HANDLING.md`), conformidade GDPR, garantias de funcionamento 100% offline (air-gapped) e política de retenção/exclusão.
- **Oportunidade para a Ulpia**: A Ulpia já nasceu 100% local e offline (invariante de custo zero no harness), mas ainda não possui um documento formal `DATA_HANDLING.md` consolidando suas garantias de telemetria zero e air-gap.

### D. Incompatibilidades Estritas de Esquema MCP (PR #741 e PR #738)
- **O Problema**: Modelos e frontends rigorosos (Moonshot/Kimi, Google Gemini/Vertex, AWS Bedrock) rejeitam esquemas JSON MCP que utilizam construções recursivas (`$defs` com `oneOf` sem `type: string` na raiz), disparando erros HTTP 400 (`infinite recursion without termination condition`).
- **Situação na Ulpia**: O construtor `tool()` em [`tools/kb/src/mcp.rs`](file:///c:/Users/richa/Desktop/ulpia/tools/kb/src/mcp.rs#L689) já gera esquemas planos com `type: "object"` e tipos primitivos explícitos, evitando esse bug por padrão.

### E. Captura Automática de Resposta Final de Agentes (Issue #743)
- **O Cenário**: Usuários solicitaram captura automática da última resposta do assistente (`last_assistant_message`) nos hooks de encerramento (`Stop` e `SubagentStop`) do OpenAI Codex CLI 0.154+.
- **Oportunidade para a Ulpia**: O `kb capture` da Ulpia atualmente captura perguntas, roteamentos e recusas. Incorporar a última resposta do assistente (quando opted-in e devidamente higienizada) enriquece a entrada do `kb consolidate`.

---

## 2. O que a Ulpia Deve Melhorar (Propostas Acionáveis)

Com base nas lições do `ai-memory`, propomos 4 melhorias cirúrgicas para a Ulpia:

### 1. Criação do `DATA_HANDLING.md` (Garantias Enterprise & Privacy-First)
Formalizar em um documento claro na raiz e no site `ulpia.io`:
- **Armazenamento 100% Local**: Os dados residem estritamente no sistema de arquivos do usuário (`fleet/` e `.kb/`).
- **Zero Telemetria e Zero Rede**: O binário `kb` não abre conexões externas, não envia métricas de uso e não possui analytics.
- **Air-Gapped por Design**: Funcionalidade completa garantida sem conexão com a internet.
- **Privacidade por Camadas**: Explicação clara de como o `private_layer` isola dados pessoais de índices públicos.

### 2. Semântica de Reivindicação em Handoffs (`kb handoff`)
- O `ai-memory` adota o conceito de *handoff baton* reivindicado exatamente uma vez (*claimed exactly once*).
- No `kb handoff` da Ulpia, podemos adicionar um estado formal de ciclo de vida (`pending` -> `claimed` -> `completed`). Isso evita que múltiplos agentes ou sessões paralelas tentem executar a mesma tarefa de passagem simultaneamente.

### 3. Captura Higienizada de Turnos do Assistente (`kb capture`)
- Permitir que o `kb capture` receba opcionalmente o resumo ou conclusão gerada pelo assistente no fim da sessão.
- Aplicar a barreira de higienização de segredos antes de persistir o evento, garantindo que o `kb consolidate` possa aprender não apenas com o que faltou, mas com o que foi solucionado com sucesso.

### 4. Resolução Estável de Repositório
- Manter a regra: a identidade do repositório deve ser derivada da raiz do Git (`git rev-parse --show-toplevel` / remote `origin`), e nunca apenas pelo nome da pasta corrente, evitando a colisão que afeta o `ai-memory` (#708).

---

## 3. Vantagens Arquiteturais que a Ulpia Já Possui

1. **Sem Daemons Residentes**: Não há servidores HTTP rodando em background para travar portas ou consumir RAM ociosa.
2. **Sem Custos de Embeddings Vetoriais**: O motor BM25/FTS5 + Small-to-big windowing entrega precisão cirúrgica sem exigir chaves de API pagas da OpenAI/Gemini.
3. **Validação Constitucional e Autocrítica**: A Ulpia possui filtros constitucionais de revisão em `kb panel` e grading de tríade offline (`kb eval --triad`), garantindo que alucinações sejam detectadas deterministicamente.
4. **Isolamento de Tenancy Confiável**: A separação por arquivos e bases locais previne qualquer vazamento acidental de dados entre agentes ou clientes.
