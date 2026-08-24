# What a deploy needs before production

**Search for:** `deploy`, `producao`, `production`, `release`, `rollback`, `lancar em producao`, `subir pra producao`, `ship to production`, `checklist de deploy`, `deploy checklist`, `zero downtime`, `downtime zero`, `janela de deploy`, `deploy window`, `reverter deploy`, `voltar atras`, `pipeline`, `ci`, `continuous integration`, `feature flag`, `canario`, `canary`, `blue green`, `quem aprova deploy`, `who approves a deploy`

**Exists to:** The checklist a change passes before production, and who decides

A change reaches production when the tests pass in CI, the rollback path is written
down before the deploy rather than improvised during it, and the person who owns the
service has said yes. A deploy without a rollback plan is a bet, not a release.
Canary first when the blast radius is unknown; blue-green when the cutover must be
instant; feature flags when the risk is in the feature rather than the infrastructure.
