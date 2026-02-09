# ADR 001: Property-based Tenant Isolation

## Status
Accepted

## Context
RKnowledge needs to support multi-tenancy to allow different projects or users to store isolated knowledge graphs in the same Neo4j instance. Neo4j Community Edition does not support multiple databases per instance, which is an Enterprise-only feature.

## Decision
We will implement property-based isolation. Every node and relationship (where applicable) will have a `tenant` property. All Cypher queries will include a MUST-MATCH condition on this property (e.g., `WHERE n.tenant = $tenant`).

## Consequences
- **Pros**:
    - Compatible with Neo4j Community Edition.
    - Low overhead for small to medium graphs.
    - Easy to implement with existing `neo4rs` driver.
- **Cons**:
    - No native security boundary (a user with access to Neo4j can see all data).
    - Query complexity increases as every query needs the tenant filter.
    - Performance may degrade on extremely large graphs compared to database-level isolation.
- **Future-proofing**: The `Config` struct includes an `isolation_mode` field to eventually support "database" isolation for Neo4j Enterprise users.
