use clap::{App, Arg, ArgMatches};

pub struct Options {
    pub yes: bool,
    pub continue_: bool,
}

pub fn cli_options() -> Options {
    let matches = matches();

    Options {
        yes: matches.is_present("yes"),
        continue_: matches.is_present("continue"),
    }
}

fn matches() -> ArgMatches<'static> {
    App::new("sortnbackup")
        .about("Copy files from multiple sources to multiple targets using highly customizable filters and rules")
        .arg(Arg::with_name("yes").help("Answer all questions with yes (non-interactive mode)").long("yes"))
        .arg(Arg::with_name("continue").help("Continue a previously started backup").short("c").long("continue"))
        .get_matches()
}
