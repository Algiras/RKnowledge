# ADR 003: Manual Graph Manipulation vs. LLM Extraction

## Status
Accepted

## Context
While RKnowledge is primarily an LLM-based extraction tool, users sometimes need to "correct" or "bootstrap" the graph with known facts that the LLM missed or that are too specific for automated extraction.

## Decision
We introduced a `rknowledge add` command that allows direct insertion of concepts and relations into the Neo4j backend. 

- Nodes added manually are treated identical to LLM-extracted nodes to ensure interoperability.
- Manual entries are tagged with a source metadata indicating they were "manual".
- Interactive mode provides a user-friendly CLI experience for ad-hoc entry.

## Consequences
- **Pros**:
    - Completes the "Human-in-the-loop" feedback cycle.
    - Allows for rapid prototyping of graphs.
    - Enables bulk import of existing structured data.
- **Cons**:
    - Higher potential for typos (though interactive mode helps).
    - Manual nodes lack the "grounding" in document text that LLM nodes have.
