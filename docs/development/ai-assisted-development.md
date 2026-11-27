# AI-assisted development

_Part of the [development docs](../../DEVELOPMENT.md)._

HoploDex is developed with AI coding agents, using two tools.

## Spec Kit

[Spec Kit](https://github.com/github/spec-kit) is a spec-driven development
toolkit. Each feature is planned through its `/speckit-*` workflow, which
writes the feature's folder under `specs/NNN-name/`: the spec, plan, data
model, contracts and tasks. The
[constitution](../../.specify/memory/constitution.md) governs the work, and
overrides other guidance. How code cites a spec's requirements is in
[citations-and-attack-surface.md](citations-and-attack-surface.md).

## Quartermaster

[Quartermaster](https://github.com/FrostyFitz/quartermaster) is the agent
supervisor that runs development work. It gives each task to a worker agent in
an isolated copy of the repository, validates the change, and opens a pull
request, which the maintainer reviews and merges.
