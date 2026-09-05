use crate::parser::prelude::*;

#[derive(Debug)]
pub struct EscParserFile {
    // NOTE: EscParserFileCell must be private,
    cell: EscParserFileCell,
}

self_cell! {
    // NOTE: EscParserFileCell must be private,
    struct EscParserFileCell {
        owner: EscParserFileOwner,
        #[covariant]
        dependent: EscParserFileData,
    }

    impl {Debug}
}

// NOTE: The self cell owns the allocator and source code together with parser
// return, which only borrows from that owner. No borrow escapes the cell.
//
// To make it safe, we must ensure:
//
// - Send is never implemented for EscParserFile.
// - EscParserFileCell, EscParserFileOwner, EscParserFileData, allocator,
//   source_code, parsed are all private.
// - Never return ParserReturn, only access via with_parsed closure.
unsafe impl Send for EscParserFile {}

#[derive(Debug)]
struct EscParserFileOwner {
    allocator: EscParserFileAllocator,
    source_code: String,
}

pub struct EscParserFileAllocator(Allocator);

impl Debug for EscParserFileAllocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EscParserFileAllocator")
    }
}

struct EscParserFileData<'a> {
    parsed: ParserReturn<'a>,
}

impl Debug for EscParserFileData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EscParserFileData")
    }
}

impl EscParserFile {
    pub fn parse(source_code: String, path: &PathBuf) -> Result<EscParserFile> {
        let owner = EscParserFileOwner {
            allocator: EscParserFileAllocator(Allocator::new()),
            source_code,
        };

        let source_type =
            SourceType::from_path(&path).context("Failed to determine source type")?;
        let cell = EscParserFileCell::new(owner, |owner| {
            let parsed = Parser::new(&owner.allocator.0, &owner.source_code, source_type).parse();
            EscParserFileData { parsed }
        });

        let file = EscParserFile { cell };
        Ok(file)
    }

    pub fn with_parsed<Return>(
        &self,
        f: impl for<'a> FnOnce(&ParserReturn<'a>) -> Return,
    ) -> Return {
        self.cell.with_dependent(|_, data| f(&data.parsed))
    }
}
