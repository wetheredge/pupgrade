use std::collections::HashMap;
use std::fmt::{self, Write as _};

use dialoguer::{FuzzySelect, MultiSelect, Select};

use crate::dep_collector::{Dep, Updates};

pub(crate) fn run(state: &mut crate::Deps) -> anyhow::Result<()> {
    let theme = dialoguer::theme::ColorfulTheme::default();

    let mut updateable = HashMap::new();
    for (id, dep) in state.deps().iter().enumerate() {
        if dep.updates.is_found() {
            updateable
                .entry(&dep.name)
                .or_insert_with(Vec::new)
                .push(id);
        }
    }
    let mut updateable = updateable
        .into_iter()
        .map(|(name, ids)| (name.clone(), ids))
        .collect::<Vec<_>>();
    updateable.sort_by(|(a, _), (b, _)| a.cmp(b));
    let updateable = updateable;

    loop {
        let id = FuzzySelect::with_theme(&theme)
            .with_prompt("Select an update to modify or <escape> to finish")
            .items(updateable.iter().map(|(name, _)| name))
            .report(false)
            .interact_opt()?;

        let Some(id) = id else {
            break;
        };

        let ids = &updateable[id].1;
        let mut _selected;
        let ids = if ids.len() == 1 {
            ids.as_slice()
        } else {
            let items = ids.iter().map(|id| (DisplayFullDep::new(state, *id), true));

            _selected = MultiSelect::with_theme(&theme)
                .with_prompt("Select which update(s) to edit")
                .items_checked(items)
                .report(false)
                .interact()?;
            for id in _selected.iter_mut() {
                *id = ids[*id];
            }
            &_selected
        };

        let mut prompt = String::new();
        for (i, id) in ids.iter().enumerate() {
            if i != 0 {
                prompt.push_str("\n  ");
            }
            write!(prompt, "{}", DisplayFullDep::new(state, *id)).unwrap();
        }

        let actions = Action::ALL;
        let action = Select::with_theme(&theme)
            .with_prompt(prompt)
            .items(actions)
            .default(0)
            .interact()?;

        for id in ids {
            let dep = state.dep_mut(*id);
            match actions[action] {
                Action::Update => dep.skip = false,
                Action::Skip => dep.skip = true,
            }
        }
    }

    Ok(())
}

struct DisplayFullDep<'a> {
    state: &'a crate::Deps,
    dep: &'a Dep,
}

impl<'a> DisplayFullDep<'a> {
    fn new(state: &'a crate::Deps, dep: usize) -> Self {
        Self {
            state,
            dep: &state.deps()[dep],
        }
    }
}

impl fmt::Display for DisplayFullDep<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(renamed) = self.dep.renamed.as_deref() {
            write!(f, "{renamed} ({})", self.dep.name)?;
        } else {
            f.write_str(&self.dep.name)?;
        }

        f.write_str(": ")?;

        if let Some(id) = self.dep.kind {
            f.write_str(self.state.kind(id))?;
            f.write_str(" ")?;
        }

        if let Some(id) = self.dep.path {
            write!(f, "in {}", self.state.path(id).as_str())?;
        }

        let Updates::Found(update) = &self.dep.updates else {
            unreachable!()
        };
        write!(f, ", {} -> {}", self.dep.version, update)
    }
}

#[derive(Debug, Clone, Copy)]
enum Action {
    Update,
    Skip,
}

impl Action {
    const ALL: &[Self] = &[Self::Update, Self::Skip];
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Update => "Update",
            Self::Skip => "Skip",
        })
    }
}
