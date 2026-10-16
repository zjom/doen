//! Tag validation and hierarchical matching.

use crate::frontmatter::Value;

/// Normalizes one tag to lowercase and validates it.
pub fn normalize(tag: &str) -> Result<String, String> {
    let t = tag.trim().to_lowercase();
    if t.is_empty() {
        return Err("empty tag".into());
    }
    if let Some(c) = t
        .chars()
        .find(|c| !matches!(c, 'a'..='z' | '0'..='9' | '-' | '_' | '/'))
    {
        return Err(format!(
            "invalid tag '{tag}': the character '{c}' is not allowed"
        ));
    }
    if t.split('/').any(str::is_empty) {
        return Err(format!("invalid tag '{tag}': empty segment"));
    }
    Ok(t)
}

/// Splits a comma-separated tag list and validates each tag.
pub fn parse_list(s: &str) -> Result<Vec<String>, String> {
    s.split(',')
        .filter(|t| !t.trim().is_empty())
        .map(normalize)
        .collect()
}

/// Reads tags from a sequence or a comma-separated string. Empty entries are skipped.
pub fn from_value(v: &Value) -> Result<Vec<String>, String> {
    match v {
        Value::Str(s) => parse_list(s),
        Value::Seq(items) => items
            .iter()
            .map(|t| t.as_str().ok_or("tags must be strings"))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|t| !t.trim().is_empty())
            .map(normalize)
            .collect(),
        Value::Map(_) => Err("tags must be a list".into()),
    }
}

pub fn to_value(tags: &[String]) -> Value {
    Value::Seq(tags.iter().map(|t| Value::Str(t.clone())).collect())
}

/// Whether `tag` is `filter` or below `filter` in the hierarchy.
pub fn matches(tag: &str, filter: &str) -> bool {
    tag == filter || (tag.starts_with(filter) && tag.as_bytes().get(filter.len()) == Some(&b'/'))
}

/// A tag filter: AND of groups, each group an OR of tags; and excluded tags.
#[derive(Default, Debug, Clone)]
pub struct TagFilter {
    pub all_of: Vec<Vec<String>>,
    pub none_of: Vec<String>,
}

impl TagFilter {
    pub fn new(include: &[String], exclude: &[String]) -> Result<TagFilter, String> {
        let mut f = TagFilter::default();
        for group in include {
            let g = parse_list(group)?;
            if !g.is_empty() {
                f.all_of.push(g);
            }
        }
        for group in exclude {
            f.none_of.extend(parse_list(group)?);
        }
        Ok(f)
    }

    pub fn accepts(&self, tags: &[String]) -> bool {
        let has = |f: &String| tags.iter().any(|t| matches(t, f));
        self.all_of.iter().all(|g| g.iter().any(has)) && !self.none_of.iter().any(has)
    }
}

/// Adds tags to a list, keeping order and skipping duplicates.
pub fn merge(into: &mut Vec<String>, extra: impl IntoIterator<Item = String>) {
    for t in extra {
        if !into.contains(&t) {
            into.push(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        assert_eq!(normalize("Work/Acme"), Ok("work/acme".into()));
        for bad in ["/work", "work/", "a//b", "a b", "#x", "é"] {
            assert!(normalize(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn from_yaml() {
        let s = |v: &str| Value::Str(v.into());
        assert_eq!(from_value(&s("A, b,")), Ok(vec!["a".into(), "b".into()]));
        assert_eq!(
            from_value(&Value::Seq(vec![s("x"), s(" ")])),
            Ok(vec!["x".into()])
        );
        assert!(from_value(&Value::Seq(vec![Value::Seq(vec![])])).is_err());
        assert!(from_value(&Value::Map(vec![])).is_err());
    }

    #[test]
    fn filter() {
        let tags = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let f = TagFilter::new(&["a,b".into(), "c".into()], &[]).unwrap();
        assert!(f.accepts(&tags(&["a", "c"])));
        assert!(f.accepts(&tags(&["b/x", "c"])));
        assert!(!f.accepts(&tags(&["a"])));
        let f = TagFilter::new(&["work".into()], &["work/acme".into()]).unwrap();
        assert!(f.accepts(&tags(&["work/other"])));
        assert!(!f.accepts(&tags(&["work/acme/x"])));
        assert!(!f.accepts(&tags(&["workshop"])));
    }
}
