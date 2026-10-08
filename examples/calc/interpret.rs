#[cfg(test)]
use expect_test::expect;

use crate::ir::{Expression, ExpressionData, FunctionId, Op, Program, StatementData, VariableId};
use crate::type_check::find_function;

/// The maximum depth of nested function calls before the interpreter gives up.
///
/// `calc` has no conditionals, so any recursive function recurses forever.
/// The checker doesn't detect recursion, so the interpreter reports it instead.
const MAX_CALL_DEPTH: usize = 64;

// ANCHOR: interpret_program
/// Executes each `print` statement in `program` and returns the printed lines.
#[salsa::tracked(returns(deref))]
pub fn interpret_program<'db>(db: &'db dyn crate::Db, program: Program<'db>) -> Vec<String> {
    let interpreter = Interpreter { db, program };
    program
        .statements(db)
        .iter()
        .filter_map(|statement| match &statement.data {
            StatementData::Function(_) => None,
            StatementData::Print(expression) => Some(match interpreter.eval(expression, &[], 0) {
                Ok(value) => value.to_string(),
                Err(message) => format!("error: {message}"),
            }),
        })
        .collect()
}
// ANCHOR_END: interpret_program

// ANCHOR: interpreter
struct Interpreter<'db> {
    db: &'db dyn crate::Db,
    program: Program<'db>,
}

impl<'db> Interpreter<'db> {
    /// Evaluates `expression`, where `env` holds the values of the variables in scope.
    fn eval(
        &self,
        expression: &Expression<'db>,
        env: &[(VariableId<'db>, f64)],
        depth: usize,
    ) -> Result<f64, String> {
        match &expression.data {
            ExpressionData::Number(n) => Ok(n.into_inner()),
            ExpressionData::Op(left, op, right) => {
                let left = self.eval(left, env, depth)?;
                let right = self.eval(right, env, depth)?;
                Ok(match op {
                    Op::Add => left + right,
                    Op::Subtract => left - right,
                    Op::Multiply => left * right,
                    Op::Divide => left / right,
                })
            }
            ExpressionData::Variable(v) => env
                .iter()
                .find(|(name, _)| name == v)
                .map(|&(_, value)| value)
                .ok_or_else(|| format!("the variable `{}` is not declared", v.text(self.db))),
            ExpressionData::Call(f, args) => self.call(*f, args, env, depth),
        }
    }

    fn call(
        &self,
        name: FunctionId<'db>,
        args: &[Expression<'db>],
        env: &[(VariableId<'db>, f64)],
        depth: usize,
    ) -> Result<f64, String> {
        let db = self.db;
        let Some(function) = find_function(db, self.program, name) else {
            return Err(format!("the function `{}` is not declared", name.text(db)));
        };
        let parameters = function.args(db);
        if parameters.len() != args.len() {
            return Err(format!(
                "the function `{}` takes {} argument(s) but {} were supplied",
                name.text(db),
                parameters.len(),
                args.len(),
            ));
        }
        if depth == MAX_CALL_DEPTH {
            return Err(format!(
                "the call to `{}` recursed too deeply",
                name.text(db)
            ));
        }

        // Arguments are evaluated in the caller's environment...
        let callee_env = parameters
            .iter()
            .zip(args)
            .map(|(&parameter, arg)| Ok((parameter, self.eval(arg, env, depth)?)))
            .collect::<Result<Vec<_>, String>>()?;

        // ...and the body in a fresh environment containing only the parameters.
        self.eval(function.body(db), &callee_env, depth + 1)
    }
}
// ANCHOR_END: interpreter

/// Parses and interprets `source_text`, returning the printed lines.
#[cfg(test)]
fn interpret_string(source_text: &str) -> Vec<String> {
    use salsa::Database;

    use crate::db::CalcDatabaseImpl;
    use crate::ir::SourceProgram;
    use crate::parser::parse_statements;

    CalcDatabaseImpl::default().attach(|db| {
        let source_program = SourceProgram::new(db, source_text.to_string());
        let program = parse_statements(db, source_program);
        interpret_program(db, program).to_vec()
    })
}

#[test]
fn interpret_example() {
    let output = interpret_string(
        "
            fn area_rectangle(w, h) = w * h
            fn area_circle(r) = 3.14 * r * r
            print area_rectangle(3, 4)
            print area_circle(1)
            print 11 * 2
        ",
    );
    expect![[r#"
        [
            "12",
            "3.14",
            "22",
        ]
    "#]]
    .assert_debug_eq(&output);
}

#[test]
fn interpret_nested_calls() {
    let output = interpret_string(
        "
            fn double(a) = a * 2
            fn quadruple(a) = double(double(a))
            print quadruple(2) - 1
            print quadruple(1) / 8
        ",
    );
    expect![[r#"
        [
            "7",
            "0.5",
        ]
    "#]]
    .assert_debug_eq(&output);
}

#[test]
fn interpret_errors() {
    let output = interpret_string(
        "
            fn forever(a) = forever(a + 1)
            fn add(a, b) = a + b
            print forever(1)
            print add(1)
            print missing(1)
            print 1 + 1
        ",
    );
    expect![[r#"
        [
            "error: the call to `forever` recursed too deeply",
            "error: the function `add` takes 2 argument(s) but 1 were supplied",
            "error: the function `missing` is not declared",
            "2",
        ]
    "#]]
    .assert_debug_eq(&output);
}

// ANCHOR: interpret_reuse
#[test]
fn interpret_reuses_unchanged_program() {
    use salsa::Setter;

    use crate::db::CalcDatabaseImpl;
    use crate::ir::SourceProgram;
    use crate::parser::parse_statements;

    let mut db = CalcDatabaseImpl::default();
    let source_program = SourceProgram::new(&db, "print 1 + 2".to_string());
    let program = parse_statements(&db, source_program);
    assert_eq!(interpret_program(&db, program), ["3"]);

    // Adding trailing whitespace re-runs the parser, but the parser produces
    // an identical `Program`, so the interpreter's memoized output is reused.
    source_program
        .set_text(&mut db)
        .to("print 1 + 2\n\n".to_string());
    db.enable_logging();
    let program = parse_statements(&db, source_program);
    assert_eq!(interpret_program(&db, program), ["3"]);
    expect![[r#"
        [
            "WillExecute { database_key: parse_statements(Id(0)) }",
        ]
    "#]]
    .assert_debug_eq(&db.take_logs());

    // Changing an operand changes the program, so the interpreter re-runs.
    source_program
        .set_text(&mut db)
        .to("print 1 + 5".to_string());
    let program = parse_statements(&db, source_program);
    assert_eq!(interpret_program(&db, program), ["6"]);
    expect![[r#"
        [
            "WillExecute { database_key: parse_statements(Id(0)) }",
            "WillExecute { database_key: interpret_program(Id(100)) }",
        ]
    "#]]
    .assert_debug_eq(&db.take_logs());
}
// ANCHOR_END: interpret_reuse
