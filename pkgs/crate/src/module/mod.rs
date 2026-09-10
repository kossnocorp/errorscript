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
    /// Owned input for worker-local ASTs. Reusing the parser and semantic build
    /// options preserves the snapshot's NodeId/SymbolId ordering.
    pub(crate) fn analysis_source(&self) -> (Arc<String>, SourceType) {
        (
            Arc::new(self.cell.borrow_owner().source_code.clone()),
            self.with_program(|program| program.source_type),
        )
    }

    pub(crate) fn from_analysis_source(source: &str, source_type: SourceType) -> Self {
        // Oxc's recursive parser/semantic traversal can exceed the default
        // blocking-thread stack on deeply nested generated source.
        stacker::maybe_grow(8 * 1024 * 1024, 16 * 1024 * 1024, || {
            Self::from_analysis_source_inner(source, source_type)
        })
    }

    fn from_analysis_source_inner(source: &str, source_type: SourceType) -> Self {
        let owner = EscModuleOwner {
            allocator: EscModuleAllocator(Allocator::new()),
            source_code: source.to_owned(),
        };
        let mut diagnostics = None;
        let cell = EscModuleCell::new(owner, |owner| {
            let parsed = Parser::new(&owner.allocator.0, &owner.source_code, source_type).parse();
            diagnostics = Some((parsed.diagnostics, parsed.panicked));
            let program: &Program<'_> = owner.allocator.0.alloc(parsed.program);
            let semantic = SemanticBuilder::new_compiler()
                .with_build_nodes(true)
                .build(program);
            EscModuleData { program, semantic }
        });
        let (diagnostics, panicked) = diagnostics.unwrap();
        Self {
            cell,
            references: EscModuleReferences::default(),
            diagnostics,
            panicked,
        }
    }

    pub fn parse(
        source_code: String,
        path: &EscModulePath,
        resolver: &EscResolver,
    ) -> Result<EscModule> {
        // Keep the same stack allowance as worker-local analysis copies.
        stacker::maybe_grow(8 * 1024 * 1024, 16 * 1024 * 1024, || {
            Self::parse_inner(source_code, path, resolver)
        })
    }

    fn parse_inner(
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
