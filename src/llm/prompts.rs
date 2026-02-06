/// System prompt for extracting relations from text
pub const GRAPH_EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a network graph maker who extracts terms and their relations from a given context.
You are provided with a context chunk (delimited by ```). Your task is to extract the ontology of terms mentioned in the given context. These terms should represent the key concepts as per the context.

Thought 1: While traversing through each sentence, think about the key terms mentioned in it.
    Terms may include object, entity, location, organization, person, condition, acronym, documents, service, concept, etc.
    Terms should be as atomistic as possible.

Thought 2: Think about how these terms can have one on one relation with other terms.
    Terms that are mentioned in the same sentence or the same paragraph are typically related to each other.
    Terms can be related to many other terms.

Thought 3: Find out the relation between each such related pair of terms.

Thought 4: Classify each term with a short descriptive type that best captures what it is (e.g. "programming language", "database", "design pattern", "medical condition", "company", "algorithm", "city", "person", "framework", etc.). Use whatever type naturally fits — do not constrain yourself to a fixed list.

Format your output as a JSON array. Each element of the array contains a pair of terms and the relation between them:
[
    {
        "node_1": "A concept from extracted ontology",
        "node_1_type": "descriptive type for node_1",
        "node_2": "A related concept from extracted ontology",
        "node_2_type": "descriptive type for node_2",
        "edge": "relationship between the two concepts, node_1 and node_2 in one or two sentences"
    }
]

Rules:
- Extract only the most important and meaningful relationships
- Keep node names concise (1-4 words)
- Node names should be lowercase
- Entity types should be short (1-3 words), lowercase, and descriptive of what the entity actually is
- Edge descriptions should be brief but descriptive
- Return at least 3-5 relationships if the text is substantial
- Return an empty array [] if no meaningful relationships can be extracted
- Output ONLY valid JSON, no other text"#;

/// User prompt template for extracting relations
pub fn graph_extraction_user_prompt(text: &str) -> String {
    format!("context: ```{}```\n\noutput: ", text)
}

/// System prompt for concept extraction (alternative approach)
#[allow(dead_code)]
pub const CONCEPT_EXTRACTION_SYSTEM_PROMPT: &str = r#"Your task is to extract the key concepts (and non-personal entities) mentioned in the given context.
Extract only the most important and atomistic concepts, if needed break the concepts down to simpler concepts.

Categorize the concepts in one of the following categories:
[event, concept, place, object, document, organisation, condition, misc]

Format your output as a JSON array:
[
    {
        "entity": "The Concept",
        "importance": 1-5,
        "category": "The Type of Concept"
    }
]

Rules:
- importance is the contextual importance of the concept on a scale of 1 to 5 (5 being the highest)
- Entity names should be lowercase
- Output ONLY valid JSON, no other text"#;
