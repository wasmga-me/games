//! Tab completion, as in a shell: commands, git's subcommands, branches,
//! remotes and file paths.

use crate::cmd::KNOWN;
use crate::git::{join, World};
use crate::vfs;

const COMMANDS: &[&str] = &[
    "git", "cd", "ls", "cat", "echo", "pwd", "touch", "edit", "nano", "vim", "vi", "code", "mkdir", "rm", "mv", "cp", "make", "hint",
    "solution", "undo", "clear", "help",
];
/// What git runs before it needs a repository (cmd.rs), then KNOWN.
const GIT_FIRST: &[&str] = &["init", "clone", "config", "help", "version"];

/// What the next word can be.
enum Kind {
    Words(&'static [&'static str]),
    Git,
    Refs,
    Branches,
    Remotes,
    Paths,
    Dirs,
    RefsAndPaths,
    Nothing,
}

impl World {
    /// The words that can finish the last word of `line` (the text before
    /// the caret), each in full, and that word's length in bytes.
    pub fn complete(&mut self, line: &str) -> (usize, Vec<String>) {
        // the last command, as git::split cuts the line; none inside quotes
        let mut start = 0;
        let mut quote = None;
        for (i, c) in line.char_indices() {
            match (quote, c) {
                (Some(q), _) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') => quote = Some(c),
                (None, ';') => start = i + 1,
                (None, '&') if line[i + 1..].starts_with('&') => start = i + 2,
                _ => {}
            }
        }
        if quote.is_some() {
            return (0, vec![]);
        }
        let seg = &line[start..];
        let mut words: Vec<&str> = seg.split_whitespace().collect();
        let mut cur = if seg.ends_with(char::is_whitespace) { "" } else { words.pop().unwrap_or("") };
        // `>file` is `>` and a file
        if let Some(i) = cur.rfind('>') {
            words.push(">");
            cur = &cur[i + 1..];
        }
        if cur.contains(['"', '\'']) {
            return (0, vec![]);
        }
        // the drawn view already holds the refs
        let names = |v: &[(String, usize)]| v.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
        let refs = |r: &crate::git::Repo| [names(&r.branches), names(&r.tags), names(&r.remote)].concat();
        let words_from = |list: &[&str]| list.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        let mut out: Vec<String> = match what_next(&words, cur) {
            Kind::Words(list) => words_from(list),
            Kind::Git => {
                let mut v = words_from(GIT_FIRST);
                v.extend(words_from(KNOWN));
                v
            }
            Kind::Refs => refs(&self.repo),
            Kind::Branches => {
                // git switch makes a local branch from origin/name
                let mut v = names(&self.repo.branches);
                v.extend(self.repo.remote.iter().filter_map(|x| x.0.strip_prefix("origin/").map(String::from)));
                v
            }
            Kind::Remotes => self.with_fs(|w| w.here().map(|g| g.remotes()).unwrap_or_default()),
            Kind::Paths => self.with_fs(|w| w.paths(cur, false)),
            Kind::Dirs => self.with_fs(|w| w.paths(cur, true)),
            Kind::RefsAndPaths => {
                let mut v = refs(&self.repo);
                v.extend(self.with_fs(|w| w.paths(cur, false)));
                v
            }
            Kind::Nothing => vec![],
        };
        out.retain(|w| w.starts_with(cur));
        out.sort();
        out.dedup();
        (cur.len(), out)
    }

    /// Files and folders that start like `cur`, folders ending in /.
    fn paths(&self, cur: &str, dirs_only: bool) -> Vec<String> {
        let (dir, name) = cur.split_at(cur.rfind('/').map_or(0, |i| i + 1));
        let Some(list) = vfs::cur().list(&join(&self.cwd, dir)) else { return vec![] };
        list.into_iter()
            .filter(|(n, d)| n.starts_with(name) && n != ".git" && (*d || !dirs_only))
            .filter(|(n, _)| !n.starts_with('.') || name.starts_with('.'))
            .map(|(n, d)| format!("{dir}{n}{}", if d { "/" } else { "" }))
            .collect()
    }
}

fn what_next(words: &[&str], cur: &str) -> Kind {
    let Some(&cmd) = words.first() else { return Kind::Words(COMMANDS) };
    if cur.starts_with('-') {
        return Kind::Nothing;
    }
    if matches!(words.last(), Some(&">") | Some(&">>")) {
        return Kind::Paths;
    }
    if cmd != "git" {
        return if cmd == "cd" { Kind::Dirs } else { Kind::Paths };
    }
    let Some(&sub) = words.get(1) else { return Kind::Git };
    // after `--` come paths
    if words.contains(&"--") {
        return Kind::Paths;
    }
    let args: Vec<&str> = words[2..].iter().copied().filter(|w| !w.starts_with('-')).collect();
    match (sub, args.first().copied()) {
        ("stash", None) => Kind::Words(&["apply", "clear", "drop", "list", "pop", "push", "show"]),
        ("bisect", None) => Kind::Words(&["bad", "good", "log", "reset", "run", "skip", "start"]),
        ("bisect", Some("good" | "bad" | "skip" | "start")) => Kind::Refs,
        ("worktree", None) => Kind::Words(&["add", "list", "prune", "remove"]),
        ("worktree", Some("add")) if args.len() == 1 => Kind::Paths,
        ("worktree", Some("add")) => Kind::Refs,
        ("worktree", Some("remove")) => Kind::Dirs,
        ("remote", None) => Kind::Words(&["add", "get-url", "remove", "show"]),
        ("remote", Some("remove" | "rm" | "get-url" | "show")) => Kind::Remotes,
        ("push" | "pull" | "fetch", None) => Kind::Remotes,
        ("push" | "pull" | "fetch" | "switch", _) => Kind::Branches,
        ("add" | "rm" | "mv" | "restore" | "ls-files" | "clean" | "init" | "clone", _) => Kind::Paths,
        ("checkout" | "diff", _) => Kind::RefsAndPaths,
        ("stash" | "bisect" | "worktree" | "remote" | "config" | "commit" | "status" | "help" | "version", _) => {
            Kind::Nothing
        }
        _ => Kind::Refs,
    }
}
