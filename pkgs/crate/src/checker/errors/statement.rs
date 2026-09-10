use super::*;

impl Analyzer<'_, '_> {
    pub(super) fn statements(&self, statements: &[Statement<'_>], env: Env) -> Flow {
        statements
            .iter()
            .fold(Flow::normal(env, Value::undefined()), |flow, statement| {
                flow.then(|state| self.statement(statement, state.env, None))
            })
    }

    fn statement(&self, statement: &Statement<'_>, env: Env, label: Option<&str>) -> Flow {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.statement_inner(statement, env, label)
        })
    }

    fn statement_inner(&self, statement: &Statement<'_>, env: Env, label: Option<&str>) -> Flow {
        match statement {
            Statement::BlockStatement(block) => self.statements(&block.body, env),
            Statement::ExpressionStatement(statement) => self.expr(&statement.expression, env).value(Value::undefined()),
            Statement::VariableDeclaration(declaration) => self.declarations(declaration, env),
            Statement::ThrowStatement(statement) => self.expr(&statement.argument, env).then(|state| {
                if state.value.types.is_empty() { Flow::default() }
                else { Flow::one(Completion::Throw, state.env, Value::types(state.value.types)) }
            }),
            Statement::ReturnStatement(statement) => {
                let flow = statement.argument.as_ref().map_or_else(
                    || Flow::normal(env.clone(), Value::undefined()), |expr| self.expr(expr, env.clone()));
                flow.then(|state| Flow::one(Completion::Return, state.env, state.value))
            }
            Statement::IfStatement(statement) => self.expr(&statement.test, env).then(|state| {
                let mut result = Flow::default();
                if state.value.truth & 2 != 0 && let Some(env) = self.narrow(&statement.test, state.env.clone(), true) {
                    result.join(self.statement(&statement.consequent, env, None));
                }
                if state.value.truth & 1 != 0 && let Some(env) = self.narrow(&statement.test, state.env, false) {
                    result.join(statement.alternate.as_ref().map_or_else(
                        || Flow::normal(env.clone(), Value::undefined()), |statement| self.statement(statement, env.clone(), None)));
                }
                result
            }),
            Statement::TryStatement(statement) => {
                let mut flow = self.statements(&statement.block.body, env);
                if let Some(handler) = &statement.handler
                    && let Some(thrown) = flow.take(&Completion::Throw)
                {
                    let caught = if let Some(param) = &handler.param {
                        self.bind(&param.pattern, thrown.env, Value::types(thrown.value.types))
                    } else { Flow::normal(thrown.env, Value::undefined()) };
                    flow.join(caught.then(|state| self.statements(&handler.body.body, state.env)));
                }
                if let Some(finalizer) = &statement.finalizer {
                    let mut result = Flow::default();
                    for (pending, state) in flow.0 {
                        let mut final_flow = self.statements(&finalizer.body, state.env);
                        if let Some(normal) = final_flow.take(&Completion::Normal) {
                            result.add(pending, State { env: normal.env, value: state.value });
                        }
                        result.join(final_flow);
                    }
                    result
                } else { flow }
            }
            Statement::BreakStatement(statement) => Flow::one(
                Completion::Break(statement.label.as_ref().map(|label| label.name.to_string())), env, Value::undefined()),
            Statement::ContinueStatement(statement) => Flow::one(
                Completion::Continue(statement.label.as_ref().map(|label| label.name.to_string())), env, Value::undefined()),
            Statement::LabeledStatement(statement) => {
                let name = statement.label.name.as_str();
                let mut flow = self.statement(&statement.body, env, Some(name));
                if let Some(state) = flow.take(&Completion::Break(Some(name.to_string()))) {
                    flow.add(Completion::Normal, state);
                }
                flow
            }
            Statement::WhileStatement(statement) => self.loop_body(env, Some(&statement.test), None, &statement.body, false, false, label),
            Statement::DoWhileStatement(statement) => self.loop_body(env, Some(&statement.test), None, &statement.body, true, false, label),
            Statement::ForStatement(statement) => {
                let flow = match &statement.init {
                    Some(ForStatementInit::VariableDeclaration(declaration)) => self.declarations(declaration, env),
                    Some(init) => init.as_expression().map_or_else(|| self.unknown(env.clone()), |expr| self.expr(expr, env.clone())),
                    None => Flow::normal(env, Value::undefined()),
                };
                flow.then(|state| self.loop_body(state.env, statement.test.as_ref(), statement.update.as_ref(), &statement.body, false, false, label))
            }
            Statement::ForOfStatement(statement) => self.expr(&statement.right, env).then(|state| {
                let mut errors = state.value.deferred;
                if !state.value.types.iter().all(|ty| matches!(ty, EscErrorType::Buildin(name) if matches!(name.as_str(), "Array" | "string" | "Generator"))) {
                    errors.insert(EscErrorType::UNKNOWN);
                }
                let element = if state.value.elements.is_empty() { Value::unknown() } else { Value::types(state.value.elements) };
                self.for_left(&statement.left, state.env.clone(), element).then(|state| {
                    self.loop_body(state.env, None, None, &statement.body, false, true, label)
                }).possible_throw(&state.env, errors)
            }),
            Statement::ForInStatement(statement) => self.expr(&statement.right, env).then(|state| {
                self.for_left(&statement.left, state.env.clone(), Value::builtin("string")).then(|state| {
                    self.loop_body(state.env, None, None, &statement.body, false, true, label)
                }).possible_throw(&state.env, HashSet::from([EscErrorType::UNKNOWN]))
            }),
            Statement::SwitchStatement(statement) => self.expr(&statement.discriminant, env).then(|state| {
                let mut result = Flow::default();
                for (start, case) in statement.cases.iter().enumerate() {
                    let mut branch = Flow::normal(state.env.clone(), Value::undefined());
                    let test_count = if case.test.is_none() { statement.cases.len() } else { start + 1 };
                    for case in statement.cases.iter().take(test_count) {
                        if let Some(test) = &case.test { branch = branch.then(|state| self.expr(test, state.env)); }
                    }
                    for case in statement.cases.iter().skip(start) {
                        branch = branch.then(|state| self.statements(&case.consequent, state.env));
                    }
                    if let Some(state) = branch.take(&Completion::Break(None)) { branch.add(Completion::Normal, state); }
                    result.join(branch);
                }
                if statement.cases.iter().all(|case| case.test.is_some()) {
                    let mut unmatched = Flow::normal(state.env, Value::undefined());
                    for case in &statement.cases {
                        if let Some(test) = &case.test { unmatched = unmatched.then(|state| self.expr(test, state.env)); }
                    }
                    result.join(unmatched);
                }
                result
            }),
            Statement::ClassDeclaration(class) => self.class_definition(class, env),
            Statement::WithStatement(statement) => self.expr(&statement.object, env).then(|state| {
                self.statement(&statement.body, state.env.clone(), None)
                    .possible_throw(&state.env, HashSet::from([EscErrorType::UNKNOWN]))
            }),
            Statement::FunctionDeclaration(_) | Statement::EmptyStatement(_) | Statement::DebuggerStatement(_)
            | Statement::TSTypeAliasDeclaration(_) | Statement::TSInterfaceDeclaration(_) => Flow::normal(env, Value::undefined()),
            _ => self.unknown(env),
        }
    }

    pub(super) fn declarations(&self, declaration: &VariableDeclaration<'_>, env: Env) -> Flow {
        if declaration.declare {
            return Flow::normal(env, Value::undefined());
        }
        declaration.declarations.iter().fold(
            Flow::normal(env, Value::undefined()),
            |flow, declaration| {
                flow.then(|state| {
                    let init = declaration.init.as_ref().map_or_else(
                        || Flow::normal(state.env.clone(), Value::undefined()),
                        |expr| self.expr(expr, state.env.clone()),
                    );
                    init.then(|state| self.bind(&declaration.id, state.env, state.value))
                })
            },
        )
    }

    pub(super) fn bind(&self, pattern: &BindingPattern<'_>, mut env: Env, value: Value) -> Flow {
        match pattern {
            BindingPattern::BindingIdentifier(id) => {
                if let Some(symbol) = id.symbol_id.get() {
                    env.insert(symbol, value);
                }
                Flow::normal(env, Value::undefined())
            }
            BindingPattern::AssignmentPattern(pattern) => {
                let mut flow = self.bind(&pattern.left, env.clone(), value);
                flow.join(
                    self.expr(&pattern.right, env)
                        .then(|state| self.bind(&pattern.left, state.env, state.value)),
                );
                flow
            }
            BindingPattern::ObjectPattern(pattern) => {
                let mut flow = self.unknown(env);
                for property in &pattern.properties {
                    flow = flow
                        .then(|state| {
                            self.property_key(&property.key, property.computed, state.env)
                        })
                        .then(|state| self.bind(&property.value, state.env, Value::unknown()));
                }
                if let Some(rest) = &pattern.rest {
                    flow =
                        flow.then(|state| self.bind(&rest.argument, state.env, Value::unknown()));
                }
                flow
            }
            BindingPattern::ArrayPattern(pattern) => {
                let mut flow = self.unknown(env);
                for pattern in pattern.elements.iter().flatten() {
                    flow = flow.then(|state| self.bind(pattern, state.env, Value::unknown()));
                }
                if let Some(rest) = &pattern.rest {
                    flow =
                        flow.then(|state| self.bind(&rest.argument, state.env, Value::unknown()));
                }
                flow
            }
        }
    }

    fn for_left(&self, left: &ForStatementLeft<'_>, env: Env, value: Value) -> Flow {
        if let ForStatementLeft::VariableDeclaration(declaration) = left {
            declaration.declarations.iter().fold(
                Flow::normal(env, Value::undefined()),
                |flow, declaration| {
                    flow.then(|state| self.bind(&declaration.id, state.env, value.clone()))
                },
            )
        } else if let Some(target) = left.as_assignment_target() {
            self.assign(target, env, value)
        } else {
            self.unknown(env)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn loop_body(
        &self,
        env: Env,
        test: Option<&Expression<'_>>,
        update: Option<&Expression<'_>>,
        body: &Statement<'_>,
        do_first: bool,
        optional: bool,
        label: Option<&str>,
    ) -> Flow {
        let mut head = env;
        let mut result = Flow::default();
        loop {
            let before = head.clone();
            let condition = if do_first {
                Flow::normal(head.clone(), Value::boolean(true))
            } else {
                test.map_or_else(
                    || {
                        Flow::normal(
                            head.clone(),
                            if optional {
                                Value::builtin("boolean")
                            } else {
                                Value::boolean(true)
                            },
                        )
                    },
                    |expr| self.expr(expr, head.clone()),
                )
            };
            let mut iteration = condition.then(|state| {
                let mut flow = Flow::default();
                if state.value.truth & 1 != 0 {
                    result.add(
                        Completion::Normal,
                        State {
                            env: state.env.clone(),
                            value: Value::undefined(),
                        },
                    );
                }
                if state.value.truth & 2 != 0 {
                    flow.join(self.statement(body, state.env, None));
                }
                flow
            });
            let mut back = iteration.take(&Completion::Normal);
            for target in [None, label.map(str::to_owned)] {
                if let Some(state) = iteration.take(&Completion::Break(target.clone())) {
                    result.add(Completion::Normal, state);
                }
                if let Some(state) = iteration.take(&Completion::Continue(target)) {
                    if let Some(back) = &mut back {
                        join_env(&mut back.env, state.env);
                    } else {
                        back = Some(state);
                    }
                }
            }
            result.join(iteration);
            let Some(back) = back else {
                break;
            };
            let mut flow = update.map_or_else(
                || Flow::normal(back.env.clone(), Value::undefined()),
                |expr| self.expr(expr, back.env.clone()),
            );
            if do_first {
                flow = flow
                    .then(|state| self.expr(test.unwrap(), state.env))
                    .then(|state| {
                        if state.value.truth & 1 != 0 {
                            result.add(
                                Completion::Normal,
                                State {
                                    env: state.env.clone(),
                                    value: Value::undefined(),
                                },
                            );
                        }
                        if state.value.truth & 2 != 0 {
                            Flow::normal(state.env, Value::undefined())
                        } else {
                            Flow::default()
                        }
                    });
            }
            let next = flow.take(&Completion::Normal);
            result.join(flow);
            let Some(next) = next else {
                break;
            };
            join_env(&mut head, next.env);
            if head == before {
                break;
            }
        }
        result
    }

    pub(super) fn narrow(
        &self,
        test: &Expression<'_>,
        mut env: Env,
        positive: bool,
    ) -> Option<Env> {
        let test = test.get_inner_expression();
        if let Expression::UnaryExpression(unary) = test
            && unary.operator.is_not()
        {
            return self.narrow(&unary.argument, env, !positive);
        }
        if let Expression::BinaryExpression(binary) = test
            && binary.operator.is_instance_of()
            && let Expression::Identifier(id) = binary.left.get_inner_expression()
            && let Some(symbol) = self.symbol(id)
            && let Some(value) = env.get_mut(&symbol)
            && let Some(bases) = self.graph.type_references.get(&EscErrorId {
                module_id: self.module.clone(),
                node: binary.node_id.get(),
            })
        {
            value.types.retain(|ty| {
                let answers = bases
                    .iter()
                    .map(|base| self.subtype(ty, base, &mut HashSet::new()))
                    .collect::<Vec<_>>();
                if answers.contains(&None) {
                    true
                } else if positive {
                    answers.contains(&Some(true))
                } else {
                    answers.contains(&Some(false))
                }
            });
            if value.types.is_empty() {
                return None;
            }
        }
        Some(env)
    }
}
