use crate::prelude::*;

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_diagnostics::Diagnostics;
use oxc_parser::Parser;
use oxc_semantic::{SemanticBuilder, SemanticBuilderReturn};
use oxc_span::SourceType;
use self_cell::self_cell;

mod id;
pub use id::*;

mod path;
pub use path::*;

mod references;
pub use references::*;

#[derive(Debug)]
pub struct EscModule {
    cell: EscModuleCell,
    pub references: EscModuleReferences,
    pub diagnostics: Diagnostics,
    pub panicked: bool,
}

self_cell! {
    struct EscModuleCell {
        owner: EscModuleOwner,
        #[covariant]
        dependent: EscModuleData,
    }

    impl {Debug}
}

// SAFETY: The cell owns the allocator and source text. Program is allocated in
// that arena, and the dependent only holds references into the owner. Moving the
// module transfers exclusive ownership of the entire arena and its dependents.
//
// - Sync must never be implemented for EscModule.
// - EscModuleCell, EscModuleOwner, EscModuleAllocator, EscModuleData,
//   allocator, source_code, program and semantic remain private.
// - Arena-backed data is only accessed through with_program and with_semantic
//   closures, which prevent borrowed data from escaping the cell.
unsafe impl Send for EscModule {}

#[derive(Debug)]
struct EscModuleOwner {
    allocator: EscModuleAllocator,
    source_code: String,
}

struct EscModuleAllocator(Allocator);

impl Debug for EscModuleAllocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EscModuleAllocator")
    }
}

struct EscModuleData<'a> {
    program: &'a Program<'a>,
    semantic: SemanticBuilderReturn<'a>,
}

impl Debug for EscModuleData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EscModuleData")
    }
}

impl EscModule {
    pub fn parse(
        source_code: String,
        path: &EscModulePath,
        resolver: &EscResolver,
    ) -> Result<EscModule> {
        let owner = EscModuleOwner {
            allocator: EscModuleAllocator(Allocator::new()),
            source_code,
        };

        let source_type = SourceType::from_path(path).context("Failed to determine source type")?;
        let mut metadata = None;
        let cell = EscModuleCell::new(owner, |owner| {
            let parsed = Parser::new(&owner.allocator.0, &owner.source_code, source_type).parse();
            let references = EscModuleReferences::collect(
                &parsed.program,
                &parsed.module_record,
                path,
                resolver,
            );
            metadata = Some((references, parsed.diagnostics, parsed.panicked));

            // Both dependents borrow the owner, rather than one borrowing the other.
            let program: &Program<'_> = owner.allocator.0.alloc(parsed.program);
            let semantic = SemanticBuilder::new_compiler()
                .with_build_nodes(true)
                .build(program);

            EscModuleData { program, semantic }
        });
        let (references, diagnostics, panicked) =
            metadata.expect("module metadata collected during parsing");

        let file = EscModule {
            cell,
            references,
            diagnostics,
            panicked,
        };
        Ok(file)
    }

    pub fn with_program<Return>(&self, f: impl for<'a> FnOnce(&Program<'a>) -> Return) -> Return {
        self.cell.with_dependent(|_, data| f(data.program))
    }

    pub fn with_semantic<Return>(
        &self,
        f: impl for<'a> FnOnce(&SemanticBuilderReturn<'a>) -> Return,
    ) -> Return {
        self.cell.with_dependent(|_, data| f(&data.semantic))
    }

    pub fn extract_dependencies(&self) -> Vec<EscModulePath> {
        self.references.dependencies().cloned().collect()
    }
}
