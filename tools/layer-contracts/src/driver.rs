//! Compiler-backed collector, pinned to toolchain.txt. No source-text matching.
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use layer_contracts::{Edge, Graph};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::{AmbigArg, Expr, ExprKind, HirId, Path, Ty, TyKind};
use rustc_interface::interface;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, TyCtxt};
use rustc_session::impl_lint_pass;
use rustc_span::Span;
use std::sync::{Arc, Mutex};

fn full_name(tcx: TyCtxt<'_>, id: DefId) -> String {
    let name = tcx.crate_name(id.krate).to_string();
    let path = tcx.def_path_str(id);
    if path.is_empty() {
        name
    } else {
        format!("{name}::{path}")
    }
}

fn module_of(tcx: TyCtxt<'_>, mut id: DefId) -> String {
    while tcx.def_kind(id) != DefKind::Mod {
        id = tcx.parent(id);
    }
    full_name(tcx, id)
}

struct Collector {
    graph: Arc<Mutex<Graph>>,
}

impl_lint_pass!(Collector => []);

impl Collector {
    fn reference(
        &self,
        cx: &LateContext<'_>,
        owner: HirId,
        target: DefId,
        span: Span,
        kind: &str,
    ) {
        if !target.is_local() {
            return;
        }
        let from = module_of(cx.tcx, owner.owner.def_id.to_def_id());
        let to = module_of(cx.tcx, target);
        if from == to {
            return;
        }
        let location = cx
            .tcx
            .sess
            .source_map()
            .span_to_diagnostic_string(span.source_callsite());
        self.graph
            .lock()
            .expect("collector lock")
            .edges
            .insert(Edge {
                from,
                to,
                symbol: full_name(cx.tcx, target),
                location,
                kind: kind.into(),
            });
    }

    fn resolved(&self, cx: &LateContext<'_>, owner: HirId, res: Res, span: Span, kind: &str) {
        if let Res::Def(_, target) = res {
            self.reference(cx, owner, target, span, kind);
        }
    }

    // Not call-graph devirtualization: generic/dynamic dispatch stays abstract.
    fn inferred(&self, cx: &LateContext<'_>, expr: &Expr<'_>, ty: ty::Ty<'_>, depth: usize) {
        if depth > 64 {
            panic!("Unsupported type nesting beyond 64; refusing partial graph");
        }
        match ty.kind() {
            ty::Adt(def, args) => {
                self.reference(cx, expr.hir_id, def.did(), expr.span, "inferred-type");
                for arg in args.iter() {
                    if let Some(ty) = arg.as_type() {
                        self.inferred(cx, expr, ty, depth + 1);
                    }
                }
            }
            ty::Ref(_, inner, _) | ty::RawPtr(inner, _) | ty::Slice(inner) | ty::Array(inner, _) => {
                self.inferred(cx, expr, *inner, depth + 1);
            }
            ty::Tuple(fields) => {
                for ty in fields.iter() {
                    self.inferred(cx, expr, ty, depth + 1);
                }
            }
            ty::FnDef(def, args) => {
                self.reference(cx, expr.hir_id, *def, expr.span, "function-item");
                for arg in args.iter() {
                    if let Some(ty) = arg.as_type() {
                        self.inferred(cx, expr, ty, depth + 1);
                    }
                }
            }
            ty::Alias(_, alias) => {
                self.reference(cx, expr.hir_id, alias.def_id, expr.span, "type-alias");
            }
            _ => {}
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Collector {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        let mut graph = self.graph.lock().expect("collector lock");
        graph.crate_name = cx.tcx.crate_name(LOCAL_CRATE).to_string();
        let root = graph.crate_name.clone();
        graph.modules.insert(root);
    }

    fn check_mod(&mut self, cx: &LateContext<'tcx>, _: &'tcx rustc_hir::Mod<'tcx>, id: HirId) {
        self.graph
            .lock()
            .expect("collector lock")
            .modules
            .insert(module_of(cx.tcx, id.owner.def_id.to_def_id()));
    }

    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &Path<'tcx>, id: HirId) {
        self.resolved(cx, id, path.res, path.span, "path");
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        if let TyKind::Path(ref path) = ty.kind {
            self.resolved(cx, ty.hir_id, cx.qpath_res(path, ty.hir_id), ty.span, "type-path");
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(ref path) = expr.kind {
            self.resolved(
                cx,
                expr.hir_id,
                cx.qpath_res(path, expr.hir_id),
                expr.span,
                "expression-path",
            );
        }
        if let ExprKind::MethodCall(..) = expr.kind {
            if let Some(def) = cx.typeck_results().type_dependent_def_id(expr.hir_id) {
                self.reference(cx, expr.hir_id, def, expr.span, "method");
            }
        }
        self.inferred(cx, expr, cx.typeck_results().expr_ty(expr), 0);
    }
}

#[derive(Default)]
struct Driver {
    graph: Arc<Mutex<Graph>>,
    completed: bool,
}

impl Callbacks for Driver {
    fn config(&mut self, config: &mut interface::Config) {
        let graph = self.graph.clone();
        config.register_lints = Some(Box::new(move |_, store| {
            let graph = graph.clone();
            store.register_late_pass(move |_| {
                Box::new(Collector {
                    graph: graph.clone(),
                })
            });
        }));
    }

    fn after_analysis<'tcx>(&mut self, _: &interface::Compiler, _: TyCtxt<'tcx>) -> Compilation {
        self.completed = true;
        Compilation::Stop
    }
}

fn main() {
    let output = std::env::var_os("LAYER_GRAPH_OUT")
        .expect("Set LAYER_GRAPH_OUT to a new graph path");
    let args: Vec<String> = std::env::args().collect();
    let mut driver = Driver::default();
    rustc_driver::run_compiler(&args, &mut driver);
    assert!(driver.completed, "Compiler did not complete analysis");
    let graph = driver.graph.lock().expect("collector lock");
    graph.validate().expect("Incomplete compiler graph");
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .expect("Graph output must be new and writable");
    file.write_all(graph.encode().as_bytes()).expect("Write graph");
}
