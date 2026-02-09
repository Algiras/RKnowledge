# ADR 002: Domain-Aware LLM Prompt Engineering

## Status
Accepted

## Context
Standard LLM extraction prompts are generalized and may miss nuances in specialized domains like medical research, legal analysis, or software architecture. Users need a way to guide the extraction process without rewriting the entire prompt logic.

## Decision
We implemented a hierarchical prompt templating system:
1.  **Base Prompt**: A static, proven prompt for graph extraction.
2.  **Domain Profile**: A configuration-based profile (`[domain]`) containing focus areas and suggested entity types.
3.  **CLI Overrides**: On-the-fly context (`--context`) and file-based templates (`--context-file`) that are appended to the system prompt.

The `prompts.rs` module dynamically constructs the final system prompt by combining these layers.

## Consequences
- **Pros**:
    - High flexibility for specialized tasks.
    - No code changes required for new domains.
    - Context file support allows users to reuse complex domain-specific instructions.
- **Cons**:
    - System prompt size increases with more context, consuming more tokens.
    - Conflicting instructions between layers could potentially confuse the LLM.
