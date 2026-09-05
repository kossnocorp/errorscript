use crate::prelude::*;

use oxc_allocator::Allocator;
use oxc_parser::{Parser, ParserReturn};
use oxc_semantic::{SemanticBuilder, SemanticBuilderReturn};
use oxc_span::SourceType;
use self_cell::self_cell;

mod path;
mod references;
pub use path::*;
pub use references::*;

#[derive(Debug)]
pub struct EscModule {
    cell: EscModuleSemanticCell,
    pub references: EscModuleReferences,
}

self_cell! {
    struct EscModuleCell {
        owner: EscModuleOwner,
        #[covariant]
        dependent: EscModuleData,
    }

    impl {Debug}
}

// NOTE: The self cell owns the allocator and source code together with parser
// return, which only borrows from that owner. No borrow escapes the cell.
//
// To make it safe, we must ensure:
//
// - Send is never implemented for EscModule.
// - EscModuleCell, EscModuleOwner, EscModuleAllocator, EscModuleData,
//   EscModuleSemanticCell, EscModuleSemanticData, allocator, source_code, parsed and
//   semantic are all private.
// - Never return ParserReturn and SemanticBuilderReturn, only access via with_parsed and
//   with_semantic closures.
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
    parsed: ParserReturn<'a>,
}

impl Debug for EscModuleData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EscModuleData")
    }
}

self_cell! {
    // NOTE: EscModuleSemantic must be private,
    struct EscModuleSemanticCell {
        owner: EscModuleCell,
        #[covariant]
        dependent: EscModuleSemanticData,
    }

    impl {Debug}
}

struct EscModuleSemanticData<'a> {
    semantic: SemanticBuilderReturn<'a>,
}

impl Debug for EscModuleSemanticData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EscModuleSemanticData")
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
        let file_cell = EscModuleCell::new(owner, |owner| {
            let parsed = Parser::new(&owner.allocator.0, &owner.source_code, source_type).parse();

            EscModuleData { parsed }
        });

        let references = file_cell.with_dependent(|_, data| {
            EscModuleReferences::collect(
                &data.parsed.program,
                &data.parsed.module_record,
                path,
                resolver,
            )
        });

        let semantic_cell = EscModuleSemanticCell::new(file_cell, |file_cell| {
            let parsed = &file_cell.with_dependent(|_, data| &data.parsed);

            let semantic = SemanticBuilder::new_compiler()
                .with_build_nodes(true)
                .build(&parsed.program);

            EscModuleSemanticData { semantic }
        });

        let file = EscModule {
            cell: semantic_cell,
            references,
        };
        Ok(file)
    }

    pub fn with_parsed<Return>(
        &self,
        f: impl for<'a> FnOnce(&ParserReturn<'a>) -> Return,
    ) -> Return {
        self.cell
            .with_dependent(|file_cell, _| file_cell.with_dependent(|_, data| f(&data.parsed)))
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
