use camino::{Utf8Path, Utf8PathBuf};
use facet::Facet;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Copy)]
pub(crate) struct DepCollector<'a> {
    data: &'a DepsBuilder,
    manager: usize,
}

pub(crate) struct DepsBuilder {
    paths: boxcar::Vec<Utf8PathBuf>,
    kinds: Mutex<HashMap<String, (usize, String)>>,
    deps: boxcar::Vec<Dep>,
    lockfiles: boxcar::Vec<Lockfile>,
}

#[derive(Facet)]
pub(crate) struct Deps {
    paths: Vec<Utf8PathBuf>,
    kinds: Vec<(String, String)>,
    deps: Vec<Dep>,
    lockfiles: Vec<Lockfile>,
}

#[derive(Facet)]
pub(crate) struct Dep {
    pub(crate) manager: usize,
    pub(crate) path: Option<usize>,
    pub(crate) kind: Option<usize>,
    #[facet(skip_serializing_if = is_default, default)]
    pub(crate) skip: bool,

    pub(crate) name: String,
    #[facet(skip_serializing_if = is_default, default)]
    pub(crate) renamed: Option<String>,
    pub(crate) version: Version,
    pub(crate) updates: Option<Version>,
}

pub(crate) struct DepInit {
    pub(crate) path: Option<usize>,
    pub(crate) kind: Option<usize>,
    pub(crate) name: String,
    pub(crate) renamed: Option<String>,
    pub(crate) version: Version,
}

#[derive(Facet)]
#[repr(u8)]
#[expect(unused)]
pub(crate) enum Version {
    SemVer(String),
    GitCommit {
        repo: String,
        commit: String,
    },
    GitPinnedTag {
        repo: String,
        commit: String,
        tag: String,
    },
}

#[derive(Facet)]
struct Lockfile {
    manager: usize,
    path: PathBuf,
}

impl Deps {
    pub(crate) fn serialize(self) -> String {
        facet_json::to_string(&self)
    }

    pub(crate) fn deserialize(s: &str) -> Result<Self, facet_json::DeserError<'_>> {
        facet_json::from_str::<Deps>(s)
    }

    pub(crate) fn deps(&self) -> &[Dep] {
        &self.deps
    }

    pub(crate) fn path(&self, id: usize) -> &Utf8Path {
        &self.paths[id]
    }

    pub(crate) fn kind(&self, id: usize) -> &str {
        &self.kinds[id].1
    }
}

impl DepsBuilder {
    pub(crate) fn new() -> Self {
        Self {
            paths: boxcar::Vec::new(),
            kinds: Mutex::new(HashMap::new()),
            deps: boxcar::Vec::new(),
            lockfiles: boxcar::Vec::new(),
        }
    }

    pub(crate) fn collector(&self, manager: usize) -> DepCollector<'_> {
        DepCollector {
            data: self,
            manager,
        }
    }
}

impl DepCollector<'_> {
    pub(crate) fn push_path(&self, path: Utf8PathBuf) -> usize {
        self.data.paths.push(path)
    }

    pub(crate) fn get_kind_id(&self, internal: String, display: impl FnOnce() -> String) -> usize {
        let mut kinds = self.data.kinds.lock().unwrap();
        let next_id = kinds.len();
        kinds
            .entry(internal)
            .or_insert_with(|| (next_id, display()))
            .0
    }

    pub(crate) fn push_dep(&self, init: DepInit) {
        self.data.deps.push(Dep {
            manager: self.manager,
            path: init.path,
            kind: init.kind,
            skip: false,
            name: init.name,
            renamed: init.renamed,
            version: init.version,
            updates: None,
        });
    }
}

impl Version {
    pub(crate) fn commit(&self) -> Option<&str> {
        match self {
            Self::SemVer(_) => None,
            Self::GitCommit { commit, .. } => Some(commit),
            Self::GitPinnedTag { commit, .. } => Some(commit),
        }
    }

    pub(crate) fn repo(&self) -> Option<&str> {
        match self {
            Self::SemVer(_) => None,
            Self::GitCommit { repo, .. } => Some(repo),
            Self::GitPinnedTag { repo, .. } => Some(repo),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemVer(semver) => f.write_str(semver),
            Self::GitCommit { .. } => todo!(),
            Self::GitPinnedTag { .. } => todo!(),
        }
    }
}

impl From<DepsBuilder> for Deps {
    fn from(builder: DepsBuilder) -> Self {
        let kinds = builder.kinds.lock().unwrap();
        let mut kinds = kinds.iter().collect::<Vec<_>>();
        kinds.sort_unstable_by_key::<usize, _>(|(_, (id, _))| *id);
        let kinds = kinds
            .into_iter()
            .map(|(internal, (_, display))| (internal.clone(), display.clone()))
            .collect();

        Self {
            paths: builder.paths.into_iter().collect(),
            kinds,
            deps: builder.deps.into_iter().collect(),
            lockfiles: builder.lockfiles.into_iter().collect(),
        }
    }
}

fn is_default<T>(x: &T) -> bool
where
    T: Default,
    for<'a> &'a T: Eq,
{
    x == &T::default()
}
