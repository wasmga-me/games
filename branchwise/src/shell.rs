//! The commands around git: moving between folders and changing files.

use crate::git::{join, pretty, World, FAIL, GOOD, HOME, OUT, R};
use crate::vfs;

impl World {
    pub(crate) fn shell(&mut self, a: &[&str]) -> R<()> {
        // `cmd ... > file` and `>> file`
        if let Some(i) = a.iter().position(|w| *w == ">" || *w == ">>") {
            let Some(file) = a.get(i + 1) else {
                return Err("syntax error: a file name must follow >".into());
            };
            let text = self.capture(&a[..i])?;
            let path = join(&self.cwd, file);
            let fs = vfs::cur();
            if fs.is_dir(&path) {
                return Err(format!("{}: is a directory", file));
            }
            if !fs.is_dir(&parent(&path)) {
                return Err(format!("{}: no such file or directory", file));
            }
            let mut b = if a[i] == ">>" { fs.read(&path).unwrap_or_default() } else { vec![] };
            b.extend_from_slice(text.as_bytes());
            fs.write(&path, &b).map_err(|_| format!("{file}: cannot write"))?;
            return Ok(());
        }
        match a[0] {
            "echo" | "cat" | "ls" | "pwd" => {
                let t = self.capture(a)?;
                if !t.is_empty() {
                    self.say(OUT, t);
                }
                Ok(())
            }
            "cd" => self.cd(a.get(1).copied().unwrap_or("~")),
            "touch" => {
                for f in &a[1..] {
                    let p = self.file_arg(f)?;
                    let fs = vfs::cur();
                    let b = fs.read(&p).unwrap_or_default();
                    fs.write(&p, &b).map_err(|_| format!("touch: cannot touch '{f}'"))?;
                }
                Ok(())
            }
            "edit" | "nano" | "vim" | "vi" | "code" => {
                if a.len() < 2 {
                    return Err(format!("{}: name a file to change, such as {} app.js", a[0], a[0]));
                }
                for f in &a[1..] {
                    let p = self.file_arg(f)?;
                    self.edits += 1;
                    let fs = vfs::cur();
                    let mut b = fs.read(&p).unwrap_or_default();
                    if !b.is_empty() && !b.ends_with(b"\n") {
                        b.push(b'\n');
                    }
                    b.extend_from_slice(format!("change {}\n", self.edits).as_bytes());
                    fs.write(&p, &b).map_err(|_| format!("{f}: cannot write"))?;
                }
                Ok(())
            }
            "mkdir" => {
                let parents = a.contains(&"-p");
                for f in a[1..].iter().filter(|x| !x.starts_with('-')) {
                    let p = join(&self.cwd, f);
                    let fs = vfs::cur();
                    if fs.exists(&p) {
                        if parents {
                            continue;
                        }
                        return Err(format!("mkdir: cannot create directory '{f}': File exists"));
                    }
                    if !parents && !fs.is_dir(&parent(&p)) {
                        return Err(format!("mkdir: cannot create directory '{f}': No such file or directory"));
                    }
                    fs.mkdir_all(&p).map_err(|_| format!("mkdir: cannot create directory '{f}'"))?;
                }
                Ok(())
            }
            "rm" => {
                let rec = a.iter().any(|x| x.starts_with('-') && x.contains('r'));
                let force = a.iter().any(|x| x.starts_with('-') && x.contains('f'));
                for f in a[1..].iter().filter(|x| !x.starts_with('-')) {
                    let p = join(&self.cwd, f);
                    let fs = vfs::cur();
                    if !fs.exists(&p) {
                        if force {
                            continue;
                        }
                        return Err(format!("rm: cannot remove '{f}': No such file or directory"));
                    }
                    if fs.is_dir(&p) && !rec {
                        return Err(format!("rm: cannot remove '{f}': Is a directory"));
                    }
                    if self.cwd == p || self.cwd.starts_with(&format!("{p}/")) {
                        return Err(format!("rm: refusing to remove '{f}': you are in it"));
                    }
                    fs.remove(&p).map_err(|_| format!("rm: cannot remove '{f}'"))?;
                }
                Ok(())
            }
            "mv" | "cp" => {
                let w: Vec<&&str> = a[1..].iter().filter(|x| !x.starts_with('-')).collect();
                if w.len() != 2 {
                    return Err(format!("{}: give a source and a destination", a[0]));
                }
                let from = join(&self.cwd, w[0]);
                let mut to = join(&self.cwd, w[1]);
                let fs = vfs::cur();
                if !fs.exists(&from) {
                    return Err(format!("{}: cannot stat '{}': No such file or directory", a[0], w[0]));
                }
                if fs.is_dir(&to) {
                    to = format!("{to}/{}", crate::git::base_name(&from));
                }
                if a[0] == "mv" {
                    fs.rename(&from, &to).map_err(|_| format!("mv: cannot move '{}'", w[0]))?;
                } else {
                    let b = fs.read(&from).ok_or(format!("cp: -r not supported: '{}' is a directory", w[0]))?;
                    fs.write(&to, &b).map_err(|_| format!("cp: cannot create '{}'", w[1]))?;
                }
                Ok(())
            }
            "make" | "test" | "./test" | "npm" => {
                if a[0] == "make" && a.get(1).is_some_and(|t| *t != "test" && *t != "check") {
                    return Err(format!("make: *** No rule to make target '{}'.  Stop.", a[1]));
                }
                self.test();
                Ok(())
            }
            "exit" | "logout" => Err("This shell stays open; switch lessons from the menu.".into()),
            "sudo" => Err("sudo: you are root here already".into()),
            c => Err(format!("{c}: command not found")),
        }
    }

    /// The output of echo, cat, ls or pwd, for printing or for > file.
    pub(crate) fn capture(&mut self, a: &[&str]) -> R<String> {
        let fs = vfs::cur();
        match a.first().copied().unwrap_or("") {
            "echo" => {
                let (nl, words) = match a.get(1) {
                    Some(&"-n") => (false, &a[2..]),
                    _ => (true, &a[1..]),
                };
                Ok(format!("{}{}", words.join(" "), if nl { "\n" } else { "" }))
            }
            "pwd" => Ok(format!("{}\n", self.cwd)),
            "cat" => {
                let mut s = String::new();
                for f in a[1..].iter().filter(|x| !x.starts_with('-')) {
                    let p = join(&self.cwd, f);
                    if fs.is_dir(&p) {
                        return Err(format!("cat: {f}: Is a directory"));
                    }
                    match fs.read(&p) {
                        Some(b) => s.push_str(&String::from_utf8_lossy(&b)),
                        None => return Err(format!("cat: {f}: No such file or directory")),
                    }
                }
                Ok(s)
            }
            "ls" => {
                let all = a.iter().any(|x| x.starts_with('-') && x.contains('a'));
                let dirs: Vec<&&str> = a[1..].iter().filter(|x| !x.starts_with('-')).collect();
                let targets: Vec<String> = if dirs.is_empty() {
                    vec![self.cwd.clone()]
                } else {
                    dirs.iter().map(|d| join(&self.cwd, d)).collect()
                };
                let mut s = String::new();
                for t in targets {
                    match fs.list(&t) {
                        Some(names) => {
                            let v: Vec<String> = names
                                .into_iter()
                                .filter(|(n, _)| all || !n.starts_with('.'))
                                .map(|(n, d)| if d { format!("{n}/") } else { n })
                                .collect();
                            if !v.is_empty() {
                                s.push_str(&v.join("  "));
                                s.push('\n');
                            }
                        }
                        None if fs.exists(&t) => {
                            s.push_str(&crate::git::base_name(&t));
                            s.push('\n');
                        }
                        None => return Err(format!("ls: cannot access '{}': No such file or directory", pretty(&t))),
                    }
                }
                Ok(s)
            }
            c => Err(format!("{c}: can't send that to a file here")),
        }
    }

    pub(crate) fn cd(&mut self, p: &str) -> R<()> {
        let to = join(&self.cwd, p);
        let fs = vfs::cur();
        if !fs.exists(&to) {
            return Err(format!("cd: no such file or directory: {p}"));
        }
        if !fs.is_dir(&to) {
            return Err(format!("cd: not a directory: {p}"));
        }
        self.cwd = to;
        Ok(())
    }

    /// A file to write: its folder must exist.
    pub(crate) fn file_arg(&self, f: &str) -> R<String> {
        let p = join(&self.cwd, f);
        let fs = vfs::cur();
        if fs.is_dir(&p) {
            return Err(format!("{f}: is a directory"));
        }
        if !fs.is_dir(&parent(&p)) {
            return Err(format!("{f}: no such file or directory"));
        }
        Ok(p)
    }

    /// `make test`: the tests fail while a file in the project holds a
    /// line marked BUG. True when they pass.
    pub(crate) fn test(&mut self) -> bool {
        let root = lg_root(&self.cwd);
        let bad: Vec<String> = vfs::cur()
            .files_under(&root)
            .into_iter()
            .filter(|(_, b)| String::from_utf8_lossy(b).contains("BUG"))
            .map(|(p, _)| p)
            .collect();
        if bad.is_empty() {
            self.say(GOOD, "All tests passed.");
            true
        } else {
            for p in bad {
                let name = p.rsplit('/').next().unwrap_or(&p).to_string();
                self.say(FAIL, format!("FAIL {name}: search shows stale results"));
            }
            false
        }
    }
}

fn parent(p: &str) -> String {
    match p.rfind('/') {
        Some(0) | None => "/".into(),
        Some(i) => p[..i].to_string(),
    }
}

/// The working tree holding `dir`, or `dir` itself outside one.
fn lg_root(dir: &str) -> String {
    match crate::lg::discover(dir) {
        Some(g) if !g.is_bare() => g.workdir(),
        _ => {
            if dir == HOME {
                dir.to_string()
            } else {
                dir.to_string()
            }
        }
    }
}
