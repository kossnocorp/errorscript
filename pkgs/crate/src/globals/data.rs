use super::*;

const UNKNOWN: &[EscErrorType] = &[EscErrorType::UNKNOWN];
const TYPE_ERROR: &[EscErrorType] = &[EscErrorType::builtin("TypeError")];
const VIEW_ERRORS: &[EscErrorType] = &[
    EscErrorType::builtin("TypeError"),
    EscErrorType::builtin("RangeError"),
];
const INDEX_ARGUMENTS: &[EscGlobalArgument] = &[EscGlobalArgument {
    index: Some(0),
    safe_types: PRIMITIVES,
    errors: UNKNOWN,
}];
const VIEW_ARGUMENTS: &[EscGlobalArgument] = &[
    EscGlobalArgument {
        index: Some(1),
        safe_types: PRIMITIVES,
        errors: UNKNOWN,
    },
    EscGlobalArgument {
        index: Some(2),
        safe_types: PRIMITIVES,
        errors: UNKNOWN,
    },
];
const SYNTAX_ERROR: &[EscErrorType] = &[EscErrorType::builtin("SyntaxError")];
const URI_ERROR: &[EscErrorType] = &[EscErrorType::builtin("URIError")];
const PRIMITIVES: &[&str] = &["string", "number", "boolean", "undefined", "null"];
const FORMAT_ARGUMENTS: &[EscGlobalArgument] = &[EscGlobalArgument {
    index: None,
    safe_types: PRIMITIVES,
    errors: UNKNOWN,
}];
const MESSAGE_ARGUMENTS: &[EscGlobalArgument] = &[
    EscGlobalArgument {
        index: Some(0),
        safe_types: PRIMITIVES,
        errors: UNKNOWN,
    },
    EscGlobalArgument {
        index: Some(1),
        safe_types: PRIMITIVES,
        errors: UNKNOWN,
    },
];
const AGGREGATE_ARGUMENTS: &[EscGlobalArgument] = &[
    EscGlobalArgument {
        index: Some(0),
        safe_types: &["Array", "string"],
        errors: UNKNOWN,
    },
    EscGlobalArgument {
        index: Some(1),
        safe_types: PRIMITIVES,
        errors: UNKNOWN,
    },
    EscGlobalArgument {
        index: Some(2),
        safe_types: PRIMITIVES,
        errors: UNKNOWN,
    },
];
const SUPPRESSED_ARGUMENTS: &[EscGlobalArgument] = &[EscGlobalArgument {
    index: Some(2),
    safe_types: PRIMITIVES,
    errors: UNKNOWN,
}];

const fn object(path: &'static str) -> EscGlobal {
    EscGlobal {
        path,
        value_type: "object",
        read_errors: &[],
        call: None,
        construct: None,
        instance_type: None,
    }
}

const fn function(
    path: &'static str,
    returns: &'static str,
    errors: &'static [EscErrorType],
    arguments: &'static [EscGlobalArgument],
) -> EscGlobal {
    EscGlobal {
        path,
        value_type: "Function",
        read_errors: &[],
        construct: None,
        instance_type: None,
        call: Some(EscGlobalCall {
            returns,
            errors,
            arguments,
            min_arguments: 0,
            missing_arguments_errors: &[],
        }),
    }
}

const fn error(
    path: &'static str,
    arguments: &'static [EscGlobalArgument],
    min_arguments: usize,
) -> EscGlobal {
    let effects = EscGlobalCall {
        returns: path,
        errors: &[],
        arguments,
        min_arguments,
        missing_arguments_errors: TYPE_ERROR,
    };
    EscGlobal {
        path,
        value_type: "Function",
        read_errors: &[],
        call: Some(effects),
        construct: Some(effects),
        instance_type: Some(path),
    }
}

// Add global/property paths here; resolver and analyzer logic is independent of
// the names. A property can have read errors even when it isn't callable.
pub(super) static GLOBALS: &[EscGlobal] = &[
    EscGlobal {
        path: "DataView",
        value_type: "Function",
        read_errors: &[],
        call: None,
        construct: Some(EscGlobalCall {
            returns: "DataView",
            errors: VIEW_ERRORS,
            arguments: VIEW_ARGUMENTS,
            min_arguments: 1,
            missing_arguments_errors: TYPE_ERROR,
        }),
        instance_type: Some("DataView"),
    },
    object("DataView.prototype"),
    function(
        "DataView.prototype.getUint8",
        "number",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getInt8",
        "number",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getUint16",
        "number",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getInt16",
        "number",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getUint32",
        "number",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getInt32",
        "number",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getFloat32",
        "number",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getFloat64",
        "number",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getBigUint64",
        "bigint",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    function(
        "DataView.prototype.getBigInt64",
        "bigint",
        VIEW_ERRORS,
        INDEX_ARGUMENTS,
    ),
    object("globalThis"),
    object("console"),
    function("console.log", "undefined", &[], FORMAT_ARGUMENTS),
    function("console.info", "undefined", &[], FORMAT_ARGUMENTS),
    function("console.warn", "undefined", &[], FORMAT_ARGUMENTS),
    function("console.error", "undefined", &[], FORMAT_ARGUMENTS),
    function("console.debug", "undefined", &[], FORMAT_ARGUMENTS),
    object("JSON"),
    function("JSON.parse", "unknown", SYNTAX_ERROR, FORMAT_ARGUMENTS),
    function("JSON.stringify", "string", TYPE_ERROR, FORMAT_ARGUMENTS),
    object("Math"),
    function("Math.imul", "number", &[], FORMAT_ARGUMENTS),
    function("Math.abs", "number", &[], FORMAT_ARGUMENTS),
    function("Math.floor", "number", &[], FORMAT_ARGUMENTS),
    function("Math.ceil", "number", &[], FORMAT_ARGUMENTS),
    function("Math.random", "number", &[], &[]),
    function("Math.imul", "number", &[], FORMAT_ARGUMENTS),
    function("parseInt", "number", &[], FORMAT_ARGUMENTS),
    function("parseFloat", "number", &[], FORMAT_ARGUMENTS),
    function("encodeURIComponent", "string", URI_ERROR, FORMAT_ARGUMENTS),
    function("decodeURIComponent", "string", URI_ERROR, FORMAT_ARGUMENTS),
    error("Error", MESSAGE_ARGUMENTS, 0),
    error("EvalError", MESSAGE_ARGUMENTS, 0),
    error("RangeError", MESSAGE_ARGUMENTS, 0),
    error("ReferenceError", MESSAGE_ARGUMENTS, 0),
    error("SyntaxError", MESSAGE_ARGUMENTS, 0),
    error("TypeError", MESSAGE_ARGUMENTS, 0),
    error("URIError", MESSAGE_ARGUMENTS, 0),
    error("AggregateError", AGGREGATE_ARGUMENTS, 1),
    error("SuppressedError", SUPPRESSED_ARGUMENTS, 0),
];
