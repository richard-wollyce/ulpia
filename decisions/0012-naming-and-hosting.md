# ADR-0012: The system is Vesta, and everything lives under a personal name

**Search for:** `Vesta`, `nome`, `naming`, `nomear`, `batizar`, `escolher nome`, `renomear`, `marca`, `brand`, `branding`, `rebranding`, `Wollyce`, `Richard Wollyce`, `richardwollyce.com`, `vesta.richardwollyce.com`, `dominio`, `subdominio`, `subdomain`, `registrar dominio`, `trademark`, `registro de marca`, `marca registrada`, `INPI`, `USPTO`, `advogado`, `Wollner`, `semiotica`, `logo`, `logotipo`, `simbolo`, `abstrato`, `nome descritivo`, `colisao`, `concorrente`, `Nestor`, `Otto`, `Janus`, `Maia`, `Jano`, `jan.ai`, `Alva`, `Nara`, `Tino`, `besta`, `fonetica`, `homofono`, `pronuncia`, `mitologia`, `deusa`, `romana`, `fogo`, `Vestais`, `estatua`, `orquestrador`, `orquestradora`, `fleet.txt`, `identidade`, `nome pessoal`, `empresa`, `socios`

**Exists to:** Why the orchestrator is called Vesta, why everything sits on subdomains of richardwollyce.com, which candidate names lost and to whom, and what a personal umbrella name costs.

**Status:** accepted
**Date:** 2026-08-17
**Supersedes:** nothing
**Related:** [ADR-0008](0008-single-user-open-source.md), [ADR-0011](0011-fleet-layout.md)

## Context

The orchestrator had no name. `fleet.txt` had no `name =` line, so `kb fleet` fell back
to the directory name and answered "Fleet", which is the category, not an identity.

Steve was asked to name it and did the work from his own base rather than from general
knowledge, which is the first time an agent in this fleet answered a real question with
its own distilled material. Two of his findings drove the outcome.

**Wollner's rule, from a dossier in Steve's base on Wollner's semiotics:**
a mark must be abstract and created, never literal. The file's evidence is four
phone-repair storefronts that all chose a phone as their logo. Applied to naming, this
eliminates the entire descriptive class on sight: Router, Hub, Conductor, Maestro,
Nexus, Fleetmind. They describe the function, which is exactly what makes every one of
them interchangeable with the others.

**The mythological namespace for AI orchestrators is already taken.** Verified by web
search, at result level rather than primary source: Nestor is an agent platform in Rust,
Otto is ServiceNow's assistant, Janus is three separate AI companies, Maia is a
Microsoft accelerator, and `nilo-assistant.com` pitches almost verbatim what this
project pitches.

Separately, Richard decided the umbrella brand. Not a product name, his own: **Richard
Wollyce**, at `richardwollyce.com`, with each system on a subdomain.

## Decision

**The system and its orchestrator are both called Vesta.** One name, because they are
one thing from the user's side: what you talk to is what you are using.

`fleet.txt` now carries it, which is the only place it is written:

```
name = Vesta
role = Keeper of the fleet: knows who is here, routes what arrives, and holds the fleet's memory
```

**Everything is hosted under `richardwollyce.com`**, one subdomain per system:
`vesta.richardwollyce.com`, `community.richardwollyce.com`, `blog.richardwollyce.com`.
Trademark registration is deferred until there is money for lawyers.

**The fleet's directory stays `fleet/`.** The directory is the structure; the name in
`fleet.txt` is the identity. Renaming it would buy nothing and would cost three
absolute paths outside the fleet: the tray's pointer file, `claude_desktop_config.json`
and `.mcp.json`.

## Why Vesta and not the runners up

Vesta is the only major Roman deity who had no statue. She was represented by the fire
itself and never depicted, which is Wollner's rule as a historical fact rather than as
an argument. Her Vestals guarded Rome's most important documents, and her fire was
ritually extinguished and relit every March.

That maps onto ADR-0003 exactly, which is why the name survives contact with the
architecture instead of decorating it:

> The index is the fire, put out and lit again. The markdown files are the wills, and
> those are what is actually guarded.

It also solves the family problem structurally. Zed, Steve and Yaron are three
colleagues. **Vesta is not a fourth colleague, she is the house they work in.** Same
single-word register, different kind of thing, and the phonetic shape is distinct from
all three.

**Jano** was second and lost on the project's own rule, recorded in
a dossier in Steve's base on the AI education market: build
vocabulary you can own rather than competing for someone else's. `jan.ai` is a
well-known local-first open-source AI assistant, meaning the same category and the same
positioning, so Jano would sit permanently downstream of it in every search a curious
developer runs.

**Alva** and **Nara** are phonetically clean and mean nothing, with three or four small
AI products already standing in each. **Tino** is the best pun available in Portuguese,
"ter tino" being the faculty of knowing what belongs where, but a VC-funded São Paulo
fintech already owns that name in Richard's own market.

## Why a personal name is the right umbrella

Not sentiment. The mechanism is that **a civil name has no third party who can hold
prior rights to it**, because the string is the person. Every product name in this
project's category is a race against someone who filed earlier, and Steve's collision
table is what that race looks like from behind. `richardwollyce.com` has no such race
to lose.

It also puts the trademark question in the right order. Registering a mark costs money
and lawyers, and the reason to register early is to stop somebody else claiming the
name first. Under a personal name that risk is much lower for the umbrella, so the
spending waits until there is something worth defending. Product names underneath can
be registered one at a time, when each one earns it.

**Unverified:** the legal reasoning above is general and was not checked against Brazilian
statute or an attorney. Before registering anything, INPI in Brazil and USPTO classes 9
and 42, as Steve flagged.

## Consequences we are accepting

**The brand does not transfer.** A personal name ties the company's value to a person.
If Richard ever sells, steps back, or takes on partners, "Richard Wollyce" does not
move with the asset the way an invented mark would. This is a known trade with known
precedents, and it is a real cost rather than a technicality.

**Vesta is one phoneme from "besta".** B and V are distinct phonemes in Brazil, so this
is a live minimal pair and not a homophone, but the joke is available to the first
person who wants it, and it becomes a true homophone in the northern Portuguese
dialects where the two merge. Steve raised this against his own recommendation and set
a test: say "Sou Vesta, a orquestradora da frota" out loud three times before
committing. Of his five finalists, his pick has the only unclean phonetic neighbourhood.

**The subdomain choice retires the weaker objection.** Steve's secondary worry was that
`vesta.com` belongs to a funded company already using agent language. Hosting at
`vesta.richardwollyce.com` means the project never needed that domain, so a collision
there costs nothing.

**One name for two things is a bet.** If the fleet ever ships more than one orchestrator,
or Vesta becomes one component among several, the name will be doing two jobs and will
have to be split. Cheap to reverse today, because the name exists in exactly one file.
