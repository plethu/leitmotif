use super::{Error, error::protocol};
use rsmt2::{Solver, errors::ErrorKind as SolverErrorKind};
use std::io::{BufRead, BufReader};

pub(super) struct InfoResponse {
    pub(super) raw: String,
    pub(super) value: String,
}

pub(super) fn get(solver: &mut Solver<()>, flag: &str) -> Result<InfoResponse, Error> {
    solver
        .get_info(flag)
        .map_err(|source| protocol("solver info request", source))?;
    let mut raw = String::new();
    // Z3 prints these responses on one line. Avoid get_sexpr's structural
    // parentheses handling inside quoted strings and preserve the solver's reader.
    BufReader::with_capacity(1, solver)
        .read_line(&mut raw)
        .map_err(|source| protocol("solver info response", source.into()))?;
    parse(&raw, flag)
}

fn parse(raw: &str, flag: &str) -> Result<InfoResponse, Error> {
    let text = raw.trim_ascii_start();
    if let Some(tail) = text.strip_prefix("unsupported") {
        end(tail, raw)?;
        return Err(protocol(
            "solver info response",
            SolverErrorKind::Unsupported.into(),
        ));
    }
    let body = text
        .strip_prefix('(')
        .ok_or_else(|| invalid("expected get-info response opening", raw))?
        .trim_ascii_start();
    let (solver_error, field) = if let Some(field) = body.strip_prefix("error") {
        (true, field)
    } else {
        (
            false,
            body.strip_prefix(flag)
                .ok_or_else(|| invalid("unexpected get-info response field", raw))?,
        )
    };
    let field = field
        .trim_ascii_start()
        .strip_prefix('"')
        .ok_or_else(|| invalid("expected quoted get-info field", raw))?;
    let (value, tail) = quoted_field(field, raw)?;
    let tail = tail
        .trim_ascii_start()
        .strip_prefix(')')
        .ok_or_else(|| invalid("expected get-info response closing", raw))?;
    end(tail, raw)?;
    if solver_error {
        return Err(protocol(
            "solver info response",
            SolverErrorKind::SolverError(value).into(),
        ));
    }
    Ok(InfoResponse {
        raw: raw.trim_ascii().to_owned(),
        value,
    })
}

// rsmt2 also byte-slices malformed Unicode tokens. Keep this command-specific
// parser UTF-8 safe; rsmt2 has no quoted-string decoder. This handles only the single string field
// of the two get-info responses, using SMT-LIB's doubled-quote escaping.
fn quoted_field<'a>(mut text: &'a str, raw: &str) -> Result<(String, &'a str), Error> {
    let mut value = String::new();
    loop {
        let (segment, tail) = text
            .split_once('"')
            .ok_or_else(|| invalid("unterminated get-info string", raw))?;
        value.push_str(segment);
        if let Some(rest) = tail.strip_prefix('"') {
            value.push('"');
            text = rest;
        } else {
            return Ok((value, tail));
        }
    }
}
fn end(tail: &str, raw: &str) -> Result<(), Error> {
    if !tail.trim_ascii().is_empty() {
        return Err(invalid("trailing get-info response fields", raw));
    }
    Ok(())
}
fn invalid(message: &str, raw: &str) -> Error {
    protocol(
        "solver info response",
        SolverErrorKind::ParseError(message.into(), raw.into()).into(),
    )
}

#[cfg(test)]
mod tests {
    use super::get;
    use rsmt2::{SmtConf, Solver};

    #[test]
    fn real_z3_info_consumption_preserves_session_after_unknown() {
        let mut config = SmtConf::z3("z3");
        config.check_success();
        config.option("-t:100");
        let mut solver = Solver::new(config, ()).unwrap();
        let version = get(&mut solver, ":version").unwrap();
        assert!(version.value.contains("5.3.1"));
        solver.declare_const("f0", "Int").unwrap();
        solver.assert("(and (<= 0 f0) (< f0 2))").unwrap();
        solver.set_option(":rlimit", 1).unwrap();
        assert_eq!(solver.check_sat_or_unk().unwrap(), None);
        let reason = get(&mut solver, ":reason-unknown").unwrap();
        assert_eq!(reason.value, "max. resource limit exceeded");
        solver.set_option(":rlimit", 0).unwrap();
        assert_eq!(solver.check_sat_or_unk().unwrap(), Some(true));
        solver.kill().unwrap();
    }
}
