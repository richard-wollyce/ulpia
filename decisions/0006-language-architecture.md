# ADR-0006: one canonical language in the core, every language at the edge

**Search for:** `language architecture`, `arquitetura de idioma`, `canonical language`, `idioma canonico`, `base em ingles`, `ingles`, `portugues`, `bilingue`, `multilingual`, `multilingue`, `mixed language`, `misturar idiomas`, `perguntar em portugues`, `responder em portugues`, `em que idioma escrever`, `escrever as notas em ingles`, `traducao`, `traduzir`, `traduzir a base`, `translation tax`, `idioma do usuario`, `idioma da conversa`, `alias table`, `tabela de alias`, `alias`, `sinonimo`, `query expansion`, `expansao de consulta`, `expansion log`, `multilingual embeddings`, `embeddings`, `semantic search`, `busca semantica`, `cascade`, `cascata`, `kb route`, `roteador errou`, `cross language miss`, `nao achou o arquivo`, `keyword line`, `palavras chave em portugues`, `jargon`, `jargao tecnico`, `termo tecnico`, `nao tem traducao`, `fontes em ingles`, `English sources`, `declarar idioma da base`, `lint rule`, `UTC`, `minor units`, `UTF-8`, `normalizar na borda`, `normalise at the boundary`, `acentos`, `sem acento`, `Yaron`, `Steve`, `regra unica para a frota`, `excecao para uma base`

**Exists to:** Which language the notes, the keywords and the conversation are written in, and how a Portuguese question reaches an English base.

- **Date:** 2026-08-13
- **Status:** proposed
- **Scope:** fleet
- **Deciders:** Richard, Zed
- **Reversibility:** cheap now, expensive after the base grows. Every note written under a language
  policy carries it.

## Context

Measured on 2026-08-13: `kb route` works within a language and collapses across languages. "a real Portuguese advertiser question about a blocked ad, quoted in substance rather than verbatim" matched only the word `meta` in Steve's base, because the
keywords are English and the question was Portuguese.

That is not a bug in the router. It is the design meeting the real world, and the real world is worse
than the test: Richard asks in Portuguese, two of three bases are written in English, this software is
meant to be used one day by people in other languages, and **most real input is already mixed**, since
technical vocabulary does not translate.

## The mistake to avoid first: three different languages get called "the language"

| Layer | What it is | Who reads it |
|---|---|---|
| **Core** | The prose inside the notes | The agent, and anyone maintaining the base |
| **Keys** | The `Search for:` terms the router matches against | The router, matching against a human question |
| **Edge** | The conversation, in and out | The person |

They are usually discussed as one decision. They are three, they have different costs, and **the
right answer is different for each**. Conflating them is why "should we be multilingual" sounds like
an unanswerable question.

## The pattern this already is

Store timestamps in UTC and render them local. Store money in minor units with an explicit currency
and format at the edge. Store text as UTF-8 and decode on display. Every one of those was learned the
same way, by systems that stored the local representation and discovered later that they could not
convert, compare or merge anything.

**Normalise at the boundary, keep one canonical representation in the core.** Language is the same
class of problem, and this is the third time the industry has learned it.

## Options

### A. Everything in Portuguese

The base, the keys and the conversation. Coherent today, and it forecloses the stated goal: software
used by people in other cultures. It also fights the source material, since almost everything we
ingest about software is published in English.

### B. Everything in the user's language, whatever it is

A base that grows in whichever language each note happened to arrive in. Sounds inclusive and it
produces a base that cannot be searched, cannot be deduplicated, and where the same fact exists three
times with three different verdicts.

### C. Canonical core, translated edge, multilingual keys

The core stays in one language. The conversation happens in any language. The keys, which are the only
part a human question has to match, are multilingual by mechanism rather than by hand.

## Decision

**Option C, with English as the canonical language.**

Not from preference. Three reasons, in order of weight:

1. **The jargon is already English and does not translate.** There is no Portuguese word for prefill,
   KV cache, backpressure or idempotency. A Portuguese question about software is already half English,
   which means a canonical English index matches the terms that carry the routing signal **directly**.
   The mixed language input Richard is worried about is the thing that makes this work.
2. **The sources are English.** A base built by ingesting English primary sources and storing
   Portuguese distillations pays a translation tax on every single note, forever, and loses the
   author's exact terms, which are what you later need to search the source again.
3. **It is the only choice that scales past two languages.** Bilingual by hand is possible. Quadrilingual
   by hand is not, and it fails at exactly the moment the software succeeds.

**Reason 1 is measured, not argued.** The same router that failed on "a real Portuguese advertiser question about a blocked ad, quoted in substance rather than verbatim" was given "quanto custa o KV cache por token", a Portuguese sentence carrying
English jargon, against Zed's English base:

```
31  zed  decisions/0005-wake-with-the-constitution.md   matched: KV cache, kv, cache, token
30  zed  knowledge/systems/quantization-and-kv-cache.md matched: KV cache, kv, cache
19  zed  knowledge/reference/local-inference-latitude-3420.md
```

Correct file first, the multi word phrase matched whole, and a wide margin over the rest. **The
Portuguese connective tissue contributed nothing and did not need to.** The words that carry the
routing signal were already canonical.

**Revised the same day, by Richard: English for the whole fleet, Yaron included.**

The draft above carved out Yaron, on the grounds that its vocabulary is Portuguese, its vocabulary and sources are Portuguese. Richard overrode it, and he is right. Two
reasons the draft underweighted:

- **An exception is a rule nobody remembers.** "Canonical per base" means every future agent, every
  tool and every shared note starts with a question about which language it is in. One rule has no
  such question, and the coordinator agent that will read across all three bases would have paid for
  that exception forever.
- **The carve out was protecting something that does not need a Portuguese base.** Domain proper nouns and local terms are exactly that. They survive untranslated inside English prose,
  exactly as they would in a paper about Brazilian nutrition published in English. Keeping the term and
  translating the argument around it loses nothing, and the alias table already exists to catch anyone
  asking for them by their Portuguese name.

**So: one canonical language for the fleet, English. The edge stays whatever the person speaks.**

One risk to carry into the migration, and it is the only one that is not mechanical:
one safety-critical protocol file exists whose entire job is knowing when to stop and hand off to a professional. A mistranslation there is not a style problem. Emergency contact details stay verbatim in the local language, because a phone number is not a phrase to translate, and that file gets translated with more
care than the rest, not less.

## How the keys become multilingual: a cascade, not a translation layer

Each step only runs when the one before it fails. The common case pays nothing.

**Step 1. Exact and alias lookup. Free, instant, no model.**
The `Search for:` terms, plus an alias table for domain jargon. This is where most of the failure
actually is: `atributos pessoais` is Meta's own published translation of `personal attributes`. Those
pairs are finite, they are already known to whoever wrote the file, and they are worth writing down
once. A few dozen aliases per base fixes most cross language misses at zero runtime cost.

**Step 2. Query expansion by the local model. Only on a miss or a tie.**
Rewrite the question into candidate canonical terms. Bounded output, 20 to 40 tokens, which is the
cheapest kind of local generation there is, and it is machine read, so grammar constrained decoding
costs nothing here, per `stop-sequences-and-constrained-decoding`.

**Step 3. Multilingual embeddings. Later, and only when keywords stop being enough.**
Semantic search is language agnostic by construction and is the real answer at scale. It stays out of
scope until the base is large enough that hand written keys stop covering it, and it changes nothing
in [[0003-knowledge-storage]], because an embedding index is still a derived, disposable projection
over files.

**The log is the product.** Every time step 2 has to run, that is a keyword line that did not carry
the words a real question uses. The expansion log becomes the worklist for improving step 1, and step
2 gets called less over time instead of more.

## Consequences

- **Every base declares its canonical language in its `index.md`.** A base with two languages in its
  core is a base that cannot be deduplicated.
- **The edge is free.** Answering in the user's language is already how all three agents work and
  costs nothing, because a model translating its own output is not a separate step.
- **`kb` gains an alias table** and, later, an expansion hook. Both are additive.
- **We take on an alias table to maintain.** It is small, it only grows where a real miss happened, and
  it is checkable: an alias pointing at a term that no longer exists is a lint rule.
- Steve's base is already English and Zed's is already English, so nothing has to be rewritten. This
  decision mostly ratifies what exists and says why, which is the point of writing it down before the
  base is large.

## Revisit trigger

- The alias table passing a few hundred entries, which means hand maintenance is losing and step 3 is
  due.
- A second human user who does not read Portuguese or English.
- Step 2 firing on most queries rather than a minority, which would mean the keys are not doing their
  job at all.
