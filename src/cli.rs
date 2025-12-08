pub(crate) struct Cli {
    pub(crate) cwd: Option<camino::Utf8PathBuf>,
    pub(crate) action: Action,
}

#[derive(Clone)]
pub(crate) enum Action {
    Usage { requested: bool },
    Init,
    Edit,
    Summarize,
    Clean,
}

pub(crate) static USAGE: &str = concat!(
    "Usage: ",
    env!("CARGO_PKG_NAME"),
    " [--cwd=DIR] <init | edit | summarize | clean>"
);

pub(crate) fn parse() -> Result<Cli, lexopt::Error> {
    use lexopt::prelude::*;

    let mut cwd = None;
    let mut action = None;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("cwd") if action.is_none() => {
                cwd = Some(parser.value()?.parse()?);
            }

            Short('h') | Long("help") => action = Some(Action::Usage { requested: true }),
            Value(v) if v == "help" => action = Some(Action::Usage { requested: true }),

            Value(v) if v == "init" => action = Some(Action::Init),
            Value(v) if v == "edit" => action = Some(Action::Edit),
            Value(v) if v == "summarize" => action = Some(Action::Summarize),
            Value(v) if v == "clean" => action = Some(Action::Clean),

            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Cli {
        cwd,
        action: action.unwrap_or(Action::Usage { requested: false }),
    })
}
