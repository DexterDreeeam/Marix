use marix_e2e_slugify::slugify;

#[test]
fn normalizes_words_and_case() {
    assert_eq!(slugify("Hello, Marix World!"), "hello-marix-world");
}

#[test]
fn collapses_separator_runs() {
    assert_eq!(slugify("rust___agents   work"), "rust-agents-work");
}

#[test]
fn removes_leading_and_trailing_separators() {
    assert_eq!(slugify("---Ship It---"), "ship-it");
}

#[test]
fn preserves_ascii_digits() {
    assert_eq!(slugify(" MX 42 "), "mx-42");
}
