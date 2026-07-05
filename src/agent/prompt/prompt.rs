pub trait Prompt {
    fn load(name: &str) -> String;

    fn prompt(&self) -> String;
}

pub fn render_template(template: &str, variables: &[(&str, String)]) -> String {
    render_variables(&render_conditionals(template, variables), variables)
}

fn render_conditionals(template: &str, variables: &[(&str, String)]) -> String {
    let mut output = template.to_owned();
    while let Some(start) = output.find("{{#IF") {
        let Some(end) = find_block_end(&output, start) else {
            break;
        };
        let block = &output[start + "{{#IF".len()..end];
        let Some((condition, accepted, rejected)) = split_if_block(block) else {
            break;
        };
        let condition = render_variables(condition.trim(), variables);
        let selected = if is_truthy(&condition) {
            accepted.trim()
        } else {
            rejected.trim()
        }
        .to_owned();
        output.replace_range(start..end + "}}".len(), &selected);
    }
    output
}

fn render_variables(template: &str, variables: &[(&str, String)]) -> String {
    let mut output = template.to_owned();
    for (name, value) in variables {
        output = output.replace(&format!("{{{{#{name}}}}}"), value);
    }
    output
}

fn find_block_end(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = start;
    while index < text.len() {
        let rest = &text[index..];
        if rest.starts_with("{{") {
            depth += 1;
            index += 2;
            continue;
        }
        if rest.starts_with("}}") {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    None
}

fn split_if_block(block: &str) -> Option<(&str, &str, &str)> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut segment_start = 0usize;
    let bytes = block.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let rest = &block[index..];
        if rest.starts_with("{{") {
            depth += 1;
            index += 2;
            continue;
        }
        if rest.starts_with("}}") {
            depth = depth.checked_sub(1)?;
            index += 2;
            continue;
        }
        if bytes[index] == b'|' && depth == 0 {
            parts.push(&block[segment_start..index]);
            segment_start = index + 1;
        }
        index += 1;
    }
    parts.push(&block[segment_start..]);
    if parts.len() == 3 {
        Some((parts[0], parts[1], parts[2]))
    } else {
        None
    }
}

fn is_truthy(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !value.eq_ignore_ascii_case("false")
        && !value.eq_ignore_ascii_case("none")
        && value != "0"
}
