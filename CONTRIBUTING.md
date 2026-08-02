# Bijdragen aan Lumi

## Branches

- `main` bevat uitsluitend productie-/releasewaardige versies;
- `dev` is de integratiebranch en de standaardbranch voor dagelijks werk;
- `feature/<korte-naam>` vertakt vanaf `dev` voor functionaliteit;
- `fix/<korte-naam>` vertakt vanaf `dev` voor normale fixes;
- `release/vX.Y.Z` vertakt vanaf `dev` voor releasevoorbereiding;
- `hotfix/vX.Y.Z` vertakt vanaf `main` voor urgente productiefixes.

Nieuwe wijzigingen gaan normaal via een pull request naar `dev`. Feature- en
fix-PR's worden gesquasht, zodat iedere gemergede PR één begrijpelijke commit op
`dev` vormt.

Een productieversie gaat via een release-PR van `dev` naar `main`. Deze PR wordt
met een normale merge commit gemergd, zodat de relatie tussen de twee branches
behouden blijft. Na een release wordt `main` teruggesynchroniseerd naar `dev`.

## Commitberichten

Gebruik Conventional Commits:

```text
feat: voeg next-trackplan toe
fix: voorkom dubbele trigger op phrasegrens
docs: verduidelijk Ableton Link timingpad
test: voeg deterministic planning scenario toe
refactor: splits planner en execution engine
build: configureer macOS signing
ci: voeg releasevalidatie toe
chore: onderhoud dependencies
```

Breaking changes krijgen `!` of een `BREAKING CHANGE:`-footer:

```text
feat(protocol)!: wijzig LightingPlan wire format
```

## Pull requests

- Houd een PR op één logisch onderwerp gericht.
- Voeg tests toe voor gewijzigde domeinlogica.
- Beschrijf gebruikersimpact, risico en verificatie.
- Gebruik `dev` als base, behalve voor release- en hotfixflows.
- Push nooit rechtstreeks naar `main`.

## Releases

De volledige procedure staat in
[`docs/release/release-and-deployment-plan.md`](docs/release/release-and-deployment-plan.md).
