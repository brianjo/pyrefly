/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests of the `State` object.

use std::mem;
use std::path::PathBuf;
use std::sync::Arc;

use dupe::Dupe;
use starlark_map::small_map::SmallMap;

use crate::config::config::ConfigFile;
use crate::config::error::ErrorConfigs;
use crate::config::finder::ConfigFinder;
use crate::error::error::print_errors;
use crate::metadata::PythonPlatform;
use crate::metadata::PythonVersion;
use crate::metadata::RuntimeMetadata;
use crate::module::module_name::ModuleName;
use crate::module::module_path::ModulePath;
use crate::state::handle::Handle;
use crate::state::loader::LoaderId;
use crate::state::require::Require;
use crate::state::state::State;
use crate::state::subscriber::TestSubscriber;
use crate::test::util::TestEnv;
use crate::test::util::init_test;
use crate::util::arc_id::ArcId;
use crate::util::lock::Mutex;
use crate::util::prelude::SliceExt;

#[test]
fn test_multiple_config() {
    let linux = RuntimeMetadata::new(PythonVersion::default(), PythonPlatform::linux());
    let windows = RuntimeMetadata::new(PythonVersion::default(), PythonPlatform::windows());

    const LIB: &str = r#"
import sys
if sys.platform == "linux":
    value = 42
else:
    value = "hello"
"#;
    let mut test_env = TestEnv::new();
    test_env.add("lib", LIB);
    test_env.add("windows", "import lib; x: str = lib.value");
    test_env.add("linux", "import lib; x: int = lib.value");
    test_env.add(
        "main",
        "import lib; x: str = lib.value  # E: `Literal[42]` is not assignable to `str`",
    );
    let state = State::new(Some(test_env.config_finder()));
    let loader = test_env.loader();

    let f = |name: &str, config: &RuntimeMetadata| {
        let name = ModuleName::from_str(name);
        let path = loader.find_import(name).unwrap();
        (
            Handle::new(name, path, config.dupe(), loader.dupe()),
            Require::Everything,
        )
    };

    let handles = [
        f("linux", &linux),
        f("windows", &windows),
        f("main", &linux),
    ];
    let mut transaction = state.new_transaction(Require::Exports, None);
    transaction.set_memory(test_env.get_memory());
    transaction.run(&handles);
    transaction
        .get_loads(handles.iter().map(|(handle, _)| handle))
        .check_against_expectations(&ErrorConfigs::default())
        .unwrap();
}

#[test]
fn test_multiple_path() {
    const LIB_PYI: &str = "x: int";
    const LIB_PY: &str = "x: str = 1  # E: `Literal[1]` is not assignable to `str`";
    const MAIN_PYI: &str =
        "import lib; y: list[int] = lib.x  # E: `int` is not assignable to `list[int]`";
    const MAIN_PY: &str =
        "import lib; y: list[str] = lib.x  # E: `int` is not assignable to `list[str]`";

    const FILES: &[(&str, &str, &str)] = &[
        ("lib", "lib.pyi", LIB_PYI),
        ("lib", "lib.py", LIB_PY),
        ("main", "main.pyi", MAIN_PYI),
        ("main", "main.py", MAIN_PY),
    ];

    let mut config = ConfigFile::default();
    config.python_environment.set_empty_to_default();
    config.search_path = Vec::new();
    for (name, path, _) in FILES.iter().rev() {
        config.custom.insert(
            ModuleName::from_str(name),
            ModulePath::memory(PathBuf::from(path)),
        );
    }
    config.configure();
    let config = ArcId::new(config);
    let loader = LoaderId::new_arc_id(config.dupe());

    let runtime = RuntimeMetadata::default();

    let state = State::new(Some(ConfigFinder::new_constant(config)));
    let handles = FILES.map(|(name, path, _)| {
        Handle::new(
            ModuleName::from_str(name),
            ModulePath::memory(PathBuf::from(path)),
            runtime.dupe(),
            loader.dupe(),
        )
    });
    let mut transaction = state.new_transaction(Require::Exports, None);
    transaction.set_memory(
        FILES.map(|(_, path, contents)| {
            (PathBuf::from(path), Some(Arc::new((*contents).to_owned())))
        }),
    );
    transaction.run(&handles.map(|x| (x.dupe(), Require::Everything)));
    let loads = transaction.get_loads(handles.iter());
    print_errors(&loads.collect_errors(&ErrorConfigs::default()).shown);
    loads
        .check_against_expectations(&ErrorConfigs::default())
        .unwrap();
    assert_eq!(
        loads.collect_errors(&ErrorConfigs::default()).shown.len(),
        3
    );
}

#[derive(Default, Clone, Dupe, Debug)]
struct IncrementalData(Arc<Mutex<SmallMap<ModuleName, Arc<String>>>>);

/// Helper for writing incrementality tests.
struct Incremental {
    data: IncrementalData,
    loader: LoaderId,
    state: State,
    to_set: Vec<(String, String)>,
}

impl Incremental {
    fn new() -> Self {
        init_test();
        let data = IncrementalData::default();

        let mut config = ConfigFile::default();
        config.python_environment.set_empty_to_default();
        config.search_path = Vec::new();
        for file in ["main", "foo", "bar", "baz"] {
            config.custom.insert(
                ModuleName::from_str(file),
                ModulePath::memory(PathBuf::from(file)),
            );
        }
        config.configure();
        let config = ArcId::new(config);

        Self {
            data: data.dupe(),
            loader: LoaderId::new_arc_id(config.dupe()),
            state: State::new(Some(ConfigFinder::new_constant(config))),
            to_set: Vec::new(),
        }
    }

    /// Change this file to these contents, expecting this number of errors.
    fn set(&mut self, file: &str, contents: &str) {
        self.to_set.push((file.to_owned(), contents.to_owned()));
    }

    fn handle(&self, x: &str) -> Handle {
        Handle::new(
            ModuleName::from_str(x),
            ModulePath::memory(PathBuf::from(x)),
            RuntimeMetadata::default(),
            self.loader.dupe(),
        )
    }

    /// Run a check. Expect recompute things to have changed.
    fn check(&mut self, want: &[&str], recompute: &[&str]) {
        let subscriber = TestSubscriber::new();
        let mut transaction = self
            .state
            .new_committable_transaction(Require::Exports, Some(Box::new(subscriber.dupe())));
        for (file, contents) in mem::take(&mut self.to_set) {
            let contents = Arc::new(contents.to_owned());
            self.data
                .0
                .lock()
                .insert(ModuleName::from_str(&file), contents.dupe());
            transaction
                .as_mut()
                .set_memory(vec![(PathBuf::from(file), Some(contents))]);
        }

        let handles = want.map(|x| self.handle(x));
        self.state.run_with_committing_transaction(
            transaction,
            &handles.map(|x| (x.dupe(), Require::Everything)),
        );
        let loads = self.state.transaction().get_loads(handles.iter());
        print_errors(&loads.collect_errors(&ErrorConfigs::default()).shown);
        loads
            .check_against_expectations(&ErrorConfigs::default())
            .unwrap();

        let mut recompute = recompute.map(|x| (*x).to_owned());
        recompute.sort();

        let mut changed = Vec::new();
        for (x, (count, _)) in subscriber.finish() {
            let m = x.module();
            if self.data.0.lock().contains_key(&m) {
                for _ in 0..count {
                    changed.push(m.as_str().to_owned());
                }
            }
        }
        changed.sort();
        assert_eq!(recompute, changed);
    }
}

#[test]
fn test_in_memory_updated_content_recheck() {
    let mut i = Incremental::new();
    i.set("main", "unbound_name # E:");
    i.check(&["main"], &["main"]);
    i.set("main", "bound_name = 3");
    i.check(&["main"], &["main"]);
}

#[test]
fn test_incremental_minimal_recompute() {
    let mut i = Incremental::new();
    i.set("main", "import foo; x = foo.x");
    i.set("foo", "x = 7");
    i.check(&["main"], &["main", "foo"]);
    i.set("foo", "x = 'test'");
    i.check(&["main"], &["main", "foo"]);
    i.set("foo", "x = 'test' # still");
    i.check(&["main"], &["foo"]);
    i.set("main", "import foo; x = foo.x # still");
    i.check(&["main"], &["main"]);

    // We stop depending on `foo`, so no longer have to recompute it even though it is dirty.
    // However, our current state algorithm does so anyway as it can be cheaper to compute
    // everything than do careful graph traversal.
    i.set("foo", "x = True");
    i.set("main", "x = 7");
    i.check(&["main"], &["main", "foo"]); // `foo` is not required here
    i.set("main", "import foo; x = foo.x # still");
    i.check(&["main"], &["main"]); // `foo` is required by this point
}

#[test]
fn test_incremental_cyclic() {
    let mut i = Incremental::new();
    i.set("foo", "import bar; x = 1; y = bar.x");
    i.set("bar", "import foo; x = True; y = foo.x");
    i.check(&["foo"], &["foo", "bar"]);
    i.set("foo", "import bar; x = 1; y = bar.x # still");
    i.check(&["foo"], &["foo"]);
    i.set("foo", "import bar; x = 'test'; y = bar.x");
    i.check(&["foo"], &["foo", "foo", "bar"]);
}

/// Check that the interface is consistent as we change things.
fn test_interface_consistent(code: &str) {
    let mut i = Incremental::new();
    i.set("main", code);
    i.check(&["main"], &["main"]);
    let base = i
        .state
        .transaction()
        .get_solutions(&i.handle("main"))
        .unwrap();

    i.set("main", &format!("{code} # after"));
    i.check(&["main"], &["main"]);
    let suffix = i
        .state
        .transaction()
        .get_solutions(&i.handle("main"))
        .unwrap();

    i.set("main", &format!("# before\n{code}"));
    i.check(&["main"], &["main"]);
    let prefix = i
        .state
        .transaction()
        .get_solutions(&i.handle("main"))
        .unwrap();

    let same = base.first_difference(&base);
    let suffix = suffix.first_difference(&base);
    let prefix = prefix.first_difference(&base);
    assert!(same.is_none(), "{code:?} led to {same:?}");
    assert!(suffix.is_none(), "{code:?} led to {suffix:?}");
    assert!(prefix.is_none(), "{code:?} led to {prefix:?}");
}

#[test]
fn test_interfaces_simple() {
    test_interface_consistent("x: int = 1\ndef f(y: bool) -> list[str]: return []");

    // Important to have a class with a field, as those also have positions
    test_interface_consistent("class X: y: int");
}

#[test]
fn test_interfaces_generic() {
    // Requires dealing with Forall.
    test_interface_consistent("def f[X](x: X) -> X: ...");
    test_interface_consistent(
        "
from typing import TypeVar, Generic
T = TypeVar('T')
class C(Generic[T]): pass",
    );
    test_interface_consistent("class C[T]: x: T");
}

#[test]
fn test_interfaces_counterexamples() {
    // These all failed at one point or another.

    test_interface_consistent(
        "
from typing import TypeVar, Generic
T = TypeVar('T')
class C(Generic[T]): x: T",
    );

    test_interface_consistent(
        "
from typing import TypeVar, Generic
T = TypeVar('T')
class C(Generic[T]): pass
class D(C[T]): pass",
    );

    test_interface_consistent(
        "
from typing import TypeVar
class C: pass
T = TypeVar('T', bound=C)",
    );

    test_interface_consistent(
        "
class C[R]:
    def __init__(self, field: R) -> None:
        self.field = R
",
    );
}

#[test]
fn test_change_require() {
    let t = TestEnv::one("foo", "x: str = 1");
    let state = State::new(Some(t.config_finder()));
    let handle = Handle::new(
        ModuleName::from_str("foo"),
        ModulePath::memory(PathBuf::from("foo")),
        t.metadata(),
        t.loader(),
    );
    state.run(&[(handle.dupe(), Require::Exports)], Require::Exports, None);
    assert_eq!(
        state
            .transaction()
            .get_loads([&handle])
            .collect_errors(&ErrorConfigs::default())
            .shown
            .len(),
        0
    );
    assert!(state.transaction().get_bindings(&handle).is_none());
    state.run(&[(handle.dupe(), Require::Errors)], Require::Exports, None);
    assert_eq!(
        state
            .transaction()
            .get_loads([&handle])
            .collect_errors(&ErrorConfigs::default())
            .shown
            .len(),
        1
    );
    assert!(state.transaction().get_bindings(&handle).is_none());
    state.run(
        &[(handle.dupe(), Require::Everything)],
        Require::Exports,
        None,
    );
    assert_eq!(
        state
            .transaction()
            .get_loads([&handle])
            .collect_errors(&ErrorConfigs::default())
            .shown
            .len(),
        1
    );
    assert!(state.transaction().get_bindings(&handle).is_some());
}
