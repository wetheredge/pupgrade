use facet::Facet;

use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;
use std::sync::Mutex;

pub(crate) struct DepCollector<'a> {
    data: &'a DepsBuilder,
    manager: usize,
}

pub(crate) struct DepsBuilder {
    groups: Mutex<Vec<Group>>,
    deps: boxcar::Vec<Dep>,
    lockfiles: boxcar::Vec<Lockfile>,
}

#[derive(Facet)]
pub(crate) struct Deps {
    groups: Vec<Group>,
    deps: Vec<Dep>,
    lockfiles: Vec<Lockfile>,
}

#[derive(Facet)]
struct Group {
    name: String,
    #[facet(skip_serializing_if = is_default, default)]
    format: GroupFormat,
    #[facet(skip_serializing_if = is_default, default)]
    parent: Option<usize>,
    #[facet(skip_serializing, default)]
    locked: bool,
}

#[derive(Facet, Default, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum GroupFormat {
    #[default]
    Plain,
    Path,
    Code,
}

#[derive(Facet)]
pub(crate) struct Dep {
    pub(crate) manager: usize,
    #[facet(skip_serializing_if = is_default, default)]
    pub(crate) skip: bool,
    group: usize,

    pub(crate) name: String,
    #[facet(skip_serializing_if = is_default, default)]
    pub(crate) renamed: Option<String>,
    pub(crate) version: Version,
    pub(crate) updates: Option<Version>,
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

#[derive(Debug, Facet, thiserror::Error)]
#[error("The group exists but is locked")]
pub(crate) struct LockedGroupError;

#[derive(Debug, Facet, thiserror::Error)]
#[error("A group with this ID already exists")]
pub(crate) struct GroupExistsError;

/// Reference to a [`Group`] from a [`DepsBuilder`]
pub(crate) struct GroupHandle<'a> {
    collector: &'a DepCollector<'a>,
    index: usize,
}

/// Reference to a [`Group`] from a finalized instance of [`Deps`].
pub(crate) struct GroupRef<'a> {
    data: &'a Deps,
    index: usize,
}

pub(crate) struct GroupIter<'a> {
    data: &'a Deps,
    parent: Option<usize>,
    cursor: Option<usize>,
}

impl Deps {
    pub(crate) fn serialize(self) -> String {
        facet_json::to_string(&Deps::from(self))
    }

    pub(crate) fn deserialize(s: &str) -> Result<Self, facet_json::DeserError<'_>> {
        facet_json::from_str::<Deps>(s).map(Self::from)
    }

    pub(crate) fn iter_root_groups(&self) -> GroupIter<'_> {
        GroupIter {
            data: self,
            parent: None,
            cursor: Some(0),
        }
    }

    pub(crate) fn get_dependency_mut(&mut self, id: usize) -> &mut Dep {
        &mut self.deps[id]
    }

    pub(crate) fn iter_dependencies<'a>(&'a self) -> impl Iterator<Item = &'a Dep> + use<'a> {
        self.deps.iter()
    }
}

impl DepsBuilder {
    pub(crate) fn new() -> Self {
        Self {
            groups: Mutex::new(Vec::new()),
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
    fn get_group<'a>(
        &'a self,
        parent: Option<usize>,
        name: &str,
    ) -> Option<Result<GroupHandle<'a>, LockedGroupError>> {
        let mut groups = self.data.groups.lock().unwrap();

        for (index, group) in groups.iter_mut().enumerate() {
            if group.parent == parent && group.name == name {
                if group.locked {
                    return Some(Err(LockedGroupError));
                }

                group.locked = true;
                return Some(Ok(GroupHandle {
                    collector: self,
                    index,
                }));
            }
        }

        None
    }

    pub(crate) fn get_or_push_group<'a>(
        &'a self,
        id: Cow<'_, str>,
        format: GroupFormat,
    ) -> Result<GroupHandle<'a>, LockedGroupError> {
        if let Some(group) = self.get_group(None, &id) {
            group
        } else {
            let mut groups = self.data.groups.lock().unwrap();

            let index = groups.len();
            groups.push(Group {
                name: id.into_owned(),
                format,
                parent: None,
                locked: true,
            });
            Ok(GroupHandle {
                collector: self,
                index,
            })
        }
    }
}

impl<'a> GroupHandle<'a> {
    pub(crate) fn full_id<T>(&self, f: impl FnOnce(&[&str]) -> T) -> T {
        let groups = self.collector.data.groups.lock().unwrap();

        let mut id = Vec::new();
        let mut current = Some(self.index);
        while let Some(index) = current {
            let group = &groups[index];
            id.push(group.name.as_str());
            current = group.parent;
        }

        f(&id)
    }

    pub(crate) fn get_group(&self, id: &str) -> Option<Result<GroupHandle<'a>, LockedGroupError>> {
        self.collector.get_group(Some(self.index), id)
    }

    pub(crate) fn new_subgroup(
        &self,
        name: String,
        format: GroupFormat,
    ) -> Result<GroupHandle<'a>, GroupExistsError> {
        let mut groups = self.collector.data.groups.lock().unwrap();

        let exists = groups
            .iter()
            .any(|group| group.parent == Some(self.index) && group.name == name);

        if exists {
            Err(GroupExistsError)
        } else {
            let index = groups.len();
            groups.push(Group {
                name,
                format,
                parent: Some(self.index),
                locked: true,
            });
            Ok(GroupHandle {
                collector: self.collector,
                index,
            })
        }
    }

    pub(crate) fn push_dep(&self, name: String, renamed: Option<String>, version: Version) {
        self.collector.data.deps.push(Dep {
            manager: self.collector.manager,
            skip: false,
            group: self.index,
            name,
            renamed,
            version,
            updates: None,
        });
    }
}

impl Drop for GroupHandle<'_> {
    fn drop(&mut self) {
        if let Ok(mut groups) = self.collector.data.groups.lock() {
            groups[self.index].locked = false;
        }
    }
}

impl<'a> GroupRef<'a> {
    pub(crate) fn name(&self) -> &str {
        &self.data.groups[self.index].name
    }

    pub(crate) fn format(&self) -> GroupFormat {
        self.data.groups[self.index].format
    }

    pub(crate) fn iter_subgroups(&self) -> GroupIter<'a> {
        GroupIter {
            data: self.data,
            parent: Some(self.index),
            cursor: Some(self.index + 1),
        }
    }

    pub(crate) fn iter_dependencies(&self) -> impl Iterator<Item = &'a Dep> + use<'a> {
        let group = self.index;
        self.data.deps.iter().filter(move |dep| dep.group == group)
    }
}

impl<'a> Iterator for GroupIter<'a> {
    type Item = GroupRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let cursor = self.cursor.as_mut()?;

        if let Some(index) = self
            .data
            .groups
            .iter()
            .skip(*cursor)
            .position(|group| group.parent == self.parent)
            .map(|i| i + *cursor)
        {
            *cursor = index + 1;
            Some(GroupRef {
                data: self.data,
                index,
            })
        } else {
            self.cursor = None;
            None
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
    fn from(plain: DepsBuilder) -> Self {
        let groups = plain.groups.into_inner().unwrap();

        let mut group_is_empty = vec![true; groups.len()];
        for (_, dep) in &plain.deps {
            let mut group = Some(dep.group);
            while let Some(id) = group.take() {
                if group_is_empty[id] {
                    // Haven't been to this group before, so continue upwards
                    group = groups[id].parent;
                }

                group_is_empty[id] = false;
            }
        }
        let mut used_groups = Vec::with_capacity(groups.len());
        let mut id_map = Vec::with_capacity(groups.len());
        let mut new_id = 0;
        for (group, is_empty) in groups.into_iter().zip(group_is_empty.into_iter()) {
            // Always push because the used id's need to match up. Empty ids won't get read anyway
            id_map.push(new_id);
            if !is_empty {
                used_groups.push(Group {
                    parent: group.parent.map(|old| id_map[old]),
                    ..group
                });
                new_id += 1;
            }
        }

        let deps = plain.deps.into_iter().map(|mut dep| {
            dep.group = id_map[dep.group];
            dep
        });

        Self {
            groups: used_groups,
            deps: deps.collect(),
            lockfiles: plain.lockfiles.into_iter().collect(),
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
