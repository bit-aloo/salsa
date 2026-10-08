#[cfg(test)]
use expect_test::expect;
use salsa::Accumulator;
#[cfg(test)]
use test_log::test;

use crate::ir::{
    Diagnostic, Expression, Function, FunctionId, Program, Span, StatementData, VariableId,
};

// ANCHOR: type_check_program
#[salsa::tracked(returns(copy))]
pub fn type_check_program<'db>(db: &'db dyn crate::Db, program: Program<'db>) {
    for statement in program.statements(db) {
        match &statement.data {
            StatementData::Function(f) => type_check_function(db, *f, program),
            StatementData::Print(e) => CheckExpression::new(db, program, &[]).check(e),
        }
    }
}

#[salsa::tracked(returns(copy))]
pub fn type_check_function<'db>(
    db: &'db dyn crate::Db,
    function: Function<'db>,
    program: Program<'db>,
) {
    CheckExpression::new(db, program, function.args(db)).check(function.body(db))
}
// ANCHOR_END: type_check_program

// ANCHOR: find_function
#[salsa::tracked(returns(copy))]
pub fn find_function<'db>(
    db: &'db dyn crate::Db,
    program: Program<'db>,
    name: FunctionId<'db>,
) -> Option<Function<'db>> {
    program
        .statements(db)
        .iter()
        .flat_map(|s| match &s.data {
            StatementData::Function(f) if f.name(db) == name => Some(*f),
            _ => None,
        })
        .next()
}
// ANCHOR_END: find_function

// ANCHOR: check_expression
struct CheckExpression<'input, 'db> {
    db: &'db dyn crate::Db,
    program: Program<'db>,
    names_in_scope: &'input [VariableId<'db>],
}

impl<'input, 'db> CheckExpression<'input, 'db> {
    pub fn new(
        db: &'db dyn crate::Db,
        program: Program<'db>,
        names_in_scope: &'input [VariableId<'db>],
    ) -> Self {
        CheckExpression {
            db,
            program,
            names_in_scope,
        }
    }
}

impl<'db> CheckExpression<'_, 'db> {
    fn check(&self, expression: &Expression<'db>) {
        match &expression.data {
            crate::ir::ExpressionData::Op(left, _, right) => {
                self.check(left);
                self.check(right);
            }
            crate::ir::ExpressionData::Number(_) => {}
            crate::ir::ExpressionData::Variable(v) => {
                if !self.names_in_scope.contains(v) {
                    self.report_error(
                        expression.span,
                        format!("the variable `{}` is not declared", v.text(self.db)),
                    );
                }
            }
            crate::ir::ExpressionData::Call(f, args) => {
                if self.find_function(*f).is_none() {
                    self.report_error(
                        expression.span,
                        format!("the function `{}` is not declared", f.text(self.db)),
                    );
                }
                for arg in args {
                    self.check(arg);
                }
            }
        }
    }

    // ANCHOR_END: check_expression

    fn find_function(&self, f: FunctionId<'db>) -> Option<Function<'db>> {
        find_function(self.db, self.program, f)
    }

    // ANCHOR: report_error
    fn report_error(&self, span: Span, message: String) {
        Diagnostic::new(span.start(self.db), span.end(self.db), message).accumulate(self.db);
    }
    // ANCHOR_END: report_error
}

/// Create a new database with the given source text and parse the result.
/// Returns the statements and the diagnostics generated.
#[cfg(test)]
fn check_string(
    source_text: &str,
    expected_diagnostics: expect_test::Expect,
    edits: &[(&str, expect_test::Expect)],
) {
    use salsa::{Database, Setter};

    use crate::db::CalcDatabaseImpl;
    use crate::ir::SourceProgram;
    use crate::parser::parse_statements;

    // Create the database
    let mut db = CalcDatabaseImpl::default();
    db.enable_logging();

    // Create the source program
    let source_program = SourceProgram::new(&db, source_text.to_string());

    // Invoke the parser
    let program = parse_statements(&db, source_program);

    // Read out any diagnostics
    db.attach(|db| {
        let rendered_diagnostics: String =
            type_check_program::accumulated::<Diagnostic>(db, program)
                .into_iter()
                .map(|d| d.render(db, source_program))
                .collect::<Vec<_>>()
                .join("\n");
        expected_diagnostics.assert_eq(&rendered_diagnostics);
    });

    // Apply edits and check diagnostics/logs after each one
    for (new_source_text, expected_diagnostics) in edits {
        source_program
            .set_text(&mut db)
            .to(new_source_text.to_string());

        db.attach(|db| {
            let program = parse_statements(db, source_program);
            expected_diagnostics
                .assert_debug_eq(&type_check_program::accumulated::<Diagnostic>(db, program));
        });
    }
}

#[test]
fn check_print() {
    check_string("print 1 + 2", expect![""], &[]);
}

// ANCHOR: check_bad_variable_in_program
#[test]
fn check_bad_variable_in_program() {
    check_string(
        "print a + b",
        expect![[r#"
            error: the variable `a` is not declared
             --> input:1:7
              |
            1 | print a + b
              |       ^ here
            error: the variable `b` is not declared
             --> input:1:11
              |
            1 | print a + b
              |           ^ here"#]],
        &[],
    );
}
// ANCHOR_END: check_bad_variable_in_program

#[test]
fn check_bad_function_in_program() {
    check_string(
        "print a(22)",
        expect![[r#"
            error: the function `a` is not declared
             --> input:1:7
              |
            1 | print a(22)
              |       ^^^^^ here"#]],
        &[],
    );
}

#[test]
fn check_bad_variable_in_function() {
    check_string(
        "
            fn add_one(a) = a + b
            print add_one(22)
        ",
        expect![[r#"
            error: the variable `b` is not declared
             --> input:2:33
              |
            2 |             fn add_one(a) = a + b
              |                                 ^ here"#]],
        &[],
    );
}

#[test]
fn check_bad_function_in_function() {
    check_string(
        "
            fn add_one(a) = add_two(a) + b
            print add_one(22)
        ",
        expect![[r#"
            error: the function `add_two` is not declared
             --> input:2:29
              |
            2 |             fn add_one(a) = add_two(a) + b
              |                             ^^^^^^^^^^ here
            error: the variable `b` is not declared
             --> input:2:42
              |
            2 |             fn add_one(a) = add_two(a) + b
              |                                          ^ here"#]],
        &[],
    );
}

#[test]
fn fix_bad_variable_in_function() {
    check_string(
        "
            fn double(a) = a * b
            fn quadruple(a) = double(double(a))
            print quadruple(2)
        ",
        expect![[r#"
            error: the variable `b` is not declared
             --> input:2:32
              |
            2 |             fn double(a) = a * b
              |                                ^ here"#]],
        &[(
            "
                fn double(a) = a * 2
                fn quadruple(a) = double(double(a))
                print quadruple(2)
            ",
            expect![[r#"
                []
            "#]],
        )],
    );
}

// ANCHOR: check_reuse
#[test]
fn check_reuses_unchanged_functions() {
    use salsa::Setter;

    use crate::db::CalcDatabaseImpl;
    use crate::ir::SourceProgram;
    use crate::parser::parse_statements;

    let mut db = CalcDatabaseImpl::default();
    let source_program = SourceProgram::new(
        &db,
        "
            fn double(a) = a * 2
            fn quadruple(a) = double(double(a))
            print quadruple(2)
        "
        .to_string(),
    );
    let program = parse_statements(&db, source_program);
    type_check_program(&db, program);

    // Edit the body of `double`, keeping the rest of the program unchanged.
    source_program.set_text(&mut db).to("
            fn double(a) = a * 3
            fn quadruple(a) = double(double(a))
            print quadruple(2)
        "
    .to_string());
    db.enable_logging();
    let program = parse_statements(&db, source_program);
    type_check_program(&db, program);

    // The parser re-runs, but it produces equal `statements` (only the body of
    // `double` changed), so `type_check_program` is not re-executed. Salsa still
    // re-checks `double`, whose `body` changed, but not `quadruple`.
    expect![[r#"
        [
            "WillExecute { database_key: parse_statements(Id(0)) }",
            "WillExecute { database_key: type_check_function(Id(300)) }",
        ]
    "#]]
    .assert_debug_eq(&db.take_logs());
}
// ANCHOR_END: check_reuse
