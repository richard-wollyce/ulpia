---
provenance: human
stage: ratified
---

# ADR-0040: a hosted service beside the binary, and the three clauses that keep the position true

**Search for:** `segunda fase do projeto`, `phase two`, `servico hospedado`, `hosted service`, `hospedar o ulpia`, `host ulpia for users`, `ulpia hospedado`, `SaaS`, `cobrar assinatura`, `assinatura mensal`, `assinatura do ulpia`, `monthly subscription`, `charge a subscription`, `mensalidade`, `conta de usuario`, `user account`, `contas de clientes`, `cobrar`, `paid tier`, `monetizacao`, `monetization`, `MCP oficial hospedado`, `hosted MCP connector`, `dados nos nossos servidores`, `data on our servers`, `self host continua existindo`, `self hosting stays`, `duas trilhas`, `two tracks`, `binario e servico`, `binary and service`, `posicao local primeiro`, `local first position`, `memoria pertence ao usuario`, `memory belongs to the user`, `sair copiando a pasta`, `leave by copying it out`, `exportar dados`, `data export`, `aprisionamento de fornecedor`, `vendor lock in`, `mesmo formato de pasta`, `same folder format`, `pagina de privacidade`, `privacy notice`, `promessa publica muda`, `public promise changes`, `ADR-0040`

**Exists to:** record that a hosted service is planned beside the local binary rather than instead of it, the three clauses the site already published that it must satisfy, and the public sentences that have to change on the day it ships

- **Date:** 2026-09-04
- **Status:** accepted as a direction, unbuilt. Nothing here is implemented and no date is committed.
- **Scope:** product and positioning
- **Deciders:** Richard
- **Builds on:** [[0008-single-user-open-source]], titled *build for one self hosted user, keep the hosted service possible*, which decided this two levels up and is not reversed here. This record is that clause being collected, not a new direction. [[0034-git-leaves-the-runtime]], because the privacy layer being a declaration read off the base rather than a question asked of git is what makes the same folder portable between a laptop and a server at all.
- **Reversibility:** fully, while it is unbuilt. Once accounts exist it stops being reversible in the way that matters, because the undo is other people's data.

## The decision

**A hosted service will exist beside the binary, not in place of it.** Two tracks:

| | Who runs it | Where the files are | What we receive |
|---|---|---|---|
| The binary, today | The user | Their disk | Nothing |
| The service, unbuilt | Us | Our servers | Their files, under the clauses below |

The first track does not shrink when the second arrives. A person downloads a release,
runs it, and sends us nothing, and that remains a complete way to use Ulpia rather than a
free tier of something else.

## Why this does not reverse the position, and the one reading that would

The README says **local first is a design position, not a limitation waiting to be
lifted.** That sentence survives, because local first means the local path is the primary
and complete one, not that a hosted option may never exist beside it. What would reverse
the position is a service that makes the local path the lesser one: features that exist
only hosted, an export that is a paid feature, a format the binary cannot read.

**The site has already published the version of this that keeps the promise**, in the
doors section of the front page, and it is a contract with three clauses rather than a
hedge:

> If we ever run a hosted version, it is this same folder, run for you; you leave by
> copying it out.

1. **This same folder.** The service stores markdown in folders, the same shape the binary
   reads. Not a proprietary schema with an exporter bolted on. The test is that a user can
   pull their data down and point the binary at it with no conversion step.
2. **Run for you.** It is operation, not possession. The value sold is that somebody else
   keeps it running, not that we hold something the user cannot hold.
3. **You leave by copying it out.** Export is the exit and not a feature. It is free, it is
   complete, and it does not require asking. A service that satisfies clauses one and two
   and charges for three has broken the promise more thoroughly than one that never made it.

## What must change on the day it ships, and it is a short list

**The privacy notice, first.** It currently reads, without qualification:

> Ulpia the software runs on your own machine and sends us nothing, so it is not covered
> here and does not need to be.

That sentence is true of the software and will be false of the service. It has to
distinguish the two before the service accepts its first account, not after. This is the
single item on this list that is not a matter of taste.

**The README's local first paragraph** gains the service as a named alternative rather
than losing its claim.

**Anything that says "there is no hosted instance to point at"** becomes a statement about
the binary rather than about the project. Sixteen places on the live site repeat some form
of the local promise; they were counted on 2026-09-03 and they will need re-counting, not
remembering.

## What is deliberately not decided here

Pricing, the account model, the storage design, the encryption scheme, and whether the
hosted MCP is the same five tools. None of that is settled and none of it should be
settled by this record. What is settled is the shape: two tracks, the local one whole, and
the three clauses binding the second.

## The revisit trigger

Before the first account is created. Not before the first line of code, because the
clauses constrain the design and the design will test them. If any clause turns out to be
expensive enough to argue about, that argument belongs in a new record rather than in a
quiet exception here.
