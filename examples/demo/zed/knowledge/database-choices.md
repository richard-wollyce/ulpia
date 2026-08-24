# Choosing a database, and what each choice costs

**Search for:** `banco de dados`, `database`, `escolher banco`, `choose a database`, `sql`, `nosql`, `postgres`, `sqlite`, `indice`, `index`, `consulta lenta`, `slow query`, `migracao de schema`, `schema migration`, `transacao`, `transaction`, `consistencia`, `consistency`, `replicacao`, `replication`, `backup do banco`, `database backup`, `modelagem de dados`, `data modeling`

**Exists to:** The trade-offs between embedded and served databases, stated plainly

An embedded database (SQLite) buys zero operations and loses concurrent writers; a
served one (Postgres) buys concurrency and costs a machine that must be cared for.
Start embedded, move when the writer count forces you to, and never before: the
migration is mechanical, but the operations burden is forever.
