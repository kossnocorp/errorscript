use super::*;
use oxc_syntax::operator::{LogicalOperator, UnaryOperator};

impl Analyzer<'_, '_> {
    pub(super) fn unknown(&self, env: Env) -> Flow {
        Flow::normal(env.clone(), Value::unknown())
            .possible_throw(&env, Types::from([EscErrorType::UNKNOWN]))
    }

    pub(super) fn expr(&self, expression: &Expression<'_>, env: Env) -> Flow {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.expr_inner(expression, env)
        })
    }

    fn expr_inner(&self, expression: &Expression<'_>, env: Env) -> Flow {
        let expression = expression.get_inner_expression();
        match expression {
            Expression::Identifier(id) => self.read(id, env),
            Expression::BooleanLiteral(value) => Flow::normal(env, Value::boolean(value.value)),
            Expression::NullLiteral(_) => Flow::normal(env, Value { truth: 1, ..Value::builtin("null") }),
            Expression::StringLiteral(value) => Flow::normal(env, Value { truth: if value.value.is_empty() { 1 } else { 2 }, ..Value::builtin("string") }),
            Expression::NumericLiteral(value) => Flow::normal(env, Value { truth: if value.value == 0.0 || value.value.is_nan() { 1 } else { 2 }, ..Value::builtin("number") }),
            Expression::BigIntLiteral(_) => Flow::normal(env, Value::builtin("bigint")),
            Expression::RegExpLiteral(_) => Flow::normal(env, Value { truth: 2, ..Value::builtin("RegExp") }),
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => Flow::normal(env, Value { truth: 2, ..Value::builtin("Function") }),
            Expression::ThisExpression(_) | Expression::Super(_) | Expression::ImportMeta(_) => Flow::normal(env, Value::unknown()),
            Expression::CallExpression(call) => self.call_expression(call.node_id.get(), &call.callee, &call.arguments, false, call.optional, env),
            Expression::NewExpression(call) => self.call_expression(call.node_id.get(), &call.callee, &call.arguments, true, false, env),
            Expression::TaggedTemplateExpression(call) => {
                let mut flow = self.expr(&call.tag, env);
                for expression in &call.quasi.expressions { flow = flow.then(|state| self.expr(expression, state.env)); }
                flow.then(|state| self.invoke(call.node_id.get(), false, false, state.env))
            }
            Expression::SequenceExpression(sequence) => sequence.expressions.iter().fold(
                Flow::normal(env, Value::undefined()), |flow, expression| flow.then(|state| self.expr(expression, state.env))),
            Expression::ConditionalExpression(conditional) => self.expr(&conditional.test, env).then(|state| {
                let mut flow = Flow::default();
                if state.value.truth & 2 != 0 && let Some(env) = self.narrow(&conditional.test, state.env.clone(), true) {
                    flow.join(self.expr(&conditional.consequent, env));
                }
                if state.value.truth & 1 != 0 && let Some(env) = self.narrow(&conditional.test, state.env, false) {
                    flow.join(self.expr(&conditional.alternate, env));
                }
                flow
            }),
            Expression::LogicalExpression(logical) => self.expr(&logical.left, env).then(|state| {
                let (right, left) = match logical.operator {
                    LogicalOperator::And => (state.value.truth & 2 != 0, state.value.truth & 1 != 0),
                    LogicalOperator::Or => (state.value.truth & 1 != 0, state.value.truth & 2 != 0),
                    LogicalOperator::Coalesce => (
                        state.value.types.iter().any(|ty| matches!(ty, EscErrorType::Buildin(name) if matches!(name.as_str(), "null" | "undefined" | "unknown"))),
                        state.value.types.iter().any(|ty| !matches!(ty, EscErrorType::Buildin(name) if matches!(name.as_str(), "null" | "undefined"))),
                    ),
                };
                let mut flow = Flow::default();
                if right { flow.join(self.expr(&logical.right, state.env.clone())); }
                if left { flow.join(Flow::normal(state.env, state.value)); }
                flow
            }),
            Expression::AssignmentExpression(assignment) => {
                self.target_effects(&assignment.left, env).then(|state| {
                    let left = if let AssignmentTarget::AssignmentTargetIdentifier(id) = &assignment.left {
                        self.symbol(id).and_then(|symbol| state.env.get(&symbol)).cloned().unwrap_or_else(Value::unknown)
                    } else { Value::unknown() };
                    self.expr(&assignment.right, state.env).then(|state| {
                        let operator = assignment.operator.as_str();
                        if operator == "=" { return self.assign(&assignment.left, state.env, state.value); }
                        if matches!(operator, "&&=" | "||=" | "??=") {
                            return self.assign(&assignment.left, state.env, Value::unknown());
                        }
                        let safe = left.plain_primitive() && state.value.plain_primitive();
                        let mut value = Value::builtin("number");
                        if operator == "+=" && (left.types.contains(&EscErrorType::builtin("string")) || state.value.types.contains(&EscErrorType::builtin("string")) || !safe) {
                            value.join(Value::builtin("string"));
                        }
                        let env = state.env.clone();
                        let flow = self.assign(&assignment.left, state.env, value);
                        if safe { flow } else { flow.possible_throw(&env, Types::from([EscErrorType::UNKNOWN])) }
                    })
                })
            }
            Expression::UpdateExpression(update) => {
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument {
                    self.read(id, env).then(|mut state| {
                        let safe = state.value.plain_primitive();
                        let value = Value::builtin("number");
                        if let Some(symbol) = self.symbol(id) { state.env.insert(symbol, value.clone()); }
                        let flow = Flow::normal(state.env.clone(), value);
                        if safe { flow } else { flow.possible_throw(&state.env, Types::from([EscErrorType::UNKNOWN])) }
                    })
                } else { self.unknown(env) }
            }
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Typeof
                && matches!(unary.argument.get_inner_expression(), Expression::Identifier(id) if self.symbol(id).is_none())
                => Flow::normal(env, Value::builtin("string")),
            Expression::UnaryExpression(unary) => self.expr(&unary.argument, env).then(|state| {
                let value = match unary.operator {
                    UnaryOperator::LogicalNot => Value { truth: ((state.value.truth & 1) << 1) | ((state.value.truth & 2) >> 1), ..Value::builtin("boolean") },
                    UnaryOperator::Typeof => Value { truth: 2, ..Value::builtin("string") },
                    UnaryOperator::Void => Value::undefined(),
                    UnaryOperator::Delete => Value::builtin("boolean"),
                    _ => Value::builtin("number"),
                };
                let flow = Flow::normal(state.env.clone(), value);
                if matches!(unary.operator, UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void)
                    || (unary.operator != UnaryOperator::Delete && state.value.plain_primitive()) { flow }
                else { flow.possible_throw(&state.env, Types::from([EscErrorType::UNKNOWN])) }
            }),
            Expression::BinaryExpression(binary) => self.expr(&binary.left, env).then(|left| {
                self.expr(&binary.right, left.env).then(|right| {
                    let operator = binary.operator.as_str();
                    let value = match operator {
                        "==" | "!=" | "===" | "!==" | "<" | "<=" | ">" | ">=" | "in" | "instanceof" => Value::builtin("boolean"),
                        "+" => {
                            let mut value = Value::builtin("number");
                            if !left.value.plain_primitive() || !right.value.plain_primitive()
                                || left.value.types.contains(&EscErrorType::builtin("string")) || right.value.types.contains(&EscErrorType::builtin("string"))
                            { value.join(Value::builtin("string")); }
                            value
                        },
                        _ => Value::builtin("number"),
                    };
                    let flow = Flow::normal(right.env.clone(), value);
                    let safe = matches!(operator, "===" | "!==")
                        || (!matches!(operator, "in" | "instanceof") && left.value.plain_primitive() && right.value.plain_primitive())
                        || (binary.operator.is_instance_of()
                        && self.graph.type_references.get(&EscErrorId { module_id: self.module.clone(), node: binary.node_id.get() })
                            .is_some_and(|types| !types.contains(&EscErrorType::UNKNOWN)));
                    if safe { flow }
                    else { flow.possible_throw(&right.env, Types::from([EscErrorType::UNKNOWN])) }
                })
            }),
            Expression::AwaitExpression(awaited) => self.expr(&awaited.argument, env).then(|state| {
                let promise = state.value.types.contains(&EscErrorType::builtin("Promise"));
                let unknown = state.value.types.contains(&EscErrorType::UNKNOWN);
                let mut types = state.value.types.clone();
                types.remove(&EscErrorType::builtin("Promise"));
                types.extend(state.value.awaited);
                let mut flow = if types.is_empty() { Flow::default() } else { Flow::normal(state.env.clone(), Value::types(types)) };
                if promise { flow = flow.possible_throw(&state.env, state.value.deferred); }
                if unknown { flow = flow.possible_throw(&state.env, Types::from([EscErrorType::UNKNOWN])); }
                flow
            }),
            Expression::YieldExpression(yielded) => {
                let flow = yielded.argument.as_ref().map_or_else(|| Flow::normal(env.clone(), Value::undefined()), |expr| self.expr(expr, env.clone()));
                flow.then(|state| {
                    let flow = Flow::normal(state.env.clone(), Value::unknown());
                    if yielded.delegate { flow.possible_throw(&state.env, state.value.deferred).possible_throw(&state.env, Types::from([EscErrorType::UNKNOWN])) }
                    else { flow }
                })
            }
            Expression::ObjectExpression(object) => {
                let mut flow = Flow::normal(env, Value::undefined());
                for property in &object.properties {
                    flow = match property {
                        ObjectPropertyKind::ObjectProperty(property) => flow
                            .then(|state| self.property_key(&property.key, property.computed, state.env))
                            .then(|state| self.expr(&property.value, state.env)),
                        ObjectPropertyKind::SpreadProperty(spread) => flow.then(|state| self.expr(&spread.argument, state.env))
                            .then(|state| self.unknown(state.env)),
                    };
                }
                flow.value(Value { truth: 2, ..Value::builtin("object") })
            }
            Expression::ArrayExpression(array) => {
                let mut flow = Flow::normal(env, Value::undefined());
                for element in &array.elements {
                    if let ArrayExpressionElement::SpreadElement(spread) = element {
                        flow = flow.then(|state| self.expr(&spread.argument, state.env)).then(|state| self.unknown(state.env));
                    } else if let Some(expr) = element.as_expression() {
                        flow = flow.then(|state| self.expr(expr, state.env));
                    }
                }
                flow.value(Value { truth: 2, ..Value::builtin("Array") })
            }
            Expression::TemplateLiteral(template) => template.expressions.iter().fold(
                Flow::normal(env, Value::undefined()), |flow, expr| flow.then(|state| self.expr(expr, state.env)))
                .value(Value::builtin("string")),
            Expression::ClassExpression(class) => self.class_definition(class, env),
            Expression::ChainExpression(chain) => {
                let mut flow = match &chain.expression {
                    ChainElement::CallExpression(call) => self.call_expression(call.node_id.get(), &call.callee, &call.arguments, false, call.optional, env.clone()),
                    _ => chain.expression.member_expression().map_or_else(|| self.unknown(env.clone()), |member| self.member(member, env.clone())),
                };
                flow.join(Flow::normal(env, Value::undefined()));
                flow
            }
            _ => expression.as_member_expression().map_or_else(|| self.unknown(env.clone()), |member| self.member(member, env.clone())),
        }
    }

    fn call_expression(
        &self,
        node: NodeId,
        callee: &Expression<'_>,
        arguments: &[Argument<'_>],
        new: bool,
        optional: bool,
        env: Env,
    ) -> Flow {
        let call = self.calls.get(&(self.module.clone(), node));
        let resolved = call.is_some_and(|call| !call.unresolved);
        let mut models = call
            .into_iter()
            .flat_map(|call| &call.global_calls)
            .filter_map(|path| self.graph.global(path))
            .filter_map(|global| if new { global.construct } else { global.call })
            .collect::<Vec<_>>();
        let mut coercion_errors = Types::new();
        let mut flow = if resolved
            && call.is_some_and(|call| call.global_calls.is_empty())
            && let Some(member) = callee.get_inner_expression().as_member_expression()
        {
            // A statically linked namespace access has no arbitrary property getter.
            self.member_operands(member, env.clone())
        } else {
            self.expr(callee, env.clone())
        };
        // Flow-derived values include constructor instances, aliases, and
        // function returns that the syntactic call linker cannot resolve.
        let mut global_targets = HashSet::new();
        let mut only_globals = false;
        flow = flow.then(|state| {
            global_targets = state.value.globals.clone();
            only_globals = !state.value.nonglobal && !global_targets.is_empty();
            for path in &global_targets {
                if let Some(model) = self
                    .graph
                    .global(path)
                    .and_then(|global| if new { global.construct } else { global.call })
                {
                    models.push(model);
                }
            }
            Flow::normal(state.env, state.value)
        });
        let mut argument_values = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            flow = if let Argument::SpreadElement(spread) = argument {
                flow.then(|state| self.expr(&spread.argument, state.env))
                    .then(|state| self.unknown(state.env))
            } else if let Some(expr) = argument.as_expression() {
                flow.then(|state| self.expr(expr, state.env))
            } else {
                flow.then(|state| self.unknown(state.env))
            };
            flow = flow.then(|state| {
                argument_values.push(state.value.clone());
                for model in &models {
                    for rule in model.arguments {
                        if rule.index.is_none_or(|position| position == index)
                            && !state.value.types.iter().all(|ty| matches!(ty, EscErrorType::Buildin(name) if rule.safe_types.contains(&name.as_str())))
                        { coercion_errors.extend(rule.errors.iter().cloned()); }
                    }
                }
                Flow::normal(state.env, state.value)
            });
        }
        flow = flow.then(|state| {
            if let Some(call) = call {
                self.arguments.record(
                    call,
                    &argument_values,
                    arguments
                        .iter()
                        .any(|argument| matches!(argument, Argument::SpreadElement(_))),
                );
            }
            if !global_targets.is_empty() {
                let mut result = Flow::default();
                for path in &global_targets {
                    let Some(global) = self.graph.global(path) else {
                        result.join(self.unknown(state.env.clone()));
                        continue;
                    };
                    if let Some(model) = if new { global.construct } else { global.call } {
                        if arguments.len() < model.min_arguments {
                            result = result.possible_throw(
                                &state.env,
                                model.missing_arguments_errors.iter().cloned().collect(),
                            );
                        } else {
                            result.join(
                                Flow::normal(state.env.clone(), Value::builtin(model.returns))
                                    .possible_throw(
                                        &state.env,
                                        model.errors.iter().cloned().collect(),
                                    ),
                            );
                        }
                    } else {
                        result.join(Flow::one(
                            Completion::Throw,
                            state.env.clone(),
                            Value::builtin("TypeError"),
                        ));
                    }
                }
                if !only_globals {
                    result.join(self.invoke(
                        node,
                        new,
                        matches!(callee, Expression::Super(_)),
                        state.env.clone(),
                    ));
                }
                return result.possible_throw(&state.env, coercion_errors);
            }
            if models
                .iter()
                .any(|model| arguments.len() < model.min_arguments)
            {
                let errors: Types = models
                    .iter()
                    .filter(|model| arguments.len() < model.min_arguments)
                    .flat_map(|model| model.missing_arguments_errors.iter().cloned())
                    .collect();
                Flow::one(Completion::Throw, state.env, Value::types(errors))
            } else {
                self.invoke(
                    node,
                    new,
                    matches!(callee, Expression::Super(_)),
                    state.env.clone(),
                )
                .possible_throw(&state.env, coercion_errors)
            }
        });
        if optional {
            flow.join(Flow::normal(env, Value::undefined()));
        }
        flow
    }

    fn invoke(&self, node: NodeId, new: bool, super_call: bool, mut env: Env) -> Flow {
        let Some(call) = self.calls.get(&(self.module.clone(), node)) else {
            return self.unknown(env);
        };
        // Function summaries don't yet carry writes to captured bindings.
        // Widen potentially mutated values instead of preserving stale types.
        if call.unresolved || !call.targets.is_empty() {
            self.bindings.widen(&mut env);
        }
        if !new
            && !super_call
            && !call.constructed_types.is_empty()
            && call
                .constructed_types
                .iter()
                .all(|ty| matches!(ty, EscErrorType::Node(_)))
        {
            let mut flow = Flow::one(Completion::Throw, env.clone(), Value::builtin("TypeError"));
            if call.unresolved || !call.targets.is_empty() {
                flow.join(self.unknown(env));
            }
            return flow;
        }
        let mut flow = Flow::default();
        for path in &call.global_calls {
            let Some(global) = self.graph.global(path) else {
                flow.join(self.unknown(env.clone()));
                continue;
            };
            if let Some(model) = if new { global.construct } else { global.call } {
                flow.join(
                    Flow::normal(env.clone(), Value::builtin(model.returns))
                        .possible_throw(&env, model.errors.iter().cloned().collect()),
                );
            } else {
                flow.join(Flow::one(
                    Completion::Throw,
                    env.clone(),
                    Value::builtin("TypeError"),
                ));
            }
        }
        if call.unresolved {
            flow.join(self.unknown(env.clone()));
        }
        if call.targets.is_empty()
            && !call.constructed_types.is_empty()
            && call.global_calls.is_empty()
        {
            flow.join(Flow::normal(
                env.clone(),
                Value {
                    truth: 2,
                    ..Value::types(call.constructed_types.clone())
                },
            ));
        }
        for target in &call.targets {
            let summary = self.summaries.get(target);
            let function = &self.graph.graph[target.node()];
            if function.is_async || function.is_generator {
                if new {
                    flow.join(Flow::one(
                        Completion::Throw,
                        env.clone(),
                        Value::builtin("TypeError"),
                    ));
                    continue;
                }
                flow.join(Flow::normal(
                    env.clone(),
                    Value {
                        deferred: summary.errors.clone(),
                        awaited: summary.returned.types.clone(),
                        truth: 2,
                        ..Value::builtin(if function.is_async {
                            "Promise"
                        } else {
                            "Generator"
                        })
                    },
                ));
            } else {
                flow = flow.possible_throw(&env, summary.errors.clone());
                if summary.completes {
                    let value = if new || super_call {
                        let fallback = if call.constructed_types.is_empty() {
                            Types::from([EscErrorType::Node(EscErrorId {
                                module_id: function.module_id.clone(),
                                node: function.node_id,
                            })])
                        } else {
                            Types::from(call.constructed_types.clone())
                        };
                        let mut value = summary.returned.clone();
                        let mut uses_instance = false;
                        value.types.retain(|ty| {
                            if matches!(ty, EscErrorType::Buildin(name) if matches!(name.as_str(), "undefined" | "null" | "number" | "string" | "boolean" | "bigint" | "symbol")) {
                                uses_instance = true;
                                false
                            } else {
                                uses_instance |= *ty == EscErrorType::UNKNOWN;
                                true
                            }
                        });
                        if uses_instance {
                            value.types.extend(fallback);
                        }
                        value.truth = 2;
                        value
                    } else {
                        summary.returned.clone()
                    };
                    flow.join(Flow::normal(env.clone(), value));
                }
            }
        }
        flow
    }

    fn member_operands(&self, member: &MemberExpression<'_>, env: Env) -> Flow {
        let flow = self.expr(member.object(), env);
        if let MemberExpression::ComputedMemberExpression(member) = member {
            flow.then(|state| self.expr(&member.expression, state.env))
        } else {
            flow
        }
    }

    fn member(&self, member: &MemberExpression<'_>, env: Env) -> Flow {
        self.expr(member.object(), env).then(|object| {
            let flow = if let MemberExpression::ComputedMemberExpression(member) = member {
                self.expr(&member.expression, object.env)
            } else {
                Flow::normal(object.env, Value::undefined())
            };
            flow.then(|state| {
                let Some(name) = member.static_property_name() else {
                    if !state.value.types.is_empty()
                        && state
                            .value
                            .types
                            .iter()
                            .all(|ty| *ty == EscErrorType::builtin("number"))
                    {
                        let (mut value, errors) = self.property(&object.value, "[index]");
                        value.join(Value::undefined());
                        return Flow::normal(state.env.clone(), value)
                            .possible_throw(&state.env, errors);
                    }
                    return self.unknown(state.env);
                };
                let (value, errors) = self.property(&object.value, name);
                Flow::normal(state.env.clone(), value).possible_throw(&state.env, errors)
            })
        })
    }

    fn target_effects(&self, target: &AssignmentTarget<'_>, env: Env) -> Flow {
        if let Some(member) = target.as_member_expression() {
            self.member_operands(member, env)
        } else {
            Flow::normal(env, Value::undefined())
        }
    }

    pub(super) fn assign(&self, target: &AssignmentTarget<'_>, mut env: Env, value: Value) -> Flow {
        if let AssignmentTarget::AssignmentTargetIdentifier(id) = target {
            if let Some(symbol) = self.symbol(id) {
                env.insert(symbol, value.clone());
            }
            Flow::normal(env, value)
        } else {
            self.unknown(env).value(value)
        }
    }

    pub(super) fn property_key(&self, key: &PropertyKey<'_>, computed: bool, env: Env) -> Flow {
        if computed && let Some(expr) = key.as_expression() {
            self.expr(expr, env)
        } else {
            Flow::normal(env, Value::undefined())
        }
    }

    pub(super) fn class_definition(&self, class: &Class<'_>, env: Env) -> Flow {
        if class.declare {
            return Flow::normal(env, Value::undefined());
        }
        let mut flow = Flow::normal(env, Value::undefined());
        if let Some(heritage) = &class.heritage {
            flow = flow.then(|state| self.expr(&heritage.expression, state.env));
            let ty = EscErrorType::Node(EscErrorId {
                module_id: self.module.clone(),
                node: class.node_id.get(),
            });
            if self
                .graph
                .super_types
                .get(&ty)
                .is_none_or(|types| types.contains(&EscErrorType::UNKNOWN))
            {
                flow = flow.then(|state| self.unknown(state.env));
            }
        }
        for decorator in &class.decorators {
            flow = flow
                .then(|state| self.expr(&decorator.expression, state.env))
                .then(|state| self.unknown(state.env));
        }
        for element in &class.body.body {
            flow = match element {
                ClassElement::StaticBlock(block) => {
                    flow.then(|state| self.statements(&block.body, state.env))
                }
                ClassElement::MethodDefinition(method) => {
                    let mut flow = flow
                        .then(|state| self.property_key(&method.key, method.computed, state.env));
                    for decorator in &method.decorators {
                        flow = flow
                            .then(|state| self.expr(&decorator.expression, state.env))
                            .then(|state| self.unknown(state.env));
                    }
                    flow
                }
                ClassElement::PropertyDefinition(field) => {
                    let mut flow =
                        flow.then(|state| self.property_key(&field.key, field.computed, state.env));
                    for decorator in &field.decorators {
                        flow = flow
                            .then(|state| self.expr(&decorator.expression, state.env))
                            .then(|state| self.unknown(state.env));
                    }
                    if field.r#static
                        && let Some(expr) = &field.value
                    {
                        flow.then(|state| self.expr(expr, state.env))
                    } else {
                        flow
                    }
                }
                ClassElement::AccessorProperty(_) => flow.then(|state| self.unknown(state.env)),
                _ => flow,
            };
        }
        flow.value(Value {
            truth: 2,
            ..Value::builtin("Function")
        })
    }
}
