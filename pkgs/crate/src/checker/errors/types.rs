use super::*;

impl Analyzer<'_, '_> {
    pub(super) fn declared_parameters(&self, id: &EscFnId, count: usize) -> Option<Vec<Value>> {
        let signatures = self
            .graph
            .signatures
            .get(id)
            .filter(|signatures| !signatures.is_empty())?;
        let mut values = vec![Value::default(); count];
        for signature in signatures {
            self.modules[&signature.module_id].with_semantic(|result| {
                let AstKind::Function(function) = result.semantic.nodes().kind(signature.node)
                else {
                    return;
                };
                for (index, value) in values.iter_mut().enumerate() {
                    let parameter = function.params.items.get(index);
                    let mut declared = parameter.map_or_else(
                        || {
                            if function.params.rest.is_some() {
                                Value::unknown()
                            } else {
                                Value::undefined()
                            }
                        },
                        |parameter| {
                            parameter.type_annotation.as_ref().map_or_else(
                                Value::unknown,
                                |annotation| {
                                    self.annotation_at(
                                        &signature.module_id,
                                        &result.semantic,
                                        &annotation.type_annotation,
                                        &mut HashSet::new(),
                                    )
                                },
                            )
                        },
                    );
                    if parameter.is_some_and(|parameter| parameter.optional) {
                        declared.join(Value::undefined());
                    }
                    value.join(declared);
                }
            });
        }
        Some(values)
    }
    pub(super) fn annotation(&self, ty: &TSType<'_>) -> Value {
        self.annotation_at(self.module, self.semantic, ty, &mut HashSet::new())
    }

    fn annotation_at(
        &self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        ty: &TSType<'_>,
        visiting: &mut HashSet<EscErrorId>,
    ) -> Value {
        match ty {
            TSType::TSNumberKeyword(_) => Value::builtin("number"),
            TSType::TSStringKeyword(_) => Value::builtin("string"),
            TSType::TSBooleanKeyword(_) => Value::builtin("boolean"),
            TSType::TSBigIntKeyword(_) => Value::builtin("bigint"),
            TSType::TSSymbolKeyword(_) => Value::builtin("symbol"),
            TSType::TSNullKeyword(_) => Value {
                truth: 1,
                ..Value::builtin("null")
            },
            TSType::TSUndefinedKeyword(_) | TSType::TSVoidKeyword(_) => Value::undefined(),
            TSType::TSNeverKeyword(_) => Value::default(),
            TSType::TSArrayType(array) => Value {
                elements: self
                    .annotation_at(module, semantic, &array.element_type, visiting)
                    .types,
                truth: 2,
                ..Value::builtin("Array")
            },
            TSType::TSTupleType(_) => Value::builtin("Array"),
            TSType::TSUnionType(union) => {
                let mut value = Value::default();
                for ty in &union.types {
                    value.join(self.annotation_at(module, semantic, ty, visiting));
                }
                value
            }
            TSType::TSTypeLiteral(literal) => {
                Value::types(HashSet::from([EscErrorType::Node(EscErrorId {
                    module_id: module.clone(),
                    node: literal.node_id.get(),
                })]))
            }
            TSType::TSTypeReference(reference) => {
                if let TSTypeName::IdentifierReference(id) = &reference.type_name
                    && matches!(id.name.as_str(), "Array" | "ReadonlyArray")
                    && id.reference_id.get().is_some_and(|id| {
                        semantic.scoping().get_reference(id).symbol_id().is_none()
                    })
                {
                    return Value {
                        elements: reference
                            .type_arguments
                            .as_ref()
                            .and_then(|arguments| arguments.params.first())
                            .map_or_else(
                                || Value::unknown().types,
                                |ty| self.annotation_at(module, semantic, ty, visiting).types,
                            ),
                        truth: 2,
                        ..Value::builtin("Array")
                    };
                }
                self.graph
                    .type_references
                    .get(&EscErrorId {
                        module_id: module.clone(),
                        node: reference.node_id.get(),
                    })
                    .map_or_else(Value::unknown, |types| {
                        let mut value = Value::default();
                        for ty in types {
                            value.join(self.expand_alias(ty, visiting));
                        }
                        value
                    })
            }
            _ => Value::unknown(),
        }
    }

    fn expand_alias(&self, ty: &EscErrorType, visiting: &mut HashSet<EscErrorId>) -> Value {
        let EscErrorType::Node(id) = ty else {
            return Value::types(HashSet::from([ty.clone()]));
        };
        if !visiting.insert(id.clone()) {
            return Value::unknown();
        }
        let value = self
            .modules
            .get(&id.module_id)
            .map_or_else(Value::unknown, |module| {
                module.with_semantic(|result| {
                    if let AstKind::TSTypeAliasDeclaration(alias) =
                        result.semantic.nodes().kind(id.node)
                    {
                        self.annotation_at(
                            &id.module_id,
                            &result.semantic,
                            &alias.type_annotation,
                            visiting,
                        )
                    } else {
                        Value::types(HashSet::from([ty.clone()]))
                    }
                })
            });
        visiting.remove(id);
        value
    }

    /// Typed data-property access follows the declared TS shape. Getter/proxy
    /// effects that are not expressed by that shape aren't inferred here.
    pub(super) fn property(&self, value: &Value, name: &str) -> (Value, Types) {
        let mut result = Value::default();
        let mut errors = Types::new();
        for object in &value.globals {
            if let Some(global) =
                EscGlobals::property(object, name).and_then(|global| self.graph.global(global.path))
            {
                result.join(Value::global(global));
                errors.extend(global.read_errors.iter().cloned());
            } else {
                result.join(Value::unknown());
                errors.insert(EscErrorType::UNKNOWN);
            }
        }
        for ty in value.types.iter().filter(|_| value.nonglobal) {
            if let EscErrorType::Buildin(instance) = ty
                && let Some(global) = self
                    .graph
                    .global(&format!("{}.prototype.{name}", instance.as_str()))
            {
                result.join(Value::global(global));
                errors.extend(global.read_errors.iter().cloned());
                continue;
            }
            if let Some(value) = self.property_type(ty, name, &mut HashSet::new()) {
                result.join(value);
            } else {
                result.join(Value::unknown());
                errors.insert(EscErrorType::UNKNOWN);
            }
        }
        if result.types.is_empty() {
            result = Value::unknown();
            errors.insert(EscErrorType::UNKNOWN);
        }
        (result, errors)
    }

    fn property_type(
        &self,
        ty: &EscErrorType,
        name: &str,
        visiting: &mut HashSet<EscErrorId>,
    ) -> Option<Value> {
        if name == "length"
            && matches!(ty, EscErrorType::Buildin(id) if matches!(id.as_str(), "Array" | "string"))
        {
            return Some(Value::builtin("number"));
        }
        let EscErrorType::Node(id) = ty else {
            return None;
        };
        if !visiting.insert(id.clone()) {
            return None;
        }
        let value = self.modules.get(&id.module_id).and_then(|module| {
            module.with_semantic(|result| match result.semantic.nodes().kind(id.node) {
                AstKind::TSInterfaceDeclaration(interface) => {
                    if let Some(value) = self.signature_property(
                        &id.module_id,
                        &result.semantic,
                        &interface.body.body,
                        name,
                    ) {
                        return Some(value);
                    }
                    let mut inherited = None;
                    for base in &interface.extends {
                        let types = self.graph.type_references.get(&EscErrorId {
                            module_id: id.module_id.clone(),
                            node: base.node_id.get(),
                        });
                        for ty in types.into_iter().flatten() {
                            if let Some(value) = self.property_type(ty, name, visiting) {
                                inherited.get_or_insert_with(Value::default).join(value);
                            }
                        }
                    }
                    inherited
                }
                AstKind::TSTypeLiteral(literal) => {
                    self.signature_property(&id.module_id, &result.semantic, &literal.members, name)
                }
                AstKind::TSTypeAliasDeclaration(_) => {
                    let value = self.expand_alias(ty, &mut HashSet::new());
                    let mut property = Value::default();
                    for ty in &value.types {
                        property.join(self.property_type(ty, name, visiting)?);
                    }
                    Some(property)
                }
                _ => None,
            })
        });
        visiting.remove(id);
        value
    }

    fn signature_property(
        &self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        signatures: &[TSSignature<'_>],
        name: &str,
    ) -> Option<Value> {
        signatures.iter().find_map(|signature| {
            let TSSignature::TSPropertySignature(property) = signature else {
                return None;
            };
            if property.computed || property.key.static_name().as_deref() != Some(name) {
                return None;
            }
            let mut value =
                property
                    .type_annotation
                    .as_ref()
                    .map_or_else(Value::unknown, |annotation| {
                        self.annotation_at(
                            module,
                            semantic,
                            &annotation.type_annotation,
                            &mut HashSet::new(),
                        )
                    });
            if property.optional {
                value.join(Value::undefined());
            }
            Some(value)
        })
    }
}
