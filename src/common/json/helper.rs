use crate::external::serde_json;

/// Canonicalizes a JSON value by recursively sorting object members by
/// key, so two values that differ only in member order become identical
/// and can be compared or hashed directly. Array order carries meaning
/// and is left untouched; every other value is returned unchanged.
///
/// `serde_json` runs with `preserve_order` in this workspace, so object
/// members are sorted explicitly instead of relying on serialization.
pub fn canonical_json_value(
    value: serde_json::Value,
) -> serde_json::Value {
    match value {
        serde_json::Value::Object(members) => {
            let mut entries = members.into_iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            serde_json::Value::Object(
                entries
                    .into_iter()
                    .map(|(name, value)| {
                        (name, canonical_json_value(value))
                    })
                    .collect(),
            )
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items.into_iter().map(canonical_json_value).collect(),
        ),
        other => other,
    }
}
