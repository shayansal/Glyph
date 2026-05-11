# Glyphspace Standard Draft

## Manifest Format

A Glyphspace manifest is a JSON document with `spec_version`, `id`, `name`, `glyphs`, `edges`, `capabilities`, `policies`, `spatial_semantics`, and `metadata`.

## Capability Format

A capability defines `id`, `name`, `description`, `intent`, input/output schemas, required permissions, risk, reversibility, audit, confirmation, domain tags, and natural-language aliases.

## Patch Format

A patch defines `spec_version`, `id`, `description`, and `ops`. Patch operations may move, resize, group, collapse, expand, hide optional glyphs, change density/depth/style, create summaries/agents, reorder focus, and bind only existing authorized capabilities.

## Lens Format

A lens is a named patch intended for a role, user, team, session, or AI suggestion layer. Layer order is base app, organization policy, role lens, user lens, session state, and temporary AI suggestions.

## Policy Format

Policy rules define mandatory surfaces, risk gates, confirmation requirements, accessibility preservation, and capability permission checks. User patches cannot mutate organization policy.

## Accessibility Expectations

Every interactive glyph has a role and label. High-risk actions have stable confirmation paths. Personalization cannot disable the accessibility tree or hide required trust surfaces.

## Serialization Rules

JSON is canonical. Stable IDs are required. Unknown extension fields with registered namespaces must be preserved and warned about by validators.

## Versioning Rules

Documents include `spec_version`. Engines expose `engine_version`, `schema_version`, feature flags, and extension namespaces.

## Extension Namespaces

Examples: `com.company.crm`, `org.glyphspace.xr`, `org.glyphspace.healthcare`, `org.glyphspace.finance`, `org.glyphspace.education`.

## Conformance Tests

Conformance suites validate schema parsing, policy invariants, accessibility survival, deterministic layout, patch reversibility, and safe AI proposal handling.

The reference suite also certifies renderer determinism, GPU pipeline planning, screenshot conformance, host adapter behavior, host certification, patch compatibility, schema compatibility, accessibility-frame validity, and semantic SSR hydration. `gx conformance --world <file>` is the CLI entrypoint for implementers.

## Security Model

AI and personalization may rearrange UI but may not create authority, bypass confirmation, impersonate users, mutate server-of-record data, change audit settings, or hide legal/payment/security requirements.

## Extension Points

Hosts may add renderers, device profiles, layout solvers, AI adapters, domain schemas, and capability namespaces while preserving the core policy and accessibility contract.

## Governance Model

The reference implementation and spec should remain separate. The standard should use public RFCs, an extension registry, security and accessibility reviews, and semantic versioning.
