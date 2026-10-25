use super::{
    super::{
        Model, Value,
        classify::{ClassificationQuery, satisfies},
    },
    Error,
};
use std::collections::BTreeMap;

pub(super) fn assignment(
    model: &Model,
    query: &ClassificationQuery,
    returned: Vec<(String, String)>,
    supports: Option<bool>,
) -> Result<(Vec<Value>, bool), Error> {
    if returned.len() != model.facts.len() {
        return Err(Error::InvalidAssignment(
            "wrong number of returned variables".into(),
        ));
    }
    let mut ordinals = BTreeMap::new();
    for (name, raw) in returned {
        // Parse the complete atom: rsmt2's try_int also accepts numeral prefixes.
        let ordinal = raw
            .trim()
            .parse::<usize>()
            .map_err(|_| Error::InvalidAssignment(format!("invalid ordinal for {name}: {raw}")))?;
        if ordinals.insert(name.clone(), ordinal).is_some() {
            return Err(Error::InvalidAssignment(format!(
                "duplicate variable {name}"
            )));
        }
    }
    let mut values = Vec::new();
    for (index, fact) in model.facts.iter().enumerate() {
        let name = format!("f{index}");
        let ordinal = ordinals
            .remove(&name)
            .ok_or_else(|| Error::InvalidAssignment(format!("missing variable {name}")))?;
        let value = fact.domain.get(ordinal).ok_or_else(|| {
            Error::InvalidAssignment(format!("ordinal outside {} domain", fact.key))
        })?;
        values.push(value.clone());
    }
    if !satisfies(&query.premises, &values)? {
        return Err(Error::InvalidAssignment(
            "ordinary evaluator rejected premises".into(),
        ));
    }
    let actual = query.hypothesis.evaluate(&values)?;
    if supports.is_some_and(|expected| expected != actual) {
        return Err(Error::InvalidAssignment(
            "ordinary evaluator rejected requested hypothesis polarity".into(),
        ));
    }
    Ok((values, actual))
}
