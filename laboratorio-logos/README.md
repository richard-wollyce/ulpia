# Laboratório (Lab) - Logotipos Oficiais (1500 x 1500 px)

Ativos visuais oficiais da marca **Laboratório (Lab)** com a tese *"Não é difícil, é estratégia."*, desenvolvidos em conformidade com as diretrizes do ecossistema e subdomínio `lab.richardwollyce.com`.

---

## Especificações Técnicas

| Propriedade | Valor | Contexto no Site (`lab.richardwollyce.com`) |
| :--- | :--- | :--- |
| **Família Tipográfica Oficial** | `Plus Jakarta Sans` | `font-heading` e `font-serif` em todo o projeto |
| **Peso (Weight)** | `ExtraBold (800)` | Densidade visual e ancoragem geométrica |
| **Espaçamento (Letter Spacing)** | `-0.035em` | `tracking-tight` de precisão |
| **Dimensões dos Arquivos** | `1500 x 1500 px` | Proporção quadrada 1:1 de alta resolução |
| **Mecânica da Animação** | CSS Grid Columns `0fr` ↔ `1fr` | Idêntico ao Veredito (`cubic-bezier(0.33, 1, 0.68, 1)`) |
| **Retração / Delay** | `2000ms` (2 segundos) | Retém aberto por 2s antes de voltar ao L isolado |
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
│   ├── laboratorio-palavra-preto-fundo-branco.png    # Wordmark preto em fundo branco
│   ├── laboratorio-palavra-branco-fundo-preto.png    # Wordmark branco em fundo preto
│   ├── laboratorio-palavra-azul-fundo-branco.png     # Wordmark azul (#002776) em fundo branco
│   ├── laboratorio-palavra-preto-transparente.png    # Wordmark preto fundo transparente
│   ├── laboratorio-palavra-branco-transparente.png    # Wordmark branco fundo transparente
│   ├── laboratorio-palavra-azul-transparente.png     # Wordmark azul fundo transparente
│   ├── laboratorio-letra-l-preto-fundo-branco.png    # Glifo L preto em fundo branco
│   ├── laboratorio-letra-l-branco-fundo-preto.png    # Glifo L branco em fundo preto
│   ├── laboratorio-letra-l-azul-fundo-branco.png     # Glifo L azul em fundo branco
│   ├── laboratorio-letra-l-preto-transparente.png    # Glifo L preto fundo transparente
│   ├── laboratorio-letra-l-branco-transparente.png   # Glifo L branco fundo transparente
│   ├── laboratorio-letra-l-azul-transparente.png     # Glifo L azul fundo transparente
│   ├── laboratorio-lockup-preto-fundo-branco.png     # Lockup completo (com bordão)
│   ├── laboratorio-lockup-branco-fundo-preto.png     # Lockup completo (com bordão)
│   ├── laboratorio-lockup-azul-fundo-branco.png      # Lockup completo (com bordão)
│   ├── laboratorio-lockup-preto-transparente.png     # Lockup completo transparente
│   ├── laboratorio-lockup-branco-transparente.png    # Lockup completo transparente
│   └── laboratorio-lockup-azul-transparente.png      # Lockup completo transparente
└── svg/                                              # Vetores SVG em 1500 x 1500 px
    ├── laboratorio-letra-l-preto.svg
    ├── laboratorio-letra-l-branco.svg
    ├── laboratorio-letra-l-azul.svg
    ├── laboratorio-logo-preto-fundo-branco.svg
    ├── laboratorio-logo-branco-fundo-preto.svg
    ├── laboratorio-logo-azul-fundo-branco.svg
    └── laboratorio-icone-app.svg
```

---

## Como Visualizar e Testar a Animação

Abra o arquivo [preview.html](file:///Users/richardwollyce/Documents/Dev/ulpia/laboratorio-logos/preview.html) diretamente no seu navegador.
Passe o cursor sobre os logotipos para verificar a expansão em tempo real e a retenção de 2 segundos após a saída do mouse.
