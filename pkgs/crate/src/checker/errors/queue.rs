//! Dependency-ready SCC queue. Workers own their AST arenas; only owned values
//! and versioned summaries cross threads. Recursive SCCs retain one summary per
//! function and are solved locally to a fixed point.
use super::*;
use oxc_span::SourceType;
use std::{
    collections::VecDeque,
    rc::Rc,
    sync::{
        Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::task::JoinSet;

type Sources = Arc<HashMap<EscModuleId, (Arc<String>, SourceType)>>;
type Store = Arc<RwLock<HashMap<EscFnId, (Summary, u64)>>>;
// Large bundles contain thousands of independent SCCs. Two exclusively owned
// arenas let them overlap without replicating the bundle across every worker.
const MODULE_CONCURRENCY: usize = 2;
type ModulePool = Arc<Mutex<HashMap<EscModuleId, Vec<EscModule>>>>;

/// Lazily parsed, worker-local modules. Rc never leaves the blocking worker.
/// Reentrant cross-module type reads clone the Rc before entering the AST.
pub(super) struct Modules {
    sources: Sources,
    pool: ModulePool,
    cache: RefCell<HashMap<EscModuleId, Rc<EscModule>>>,
}

impl Modules {
    pub(super) fn get(&self, id: &EscModuleId) -> Option<Rc<EscModule>> {
        if let Some(module) = self.cache.borrow().get(id) {
            return Some(module.clone());
        }
        let (source, source_type) = self.sources.get(id)?;
        let pooled = self.pool.lock().unwrap().get_mut(id).and_then(Vec::pop);
        let module = Rc::new(
            pooled.unwrap_or_else(|| EscModule::from_analysis_source(source, *source_type)),
        );
        self.cache.borrow_mut().insert(id.clone(), module.clone());
        Some(module)
    }
}

pub(super) struct Summaries {
    store: Store,
    local: RefCell<HashMap<EscFnId, Summary>>,
    reads: RefCell<HashMap<EscFnId, u64>>,
    local_read: std::cell::Cell<bool>,
}

impl Summaries {
    pub(super) fn get(&self, id: &EscFnId) -> Summary {
        if let Some(summary) = self.local.borrow().get(id) {
            self.local_read.set(true);
            return summary.clone();
        }
        let (summary, version) = self.store.read().unwrap()[id].clone();
        self.reads
            .borrow_mut()
            .entry(id.clone())
            .and_modify(|first| *first = (*first).min(version))
            .or_insert(version);
        summary
    }
}

struct Job {
    component: usize,
    arguments: HashMap<EscFnId, Vec<Value>>,
}

struct Output {
    component: usize,
    summaries: HashMap<EscFnId, Summary>,
    arguments: HashMap<EscFnId, Vec<Value>>,
    reads: HashMap<EscFnId, u64>,
    inputs: HashMap<EscFnId, Vec<Value>>,
}

#[derive(Default)]
struct Worker {
    preferred: Option<EscModuleId>,
}

#[derive(Default)]
struct Ready {
    modules: HashMap<EscModuleId, VecDeque<usize>>,
    order: VecDeque<EscModuleId>,
    // Affinity pops can drain a module without consuming its fallback marker.
    // Keep that marker unique when newly ready work refills the module queue.
    queued: HashSet<EscModuleId>,
}

impl Ready {
    fn batch(&mut self, first: usize, graph: &EscCallGraph) -> Vec<usize> {
        const BATCH_SIZE: usize = 16;
        let mut batch = vec![first];
        let module = graph.sccs[first][0].module_id();
        // Multi-module recursive components keep their own ownership reservation.
        if graph.sccs[first].iter().any(|id| id.module_id() != module) {
            return batch;
        }
        if let Some(pending) = self.modules.get_mut(module) {
            while batch.len() < BATCH_SIZE
                && pending.front().is_some_and(|component| {
                    graph.sccs[*component]
                        .iter()
                        .all(|id| id.module_id() == module)
                })
            {
                batch.push(pending.pop_front().unwrap());
            }
            if pending.is_empty() {
                self.modules.remove(module);
            }
        }
        batch
    }

    fn push(&mut self, component: usize, graph: &EscCallGraph) {
        let module = graph.sccs[component][0].module_id().clone();
        let pending = self.modules.entry(module.clone()).or_default();
        if self.queued.insert(module.clone()) {
            self.order.push_back(module);
        }
        pending.push_back(component);
    }

    fn pop(
        &mut self,
        preferred: Option<&EscModuleId>,
        active: &HashMap<EscModuleId, usize>,
        graph: &EscCallGraph,
    ) -> Option<usize> {
        let available = |component: &usize| {
            graph.sccs[*component].iter().all(|id| {
                active.get(id.module_id()).copied().unwrap_or_default() < MODULE_CONCURRENCY
            })
        };
        if let Some(module) = preferred
            && let Some(pending) = self.modules.get_mut(module)
            && pending.front().is_some_and(available)
        {
            let result = pending.pop_front();
            if pending.is_empty() {
                self.modules.remove(module);
            }
            return result;
        }
        // Inspect each queued module at most once; occupied primary arenas are
        // released on completion. Other modules can run concurrently.
        for _ in 0..self.order.len() {
            let module = self.order.pop_front().unwrap();
            if let Some(pending) = self.modules.get_mut(&module) {
                if !pending.front().is_some_and(available) {
                    self.order.push_back(module);
                    continue;
                }
                let result = pending.pop_front();
                if pending.is_empty() {
                    self.modules.remove(&module);
                    self.queued.remove(&module);
                } else {
                    self.order.push_front(module);
                }
                return result;
            }
            self.queued.remove(&module);
        }
        None
    }
}

struct Shared {
    sources: Sources,
    pool: ModulePool,
    graph: Arc<EscCallGraph>,
    store: Store,
    counts: Arc<HashMap<EscFnId, usize>>,
    calls: HashMap<(EscModuleId, NodeId), EscCall>,
    cancelled: Arc<AtomicBool>,
}

struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn work(job: Job, shared: Arc<Shared>) -> Result<(Worker, Output)> {
    let graph = &shared.graph;
    let store = &shared.store;
    let modules = Modules {
        sources: shared.sources.clone(),
        pool: shared.pool.clone(),
        cache: RefCell::new(HashMap::new()),
    };
    let component = &graph.sccs[job.component];
    let recursive = component.len() > 1
        || graph
            .graph
            .find_edge(component[0].node(), component[0].node())
            .is_some();
    let summaries = Summaries {
        store: store.clone(),
        local: RefCell::new({
            let stored = store.read().unwrap();
            component
                .iter()
                .map(|id| (id.clone(), stored[id].0.clone()))
                .collect()
        }),
        reads: RefCell::new(HashMap::new()),
        local_read: std::cell::Cell::new(false),
    };
    let arguments = arguments::Arguments {
        counts: shared.counts.clone(),
        values: RefCell::new(job.arguments),
    };
    loop {
        let previous_arguments = arguments.snapshot();
        let mut changed = false;
        for id in component {
            ensure!(
                !shared.cancelled.load(Ordering::Relaxed),
                "Checking cancelled"
            );
            if matches!(arguments.parameters(id), Some(None)) {
                continue;
            }
            let function = &graph.graph[id.node()];
            let module = modules.get(&function.module_id).unwrap();
            let next = module.with_semantic(|result| {
                Analyzer {
                    mutation_cache: RefCell::new(HashMap::new()),
                    modules: &modules,
                    module: &function.module_id,
                    semantic: &result.semantic,
                    graph,
                    calls: &shared.calls,
                    summaries: &summaries,
                    reading: RefCell::new(HashSet::new()),
                    arguments: &arguments,
                }
                .function(function, id)
            });
            let mut local = summaries.local.borrow_mut();
            let current = local.get_mut(id).unwrap();
            let previous = current.clone();
            current.join(next);
            changed |= *current != previous;
        }
        if (!recursive && !summaries.local_read.get())
            || (!changed && previous_arguments == arguments.snapshot())
        {
            break;
        }
    }
    let inputs = component
        .iter()
        .filter_map(|id| {
            arguments
                .values
                .borrow()
                .get(id)
                .map(|values| (id.clone(), values.clone()))
        })
        .collect();
    let output = Output {
        component: job.component,
        summaries: summaries.local.into_inner(),
        reads: summaries.reads.into_inner(),
        arguments: arguments.values.into_inner(),
        inputs,
    };
    // Transfer exclusive ownership back to the shared pool. Concurrent type
    // queries may make auxiliary copies; retain a bounded number per module.
    // Destruction of duplicate arenas happens outside the pool lock.
    let mut discarded = Vec::new();
    {
        let mut pool = shared.pool.lock().unwrap();
        for (id, module) in modules.cache.into_inner() {
            let module = Rc::try_unwrap(module).expect("Analysis module borrow escaped its job");
            let copies = pool.entry(id).or_default();
            if copies.len() < MODULE_CONCURRENCY {
                copies.push(module);
            } else {
                discarded.push(module);
            }
        }
    }
    drop(discarded);
    let worker = Worker {
        preferred: Some(component[0].module_id().clone()),
    };
    Ok((worker, output))
}

pub(crate) async fn resolve_errors(
    parsed: &EscProjectStateParsed,
    graph: &EscCallGraph,
) -> Result<HashMap<EscFnId, Types>> {
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    resolve_with_workers(parsed, graph, workers).await
}

pub(super) async fn resolve_with_workers(
    parsed: &EscProjectStateParsed,
    graph: &EscCallGraph,
    workers: usize,
) -> Result<HashMap<EscFnId, Types>> {
    let count = graph.sccs.len();
    if count == 0 {
        return Ok(HashMap::new());
    }
    let worker_count = workers.max(1).min(count);
    let sources: Sources = Arc::new(
        parsed
            .parsed_files
            .iter()
            .map(|(id, module)| (id.clone(), module.analysis_source()))
            .collect(),
    );
    let arguments = arguments::Arguments::new(parsed, graph);
    let graph = Arc::new(graph.clone());
    let store: Store = Arc::new(RwLock::new(
        graph
            .sccs
            .iter()
            .flatten()
            .map(|id| (id.clone(), (Summary::default(), 0)))
            .collect(),
    ));
    let mut dependencies = vec![HashSet::new(); count];
    let mut dependents = vec![HashSet::new(); count];
    let mut readers: HashMap<EscFnId, HashSet<usize>> = HashMap::new();
    for call in &graph.calls {
        let Some(caller) = &call.caller else {
            continue;
        };
        let caller = graph.component_of[caller];
        for target in &call.targets {
            let callee = graph.component_of[target];
            if caller != callee {
                dependencies[caller].insert(callee);
                dependents[callee].insert(caller);
                readers.entry(target.clone()).or_default().insert(caller);
            }
        }
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel_on_drop = CancelOnDrop(cancelled.clone());
    let shared = Arc::new(Shared {
        sources,
        pool: Arc::new(Mutex::new(HashMap::new())),
        graph: graph.clone(),
        store: store.clone(),
        counts: arguments.counts.clone(),
        calls: graph
            .calls
            .iter()
            .map(|call| {
                (
                    (call.site.module_id.clone(), call.site.node_id),
                    call.clone(),
                )
            })
            .collect(),
        cancelled,
    });
    let mut tasks = JoinSet::new();
    let mut idle = (0..worker_count)
        .map(|_| Worker::default())
        .collect::<Vec<_>>();
    let mut dirty = (0..count).collect::<HashSet<_>>();
    let mut active_modules = HashMap::<EscModuleId, usize>::new();
    let mut observed_reads = vec![HashMap::new(); count];
    let mut observed_inputs = vec![HashMap::new(); count];
    let mut remaining = vec![0; count];
    while !dirty.is_empty() {
        // Late fixed-point rounds often contain only a handful of components.
        // Rebuild readiness for those components, not the entire project.
        let mut pending = dirty.iter().copied().collect::<Vec<_>>();
        pending.sort_unstable();
        let mut ready = Ready::default();
        for component in pending {
            remaining[component] = dependencies[component].intersection(&dirty).count();
            if remaining[component] == 0 {
                ready.push(component, &graph);
            }
        }
        let mut next_dirty = HashSet::new();
        let mut completed = 0;
        while completed < dirty.len() {
            while let Some(worker) = idle.pop() {
                let Some(component) = ready.pop(worker.preferred.as_ref(), &active_modules, &graph)
                else {
                    idle.push(worker);
                    break;
                };
                let modules = graph.sccs[component]
                    .iter()
                    .map(EscFnId::module_id)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                for module in &modules {
                    *active_modules.entry(module.clone()).or_default() += 1;
                }
                let values = arguments.values.borrow();
                let jobs = ready
                    .batch(component, &graph)
                    .into_iter()
                    .map(|component| {
                        let inputs = graph.sccs[component]
                            .iter()
                            .filter_map(|id| {
                                values.get(id).map(|values| (id.clone(), values.clone()))
                            })
                            .collect();
                        Job {
                            component,
                            arguments: inputs,
                        }
                    })
                    .collect::<Vec<_>>();
                let shared = shared.clone();
                // Batch only already-ready SCCs. Each retains its own local
                // fixed point, read versions, argument facts, and summary.
                tasks.spawn_blocking(move || -> Result<_> {
                    let mut results = Vec::with_capacity(jobs.len());
                    let mut worker = Worker::default();
                    for job in jobs {
                        let (last, result) = work(job, shared.clone())?;
                        worker = last;
                        results.push(result);
                    }
                    Ok((worker, results, modules))
                });
            }
            ensure!(
                idle.len() < worker_count,
                "Function dependency queue stalled"
            );
            let (worker, results, modules) = tasks
                .join_next()
                .await
                .context("No checking jobs remain")?
                .context("Checking worker failed")??;
            idle.push(worker);
            for module in modules {
                *active_modules.get_mut(&module).unwrap() -= 1;
            }
            for result in results {
                completed += 1;
                // Merge monotonically; never replace a newer fact with a stale job's
                // snapshot. Read versions also cover dependencies absent from the
                // call graph, such as calls in captured variable initializers.
                {
                    let mut stored = store.write().unwrap();
                    for (id, version) in &result.reads {
                        readers
                            .entry(id.clone())
                            .or_default()
                            .insert(result.component);
                        if stored[id].1 != *version {
                            next_dirty.insert(result.component);
                        }
                    }
                    for (id, summary) in result.summaries {
                        let (current, version) = stored.get_mut(&id).unwrap();
                        let previous = current.clone();
                        current.join(summary);
                        if *current != previous {
                            *version += 1;
                            next_dirty.extend(readers.get(&id).into_iter().flatten().copied());
                        }
                    }
                }
                observed_reads[result.component] = result.reads;
                observed_inputs[result.component] = result.inputs;
                {
                    let mut values = arguments.values.borrow_mut();
                    for (id, incoming) in result.arguments {
                        let previous = values.get(&id).cloned();
                        let current = values
                            .entry(id.clone())
                            .or_insert_with(|| vec![Value::default(); incoming.len()]);
                        for (current, incoming) in current.iter_mut().zip(incoming) {
                            current.join(incoming);
                        }
                        if previous.as_ref() != Some(current) {
                            next_dirty.insert(graph.component_of[&id]);
                        }
                    }
                }
                for &dependent in &dependents[result.component] {
                    if dirty.contains(&dependent) {
                        remaining[dependent] -= 1;
                        if remaining[dependent] == 0 {
                            ready.push(dependent, &graph);
                        }
                    }
                }
            }
        }
        // A dependent may already have run after its producer completed during
        // this round. Requeue only if a fact it actually consumed is now stale.
        {
            let stored = store.read().unwrap();
            let inputs = arguments.values.borrow();
            next_dirty.retain(|component| {
                observed_reads[*component]
                    .iter()
                    .any(|(id, version)| stored[id].1 != *version)
                    || graph.sccs[*component]
                        .iter()
                        .any(|id| inputs.get(id) != observed_inputs[*component].get(id))
            });
        }
        dirty = next_dirty;
    }
    let result = store
        .read()
        .unwrap()
        .iter()
        .map(|(id, (summary, _))| (id.clone(), summary.errors.clone()))
        .collect();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affinity_requeue_keeps_one_fallback_entry_and_releases_blocked_work() {
        let dir = tempfile::tempdir().unwrap();
        let repo = EscRepoPath::try_new(dir.path().to_path_buf()).unwrap();
        let mut graph = EscCallGraph::default();
        for name in ["a.ts", "b.ts"] {
            let path = dir.path().join(name);
            std::fs::write(&path, "").unwrap();
            let module =
                EscModuleId::from_path(&EscModulePath::try_new(path).unwrap(), &repo).unwrap();
            graph.sccs.push(vec![EscFnId::new(
                module,
                petgraph::graph::NodeIndex::new(graph.sccs.len()),
            )]);
        }
        let a = graph.sccs[0][0].module_id();
        let b = graph.sccs[1][0].module_id();
        let mut ready = Ready::default();
        let empty = HashMap::new();
        for _ in 0..1000 {
            ready.push(0, &graph);
            assert_eq!(ready.pop(Some(a), &empty, &graph), Some(0));
        }
        assert_eq!(ready.order.len(), 1);
        ready.push(0, &graph);
        ready.push(1, &graph);
        let active = HashMap::from([(a.clone(), MODULE_CONCURRENCY)]);
        assert_eq!(ready.pop(Some(a), &active, &graph), Some(1));
        assert_eq!(ready.pop(Some(b), &active, &graph), None);
        assert_eq!(ready.pop(None, &empty, &graph), Some(0));
        assert!(ready.order.is_empty());
        assert!(ready.queued.is_empty());

        // A batch must not consume a cross-module SCC without reserving its
        // other arena, or grow without bound on a large generated bundle.
        let a = a.clone();
        let b = b.clone();
        for _ in 0..20 {
            graph.sccs.push(vec![EscFnId::new(
                a.clone(),
                petgraph::graph::NodeIndex::new(graph.sccs.len()),
            )]);
        }
        let cross = graph.sccs.len();
        graph.sccs.push(vec![
            EscFnId::new(a.clone(), petgraph::graph::NodeIndex::new(cross)),
            EscFnId::new(b, petgraph::graph::NodeIndex::new(cross + 1)),
        ]);
        for component in 2..=cross {
            ready.push(component, &graph);
        }
        let first = ready.pop(Some(&a), &empty, &graph).unwrap();
        assert_eq!(ready.batch(first, &graph), (2..18).collect::<Vec<_>>());
        let first = ready.pop(Some(&a), &empty, &graph).unwrap();
        assert_eq!(ready.batch(first, &graph), (18..cross).collect::<Vec<_>>());
        let first = ready.pop(Some(&a), &empty, &graph).unwrap();
        assert_eq!(ready.batch(first, &graph), vec![cross]);
        assert_eq!(ready.pop(None, &empty, &graph), None);
    }
}
