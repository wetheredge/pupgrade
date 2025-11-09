use crossterm::event::{self, Event, KeyCode};
use ratatui::crossterm;
use ratatui::style::{self, Style};
use ratatui::widgets;

pub(crate) fn run(
    mut terminal: ratatui::DefaultTerminal,
    deps: &mut crate::Deps,
) -> anyhow::Result<()> {
    let names = get_names(&deps);
    let get_list = |deps: &crate::Deps| {
        let items = deps
            .iter_dependencies()
            .zip(names.iter())
            .map(|(dep, name)| {
                let mut style = Style::new();
                if dep.skip {
                    style.add_modifier = style::Modifier::DIM | style::Modifier::ITALIC;
                }
                widgets::ListItem::new(name.as_str()).style(style)
            })
            .collect::<Vec<_>>();

        widgets::List::new(items).highlight_symbol("> ")
    };

    let mut list = get_list(&deps);
    let mut state = widgets::ListState::default().with_selected(Some(0));

    let mut height = terminal.size()?.height;
    let is_ctrl = |key: event::KeyEvent| key.modifiers == event::KeyModifiers::CONTROL;
    loop {
        terminal.draw(|frame| frame.render_stateful_widget(&list, frame.area(), &mut state))?;

        match event::read()? {
            Event::Key(key) => match key.code {
                KeyCode::Esc => return Ok(()),
                KeyCode::Char(' ') => {
                    let current = state.selected().unwrap();
                    deps.get_dependency_mut(current).skip ^= true;
                    list = get_list(&deps);
                }

                KeyCode::Char('j') | KeyCode::Down => state.select_next(),
                KeyCode::Char('k') | KeyCode::Up => state.select_previous(),
                KeyCode::Char('n') if is_ctrl(key) => state.select_next(),
                KeyCode::Char('p') if is_ctrl(key) => state.select_previous(),

                KeyCode::Char('d') if is_ctrl(key) => state.scroll_down_by(height / 2),
                KeyCode::Char('u') if is_ctrl(key) => state.scroll_up_by(height / 2),

                KeyCode::Char('f') if is_ctrl(key) => state.scroll_down_by(height),
                KeyCode::Char('b') if is_ctrl(key) => state.scroll_up_by(height),
                KeyCode::PageDown => state.scroll_down_by(height),
                KeyCode::PageUp => state.scroll_up_by(height),

                KeyCode::Char('g') | KeyCode::Home => state.select_first(),
                KeyCode::Char('G') | KeyCode::End => state.select_last(),

                _ => {}
            },
            Event::Resize(_, h) => height = h,
            _ => {}
        }
    }
}

fn get_names(deps: &crate::Deps) -> Box<[String]> {
    let mut names = Vec::new();
    let mut stack = Vec::<(Option<String>, _)>::new();
    stack.push((None, deps.iter_root_groups().peekable()));
    while let Some((_, iter)) = stack.last_mut() {
        if let Some(group) = iter.next() {
            let group_name = group.name();

            for dep in group.iter_dependencies() {
                let mut name = Vec::new();
                for ancestor in stack.iter().filter_map(|(name, _)| name.as_ref()) {
                    name.push(ancestor.to_owned());
                }
                name.push(group_name.to_owned());
                name.push(dep.name.to_owned());
                let name = name.join(" > ");

                names.push(format!("{name}: {}", &dep.version));
            }

            let mut subgroups = group.iter_subgroups().peekable();
            if subgroups.peek().is_some() {
                stack.push((Some(group_name.to_owned()), subgroups));
            }
        } else {
            stack.pop();
        }
    }

    names.into_boxed_slice()
}
