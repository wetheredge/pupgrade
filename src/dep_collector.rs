use camino::{Utf8Path, Utf8PathBuf};
use facet::Facet;

use std::collections::HashMap;
use std::fmt;
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
}

#[derive(Facet)]
pub(crate) struct Deps {
    paths: Vec<Utf8PathBuf>,
    kinds: Vec<(String, String)>,
    deps: Vec<Dep>,
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
    #[facet(skip_serializing_if = Updates::is_none, default)]
    pub(crate) updates: Updates,
}

#[derive(Facet, Default)]
#[repr(u8)]
pub(crate) enum Updates {
    #[default]
    None,
    Failed,
    Found(Version),
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
pub(crate) struct UpdatePair<'a> {
    old: &'a Version,
    new: &'a Updates,
}

impl Deps {
    pub(crate) fn serialize(self) -> String {
        facet_json::to_string(&self)
    }

    pub(crate) fn deserialize(s: &str) -> Result<Self, facet_json::JsonError> {
        facet_json::from_str::<Deps>(s)
    }

    pub(crate) fn deps(&self) -> &[Dep] {
        &self.deps
    }

    pub(crate) fn deps_mut(&mut self) -> &mut [Dep] {
        &mut self.deps
    }

    pub(crate) fn path(&self, id: usize) -> &Utf8Path {
        &self.paths[id]
    }

    pub(crate) fn internal_kind(&self, id: usize) -> &str {
        &self.kinds[id].0
    }

    pub(crate) fn kind(&self, id: usize) -> &str {
        &self.kinds[id].1
    }

    pub(crate) fn dep_mut(&mut self, id: usize) -> &mut Dep {
        &mut self.deps[id]
    }
}

impl DepsBuilder {
    pub(crate) fn new() -> Self {
        Self {
            paths: boxcar::Vec::new(),
            kinds: Mutex::new(HashMap::new()),
            deps: boxcar::Vec::new(),
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.deps.count()
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
            updates: Updates::None,
        });
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let write_commit = |f: &mut fmt::Formatter, hash: &str| {
            let hash = if hash.is_ascii() && hash.len() >= 8 {
                &hash[0..8]
            } else {
                hash
            };

            if f.alternate() {
                write!(f, "`{hash}`")
            } else {
                f.write_str(hash)
            }
        };

        match self {
            Version::SemVer(semver) => f.write_str(semver),
            Version::GitCommit { commit, .. } => write_commit(f, commit),
            Version::GitPinnedTag { commit, tag, .. } => {
                f.write_str(tag)?;
                f.write_str(" @ ")?;
                write_commit(f, commit)
            }
        }
    }
}

impl Updates {
    /// Returns `true` if `self` is [`None`].
    ///
    /// [`None`]: Updates::None
    #[must_use]
    pub(crate) fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns `true` if the updates is [`Found`].
    ///
    /// [`Found`]: Updates::Found
    #[must_use]
    pub(crate) fn is_found(&self) -> bool {
        matches!(self, Self::Found(..))
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
