use crate::ir::{Program, SourceProgram};
use crate::parser::parse_statements;
use crate::type_check::type_check_program;

// ANCHOR: compile
/// Parses and checks `source_program`, returning the parsed program.
///
/// Diagnostics from both phases can be read with `compile::accumulated`.
#[salsa::tracked(returns(copy))]
pub fn compile(db: &dyn crate::Db, source_program: SourceProgram) -> Program<'_> {
    let program = parse_statements(db, source_program);
    type_check_program(db, program);
    program
}
// ANCHOR_END: compile
