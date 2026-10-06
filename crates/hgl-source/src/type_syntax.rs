//! Balanced source type applications without semantic type resolution.
/// Split an outer application without confusing nested argument separators.
pub fn application(name: &str) -> Option<(&str, Vec<&str>)> {
    let (origin, body) = name.split_once('<')?;
    if origin.contains(['(', ')']) {
        return None;
    }
    let body = body.strip_suffix('>')?;
    let mut depth = 0_i32;
    let mut parentheses = 0_i32;
    let mut start = 0;
    let mut arguments = Vec::new();
    for (index, ch) in body.char_indices() {
        match ch {
            '(' => parentheses += 1,
            ')' => parentheses -= 1,
            '<' if parentheses == 0 => depth += 1,
            '>' if parentheses == 0 => depth -= 1,
            ',' if depth == 0 && parentheses == 0 => {
                arguments.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        if depth < 0 || parentheses < 0 {
            return None;
        }
    }
    if depth != 0 || parentheses != 0 {
        return None;
    }
    arguments.push(&body[start..]);
    Some((origin, arguments))
}

/// Recognize the contextual type relationship without consuming nested applications.
pub fn delta_argument(name: &str) -> Option<&str> {
    let (base, arguments) = application(name)?;
    if base == "delta" && arguments.len() == 1 && !arguments[0].is_empty() {
        Some(arguments[0])
    } else {
        None
    }
}
/// Split a list element and optional exact size without losing nested applications.
pub fn list_parts(name: &str) -> Option<(&str, Option<usize>)> {
    let body = name.strip_prefix("list<")?.strip_suffix('>')?;
    let mut depth = 0;
    for (index, ch) in body.char_indices() {
        if matches!(ch, '<' | '(') {
            depth += 1;
        }
        if matches!(ch, '>' | ')') {
            depth -= 1;
        }
        if ch == ',' && depth == 0 {
            let size = &body[index + 1..];
            return Some((
                &body[..index],
                if size == "unbounded" {
                    None
                } else {
                    Some(usize::try_from(size.parse::<i64>().ok()?).ok()?)
                },
            ));
        }
    }
    Some((body, None))
}
