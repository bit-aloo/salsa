use db::CalcDatabaseImpl;
use ir::{Diagnostic, SourceProgram};
use salsa::Database as Db;
use salsa::Setter as _;

mod compile;
mod db;
mod interpret;
mod ir;
mod parser;
mod type_check;

const EXAMPLE: &str = "\
fn area_rectangle(w, h) = w * h
fn area_circle(r) = 3.14 * r * r
print area_rectangle(3, 4)
print area_circle(1)
print 11 * 2
";

// ANCHOR: main
pub fn main() {
    let mut db = CalcDatabaseImpl::default();

    // Run the program named on the command line, if any.
    if let Some(path) = std::env::args().nth(1) {
        let source_text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            eprintln!("failed to read `{path}`: {error}");
            std::process::exit(1);
        });
        let source_program = SourceProgram::new(&db, source_text);
        run(&db, source_program);
        return;
    }

    // Otherwise, run the built-in example...
    let source_program = SourceProgram::new(&db, EXAMPLE.to_string());
    run(&db, source_program);

    // ...then edit it and run it again. Salsa re-executes only what the edit affected.
    let edited = format!("{EXAMPLE}print area_circle(2)\n");
    source_program.set_text(&mut db).to(edited);
    println!("--- after edit ---");
    run(&db, source_program);
}

/// Compiles `source_program`, then interprets it if no errors were found.
fn run(db: &dyn Db, source_program: SourceProgram) {
    let program = compile::compile(db, source_program);
    let diagnostics = compile::compile::accumulated::<Diagnostic>(db, source_program);
    if !diagnostics.is_empty() {
        for diagnostic in diagnostics {
            eprintln!("{}", diagnostic.render(db, source_program));
        }
        return;
    }

    for line in interpret::interpret_program(db, program) {
        println!("{line}");
    }
}
// ANCHOR_END: main
