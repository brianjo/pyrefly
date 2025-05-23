/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use dupe::Dupe;

use crate::config::config::ConfigFile;
use crate::module::module_name::ModuleName;
use crate::module::module_path::ModulePath;
use crate::util::arc_id::ArcId;
use crate::util::display::commas_iter;
use crate::util::locked_map::LockedMap;

#[derive(Debug, Clone, Dupe)]
pub enum FindError {
    /// This module could not be found, and we should emit an error
    NotFound(Arc<anyhow::Error>),
    /// This import could not be found, but the user configured it to be ignored
    Ignored,
    /// This site package path entry was found, but does not have a py.typed entry
    /// and ignore_py_typed_package_errors is disabled
    NoPyTyped,
}

impl FindError {
    pub const NO_PY_TYPED_ERROR_MESSAGE: &'static str = "Imported package does not contain a py.typed file, \
        and therefore cannot be typed. See `use_untyped_imports` to import anyway.";

    pub fn not_found(err: anyhow::Error) -> Self {
        Self::NotFound(Arc::new(err))
    }

    pub fn search_path(search_roots: &[PathBuf], site_package_path: &[PathBuf]) -> FindError {
        if search_roots.is_empty() && site_package_path.is_empty() {
            Self::not_found(anyhow!("no search roots or site package path"))
        } else {
            Self::not_found(anyhow!(
                "looked at search roots ({}) and site package path ({})",
                commas_iter(|| search_roots.iter().map(|x| x.display())),
                commas_iter(|| site_package_path.iter().map(|x| x.display())),
            ))
        }
    }

    pub fn display(err: Arc<anyhow::Error>, module: ModuleName) -> String {
        format!("Could not find import of `{module}`, {:#}", err)
    }
}

#[derive(Clone, Dupe, Debug, Hash, PartialEq, Eq)]
pub struct LoaderId(ArcId<ConfigFile>);

impl LoaderId {
    pub fn new(loader: ConfigFile) -> Self {
        Self(ArcId::new(loader))
    }

    pub fn new_arc_id(loader: ArcId<ConfigFile>) -> Self {
        Self(loader)
    }

    pub fn find_import(&self, module: ModuleName) -> Result<ModulePath, FindError> {
        self.0.find_import(module)
    }
}

#[derive(Debug)]
pub struct LoaderFindCache {
    loader: LoaderId,
    cache: LockedMap<ModuleName, Result<ModulePath, FindError>>,
}

impl LoaderFindCache {
    pub fn new(loader: LoaderId) -> Self {
        Self {
            loader,
            cache: Default::default(),
        }
    }

    pub fn find_import(&self, module: ModuleName) -> Result<ModulePath, FindError> {
        self.cache
            .ensure(&module, || self.loader.find_import(module))
            .dupe()
    }
}
