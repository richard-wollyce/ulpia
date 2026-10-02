# Laboratório (Lab) - Logotipos Oficiais (1500 x 1500 px)

Ativos visuais oficiais da marca **Laboratório (Lab)** com a tese *"Não é difícil, é estratégia."*, desenvolvidos em conformidade com as diretrizes do ecossistema e subdomínio `lab.richardwollyce.com`.

---

## Especificações Técnicas

| Propriedade | Valor | Contexto no Site (`lab.richardwollyce.com`) |
| :--- | :--- | :--- |
| **Família Tipográfica Oficial** | `Plus Jakarta Sans` | `font-heading` e `font-serif` em todo o projeto |
| **Base Visual da Marca** | `Lab` | Evita qualquer ambiguidade com o "L" isolado |
| **Peso (Weight)** | `ExtraBold (800)` | Densidade visual e ancoragem geométrica |
| **Espaçamento (Letter Spacing)** | `-0.035em` | `tracking-tight` de precisão |
| **Dimensões dos Arquivos** | `1500 x 1500 px` | Proporção quadrada 1:1 de alta resolução |
| **Mecânica da Animação** | CSS Grid Columns `0fr` ↔ `1fr` | `Lab` expande para `Laboratório` via `oratório` (`cubic-bezier(0.33, 1, 0.68, 1)`) |
| **Retração / Delay** | `2000ms` (2 segundos) | Retém aberto por 2s antes de voltar para `Lab` |
| **Cor Primária (Light)** | `#000000` | Preto puro |
| **Cor Primária (Dark)** | `#FFFFFF` | Branco puro |
| **Azul Oficial da Marca** | `#002776` | Azul executivo de inteligência e estratégia |
| **Fundo Escuro (Dark Mode)** | `#000000` | Preto absoluto |

---

## Estrutura de Arquivos

```
laboratorio-logos/
├── preview.html                                      # Visualizador interativo e galeria de testes
├── README.md                                         # Documentação técnica e especificações
├── fonts/                                            # Fontes originais Plus Jakarta Sans (.ttf)
│   ├── PlusJakartaSans-ExtraBold.ttf
│   └── PlusJakartaSans-Medium.ttf
├── png/                                              # Arquivos rasterizados oficiais 1500 x 1500 px
│   ├── laboratorio-marca-lab-preto-fundo-branco.png  # Marca Lab preto em fundo branco
│   ├── laboratorio-marca-lab-branco-fundo-preto.png  # Marca Lab branco em fundo preto
│   ├── laboratorio-marca-lab-azul-fundo-branco.png   # Marca Lab azul (#002776) em fundo branco
│   ├── laboratorio-marca-lab-preto-transparente.png  # Marca Lab preto transparente
│   ├── laboratorio-marca-lab-branco-transparente.png # Marca Lab branco transparente
│   ├── laboratorio-marca-lab-azul-transparente.png   # Marca Lab azul transparente
│   ├── laboratorio-palavra-preto-fundo-branco.png    # Wordmark Laboratório preto fundo branco
│   ├── laboratorio-palavra-branco-fundo-preto.png    # Wordmark Laboratório branco fundo preto
│   ├── laboratorio-palavra-azul-fundo-branco.png     # Wordmark Laboratório azul fundo branco
│   ├── laboratorio-palavra-preto-transparente.png    # Wordmark Laboratório preto transparente
│   ├── laboratorio-palavra-branco-transparente.png   # Wordmark Laboratório branco transparente
│   ├── laboratorio-palavra-azul-transparente.png     # Wordmark Laboratório azul transparente
│   ├── laboratorio-lockup-preto-fundo-branco.png     # Lockup completo (com bordão)
│   ├── laboratorio-lockup-branco-fundo-preto.png     # Lockup completo (com bordão)
│   ├── laboratorio-lockup-azul-fundo-branco.png      # Lockup completo (com bordão)
│   ├── laboratorio-lockup-preto-transparente.png     # Lockup completo transparente
│   ├── laboratorio-lockup-branco-transparente.png    # Lockup completo transparente
│   └── laboratorio-lockup-azul-transparente.png      # Lockup completo transparente
└── svg/                                              # Vetores SVG em 1500 x 1500 px
    ├── laboratorio-marca-lab-azul.svg
    ├── laboratorio-marca-lab-preto.svg
    ├── laboratorio-marca-lab-branco.svg
    ├── laboratorio-marca-lab-azul-fundo-branco.svg
    ├── laboratorio-marca-lab-preto-fundo-branco.svg
    ├── laboratorio-marca-lab-branco-fundo-preto.svg
    ├── laboratorio-logo-preto-fundo-branco.svg
    ├── laboratorio-logo-branco-fundo-preto.svg
    ├── laboratorio-logo-azul-fundo-branco.svg
    └── laboratorio-icone-app.svg
```

---

## Como Visualizar e Testar a Animação

Abra o arquivo [preview.html](file:///Users/richardwollyce/Documents/Dev/ulpia/laboratorio-logos/preview.html) diretamente no seu navegador.
Passe o cursor sobre os logotipos para verificar a transição suave de `Lab` para `Laboratório` e a retenção de 2 segundos após a saída do mouse.
