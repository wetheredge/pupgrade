use std::borrow::Cow;
use std::fs;
use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};
use sha2::{Digest as _, Sha256};

use crate::dep_collector::{Dep, DepInit, Deps, Updates, Version};

pub(super) struct Manager;

impl super::Manager for Manager {
    fn name(&self) -> &'static str {
        "galock"
    }

    fn walk_directory(&self, path: &Utf8Path) -> bool {
        path.file_name().is_some_and(|dir| dir == ".github")
    }

    fn walk_file(&self, path: &Utf8Path) -> bool {
        path.file_name().is_some_and(|name| name == "galock.toml")
    }

    fn scan_file(&self, _path: &Utf8Path, collector: crate::DepCollector<'_>) {
        #[derive(facet::Facet)]
        struct Action<'a> {
            repo: &'a str,
            tag: &'a str,
            commit: &'a str,
        }

        let json = duct::cmd!("galock", "list", "--json")
            .stdin_null()
            .stderr_null()
            .stdout_capture()
            .read()
            .unwrap();
        let actions: Vec<Action> = facet_json::from_str(&json).unwrap();

        for action in actions {
            collector.push_dep(DepInit {
                path: None,
                kind: None,
                name: action.repo.to_owned(),
                renamed: None,
                version: Version::GitPinnedTag {
                    repo: action.repo.to_owned(),
                    commit: action.commit.to_owned(),
                    tag: action.tag.to_owned(),
                },
            });
        }
    }

    fn find_updates(&self, dep: &Dep) -> Updates {
        let Version::GitPinnedTag { repo, commit, tag } = &dep.version else {
            unreachable!()
        };
        let repo_url = repo.clone();

        let repo = open_repo(&git_url(repo));
        let refs = repo.references().unwrap();
        let tags = refs.tags().unwrap();

        let mut latest = None;
        for mut tag in tags.filter_map(Result::ok) {
            let name: &str = tag.name().shorten().try_into().unwrap();
            let pruned = name.strip_prefix('v').unwrap_or(name);

            if !pruned.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }

            let name = name.to_owned();

            let commit = tag.peel_to_commit().unwrap();
            let commit = hex::encode(commit.id.as_slice());

            latest = Some((commit, name));
        }

        if let Some((latest_commit, latest_tag)) = latest {
            if latest_commit != *commit || latest_tag != *tag {
                return Updates::Found(Version::GitPinnedTag {
                    repo: repo_url.clone(),
                    commit: latest_commit,
                    tag: latest_tag,
                });
            }
        } else {
            todo!()
        }

        Updates::None
    }

    fn apply(&self, _deps: &Deps, _dep: &Dep, version: &Version) {
        let Version::GitPinnedTag { repo, commit, tag } = version else {
            unreachable!()
        };

        duct::cmd!("galock", "set", repo, tag, commit)
            .stdin_null()
            .stderr_null()
            .stdout_null()
            .run()
            .unwrap();
    }
}

fn git_url<'a>(repo: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
    let repo = repo.into();
    if repo.chars().filter(|c| *c == '/').count() == 1 {
        Cow::Owned(format!("https://github.com/{repo}.git"))
    } else {
        repo
    }
}

fn open_repo(url: &str) -> gix::Repository {
    let cache_name = {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        hex::encode(hasher.finalize())
    };

    let cache = dirs::cache_dir().unwrap();
    let cache = Utf8PathBuf::from_path_buf(cache).unwrap();
    let repo_dir = cache
        .join(env!("CARGO_PKG_NAME"))
        .join("git")
        .join(cache_name);

    if let Ok(true) = fs::exists(&repo_dir) {
        let repo = gix::open(repo_dir).unwrap();

        let direction = gix::remote::Direction::Fetch;
        let remote = repo.find_default_remote(direction).unwrap().unwrap();
        let connection = remote.connect(direction).unwrap();
        let fetch = connection
            .prepare_fetch(
                gix::progress::Discard,
                gix::remote::ref_map::Options::default(),
            )
            .unwrap();
        fetch
            .receive(gix::progress::Discard, &AtomicBool::new(false))
            .unwrap();

        repo
    } else {
        fs::create_dir_all(&repo_dir).unwrap();

        gix::prepare_clone_bare(url, repo_dir)
            .unwrap()
            .fetch_only(gix::progress::Discard, &AtomicBool::new(false))
            .unwrap()
            .0
    }
}
