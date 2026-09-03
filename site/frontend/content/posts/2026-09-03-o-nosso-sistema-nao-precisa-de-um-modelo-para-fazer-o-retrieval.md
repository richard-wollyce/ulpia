---
title: O nosso sistema não precisa de um modelo para fazer o retrieval
date: 2026-09-03
lang: pt
description: O primeiro vídeo do canal, gravado no Museu Belas Artes do Chile: por que decidi fazer a Ulpia, por que a memória de IA deveria ser local, e por que aqui o retrieval não passa por um modelo.
source: https://youtu.be/ADNbzwcNHHY
---

Fala galera, eu tô sem microfone, então provavelmente o áudio desse vídeo vai ser
horrível. Mas eu preciso aproveitar esse momento pra falar sobre o projeto que eu tô
fazendo, Ulpia. E esse vai ser o primeiro vídeo do canal também. Esse canal aqui vai ser
focado em falar sobre tecnologia e outras coisas que eu acho interessante, mas
principalmente a gente vai falar sobre desenvolvimento de software e tecnologia.

Eu tô aqui no Museu Belas Artes do Chile, então eu achei aqui um lugar bonito pra fazer o
vídeo. Por isso que a gente tá gravando aqui. Esse vídeo vai ser bem rápido, mas eu espero
que seja bem informativo a respeito da Ulpia, e também sobre o motivo pelo qual eu decidi
fazer ela. Então vamos lá.

Eu notei que inteligência artificial realmente já faz parte da nossa vida, especialmente
quem trabalha com desenvolvimento de software. É uma coisa que a gente não consegue fugir
mais, especialmente quem trabalha já a nível de produção mesmo, quem já tá mandando o
produto pro mercado e tudo mais. Hoje em dia não tem como a gente fazer as coisas somente
na mão, codando tudo na mão. Hoje em dia é necessário ter a eficiência que a IA ajuda a
ter. E, claro, seguindo fundamentos de engenharia de software pra não shipar nada sem
qualidade.

## A documentação é um dos pilares principais

E uma das coisas que é evidente, que melhora muito o output dos seus agentes de IA, do seu
trabalho com a inteligência artificial, é justamente a documentação. Quanto mais bem
documentado for o seu sistema, melhor vai ser qualquer manutenção que você for dar ali, ou
implementação de novas features. E é por isso que eu acho que a documentação é um dos
pilares principais hoje pra quem desenvolve software.

E pensando nisso, a gente chega em um outro ponto, que é um ponto correlacionado a isso,
que não necessariamente funciona só pra quem desenvolve software. Resolve problemas de
outras pessoas, de outras áreas, mas em especial resolve pra quem precisa que um agente
tenha uma memória específica, detalhada e persistente.

## A gente chega nesse problema de AI Memory

Então, a gente chega nesse problema de AI Memory, memória de inteligência artificial. E foi
pensando nesse problema que eu identifiquei algumas possíveis soluções para o que eu estava
enfrentando ali no meu dia a dia.

Eu sei que existem algumas soluções. Existem soluções como Mem0, tem Zep, tem Letta, tem,
enfim, outras startups. Tanto o open source quanto o de código fechado também, que buscam
solucionar esse problema de memória de IA. Mas o projeto da Ulpia, eu acredito que vai ser
ainda mais revolucionário, vamos dizer assim, pelo fato de que eu decidi fazer ele não só
focado em AI Memory, mas também focado em quem quer ter as coisas localmente.

## A privacidade desses dados

Porque aí entra um outro ponto. Além da qualidade da documentação e dos registros que a
gente precisa fazer quando a gente está desenvolvendo um software, tem também a privacidade
disso. A privacidade desses dados, a forma como você guarda essas informações sensíveis,
informações de negócio e tudo mais. Essas coisas precisam ter um cuidado especial,
especialmente quando você está lidando com coisas um pouco mais delicadas e tudo mais.

E é por isso que eu acredito que o futuro da memória para inteligência artificial não deve
ser uma memória hospedada numa cloud service, e sim uma memória que você pode ter
localmente, e que você pode rodar tanto os modelos de agente de cloud quanto modelos de
agente locais.

É claro que hoje ainda não é uma realidade, por uma questão de infraestrutura. O hardware
hoje em dia ainda é muito caro. Então ter notebooks e computadores com memória RAM
suficiente, processadores suficientes para poder rodar inteligência artificial localmente,
ainda não é muito interessante, eu sei disso. Mas eu acredito que muito em breve vai ser.

E é por isso que eu já decidi tomar a iniciativa de desenvolver esse sistema, que vai te dar
a possibilidade de ter uma memória de inteligência artificial local, com uma funcionalidade
expressiva, uma latência muito baixa.

Eita, nossa, como... Vixi, agora o sol apareceu e mudou completamente o vídeo aqui.

Vai te dar uma latência muito baixa porque nós utilizamos Rust para poder fazer o caminho de
retrieval das informações dessas memórias.

E no caso, eu construí a Ulpia justamente para que você consiga fazer o retrieval e o
armazenamento dessas memórias de uma maneira que você não dependa de modelo de inteligência
artificial administrando todas as ações. Isso é um dos pontos principais da Ulpia.

## As outras soluções que existem no mercado

As outras soluções que existem no mercado, elas usam o modelo para poder registrar essas
memórias. Nós também podemos utilizar. Mas eles também usam o modelo para poder fazer o
retrieval, fazer a leitura dessas informações. E o nosso sistema não precisa de um modelo
para fazer o retrieval.

A gente utiliza um sistema inteligente de captação de informações na sua base de memória,
que faz um ranqueamento dessas memórias para poder ranquear os arquivos que são compatíveis
com o seu input, com o que você está mandando para a sua inteligência artificial lidar. E só
depois que a gente tem um ranqueamento é que de fato a gente passa para um modelo de IA,
seja local, seja cloud, enfim, um provider externo. Aí sim esse modelo vai começar a atuar
em cima das informações que aquele arquivo traz, ou seja, aquele arquivo de memória.

## Esse repositório vai ser open source

Mas enfim, é isso. Hoje eu não vou poder entrar muito em detalhes, mas eu precisava fazer um
vídeo de apresentação do projeto. Eu vou deixar o link do repositório, porque esse
repositório vai ser open source, ele vai ser público. Então vocês podem contribuir para a
evolução desse projeto, bem como podem também fazer o self-host, podem utilizar aí na
máquina de vocês e integrar com qualquer tipo de LLM que vocês quiserem. Seja local, seja
cloud, seja, por exemplo, um [inaudível 06:54] de IA, por exemplo, da Anthropic, da OpenAI,
DeepSeek, enfim, qualquer modelo de LLM que vocês quiserem utilizar.

Vocês podem implementar esse sistema. Ele funciona via MCP localmente também. Então você vai
ter o sistema, e o seu agente vai fazer chamadas via MCP e vai receber também essas
informações via MCP para poder conversar com o sistema.

E assim você vai poder economizar tokens de leitura, porque você não vai ter que esperar que
o seu modelo de LLM faça a leitura do arquivo para poder achar qual arquivo que tem o
assunto pelo qual você está buscando.

E você vai poder, obviamente, como eu falei, manusear a memória por conta própria. Você
mesmo pode escrever informações nos arquivos de memória, bem como você pode mandar um
agente, um modelo, um agente específico que você possa ter construído também. Isso aí é
outra coisa que vai ficar para um outro vídeo, é o formato que a gente utiliza para
construção de agentes especializados. Mas enfim, você também pode colocar esse modelo para
poder registrar na memória o que você precisar, as informações que você necessita.

E é isso rapaziada. Espero que vocês gostem do projeto. E a gente está aberto a feedback,
obviamente, então podem comentar aqui no vídeo, podem fazer comentários ou abrir issues lá
no repositório do GitHub, que a gente vai cuidando e vamos melhorar isso aqui, para poder
cumprir com esse propósito, que é ajudar a tornar a inteligência artificial ainda mais
eficiente, ainda mais segura e com ainda mais privacidade.

E é isso, valeu.
