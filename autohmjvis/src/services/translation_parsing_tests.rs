// Temporary test file for parse_translation_map / extract_json resilience.
// Delete once satisfied.

#[cfg(test)]
mod tests {
    use crate::services::translate::{extract_json, parse_translation_map};

    // --- extract_json ---

    #[test]
    fn extract_json_strips_markdown_fence() {
        let s = "```json\n{\"EN\":\"hello\"}\n```";
        assert_eq!(extract_json(s), "{\"EN\":\"hello\"}");
    }

    #[test]
    fn extract_json_strips_surrounding_text() {
        let s = "Here is the JSON: {\"EN\":\"hi\"} done.";
        assert_eq!(extract_json(s), "{\"EN\":\"hi\"}");
    }

    #[test]
    fn extract_json_stops_at_first_balanced_close() {
        let s = "{\"EN\":\"hello\"}\n\"}\n\"}\n\"}\n\"}";
        assert_eq!(extract_json(s), "{\"EN\":\"hello\"}");
    }

    #[test]
    fn extract_json_handles_array_wrapped_object() {
        // extract_json slices from first { to last }, so the inner object is returned
        let s = "[{\"EN\":\"hello\"}]";
        assert_eq!(extract_json(s), "{\"EN\":\"hello\"}");
    }

    // --- parse_translation_map ---

    #[test]
    fn parse_handles_leading_brace_quote_prefix() {
        // AI prefixes the real JSON with a stray {" on its own line
        let s = "{\"\n{\"EN\": \"hello\", \"ES\": \"hola\"}";
        let result = parse_translation_map(s).expect("fallback should recover");
        assert_eq!(result.get("EN").map(String::as_str), Some("hello"));
        assert_eq!(result.get("ES").map(String::as_str), Some("hola"));
    }

    #[test]
    fn parse_handles_extra_quoted_values() {
        // serde_json parses {"EN": "\"hello\""} as value = "hello" (with literal quotes)
        // parse_translation_map strips those surrounding quotes
        let json = r#"{"EN": "\"hello\""}"#;
        // Direct serde parse gives us the string with literal quote chars
        let map: std::collections::HashMap<String, String> =
            serde_json::from_str(json).expect("serde should succeed here");
        assert_eq!(map["EN"], "\"hello\"");

        // But parse_translation_map strips them
        let result = parse_translation_map(json).expect("fallback should recover");
        assert_eq!(result["EN"], "hello");
    }

    #[test]
    fn parse_handles_nested_wrapper_key() {
        let json = r#"{"translations": {"EN": "hello", "ES": "hola"}}"#;
        let result = parse_translation_map(json).expect("fallback should recover");
        assert_eq!(result.get("EN").map(String::as_str), Some("hello"));
        assert_eq!(result.get("ES").map(String::as_str), Some("hola"));
    }

    #[test]
    fn parse_handles_array_wrapped_object() {
        let json = r#"[{"EN": "hello", "FR": "bonjour"}]"#;
        let result = parse_translation_map(json).expect("fallback should recover");
        assert_eq!(result.get("EN").map(String::as_str), Some("hello"));
        assert_eq!(result.get("FR").map(String::as_str), Some("bonjour"));
    }

    #[test]
    fn parse_returns_none_on_garbage() {
        assert!(parse_translation_map("not json at all").is_none());
        assert!(parse_translation_map("").is_none());
        assert!(parse_translation_map("[]").is_none());
    }

    #[test]
    fn parse_flat_map_no_extra_quotes_passthrough() {
        let json = r#"{"EN": "hello", "KO": "안녕"}"#;
        let result = parse_translation_map(json).expect("should parse");
        assert_eq!(result.get("EN").map(String::as_str), Some("hello"));
        assert_eq!(result.get("KO").map(String::as_str), Some("안녕"));
    }
}
