//! libgit2, as the game uses it: the C calls (with libgit2/shim.c for the
//! ones that take option structs) and small safe wrappers around them.
//! Every call runs inside `vfs::enter`, on the world's own files.

#![allow(dead_code)]

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};

type P = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct Oid(pub [u8; 20]);

#[repr(C)]
struct StrArray {
    strings: *mut *mut c_char,
    count: usize,
}

#[repr(C)]
struct Buf {
    ptr: *mut c_char,
    reserved: usize,
    size: usize,
}

#[repr(C)]
struct ErrS {
    message: *const c_char,
    klass: c_int,
}

#[repr(C)]
struct Sig {
    name: *const c_char,
    email: *const c_char,
    time: i64,
    offset: c_int,
    sign: c_char,
}

#[repr(C)]
struct RevSpec {
    from: P,
    to: P,
    flags: c_uint,
}

pub const ENOTFOUND: c_int = -3;
pub const EEXISTS: c_int = -4;
pub const EUNBORN: c_int = -9;
pub const ECONFLICT: c_int = -13;

pub const OBJ_ANY: c_int = -2;
pub const OBJ_COMMIT: c_int = 1;
pub const OBJ_TREE: c_int = 2;
pub const OBJ_BLOB: c_int = 3;
pub const OBJ_TAG: c_int = 4;

pub const CHECKOUT_SAFE: u32 = 0;
pub const CHECKOUT_FORCE: u32 = 1 << 1;
pub const CHECKOUT_ALLOW_CONFLICTS: u32 = 1 << 4;
pub const CHECKOUT_USE_OURS: u32 = 1 << 11;
pub const CHECKOUT_USE_THEIRS: u32 = 1 << 12;

pub const ST_INDEX_NEW: u32 = 1;
pub const ST_INDEX_MODIFIED: u32 = 2;
pub const ST_INDEX_DELETED: u32 = 4;
pub const ST_WT_NEW: u32 = 128;
pub const ST_WT_MODIFIED: u32 = 256;
pub const ST_WT_DELETED: u32 = 512;
pub const ST_CONFLICTED: u32 = 32768;

pub const STATE_NONE: c_int = 0;
pub const STATE_MERGE: c_int = 1;
pub const STATE_REVERT: c_int = 2;
pub const STATE_CHERRYPICK: c_int = 4;

extern "C" {
    fn git_libgit2_init() -> c_int;
    fn git_error_last() -> *const ErrS;
    fn git_error_clear();
    fn git_strarray_dispose(a: *mut StrArray);
    fn git_buf_dispose(b: *mut Buf);

    fn git_repository_init(out: *mut P, path: *const c_char, bare: c_uint) -> c_int;
    fn git_repository_open(out: *mut P, path: *const c_char) -> c_int;
    fn git_repository_open_ext(out: *mut P, path: *const c_char, flags: c_uint, ceiling: *const c_char) -> c_int;
    fn git_repository_free(r: P);
    fn git_repository_workdir(r: P) -> *const c_char;
    fn git_repository_path(r: P) -> *const c_char;
    fn git_repository_commondir(r: P) -> *const c_char;
    fn git_repository_is_bare(r: P) -> c_int;
    fn git_repository_head_detached(r: P) -> c_int;
    fn git_repository_head_unborn(r: P) -> c_int;
    fn git_repository_set_head(r: P, refname: *const c_char) -> c_int;
    fn git_repository_set_head_detached(r: P, id: *const Oid) -> c_int;
    fn git_repository_state(r: P) -> c_int;
    fn git_repository_state_cleanup(r: P) -> c_int;
    fn git_repository_index(out: *mut P, r: P) -> c_int;
    fn git_repository_config(out: *mut P, r: P) -> c_int;
    fn git_repository_message(out: *mut Buf, r: P) -> c_int;
    fn git_repository_message_remove(r: P) -> c_int;

    fn git_reference_lookup(out: *mut P, r: P, name: *const c_char) -> c_int;
    fn git_reference_name_to_id(out: *mut Oid, r: P, name: *const c_char) -> c_int;
    fn git_reference_create(out: *mut P, r: P, name: *const c_char, id: *const Oid, force: c_int, msg: *const c_char) -> c_int;
    fn git_reference_symbolic_create(out: *mut P, r: P, name: *const c_char, target: *const c_char, force: c_int, msg: *const c_char) -> c_int;
    fn git_reference_delete(rf: P) -> c_int;
    fn git_reference_free(rf: P);
    fn git_reference_target(rf: P) -> *const Oid;
    fn git_reference_symbolic_target(rf: P) -> *const c_char;
    fn git_reference_name(rf: P) -> *const c_char;
    fn git_reference_list(out: *mut StrArray, r: P) -> c_int;
    fn git_reference_dwim(out: *mut P, r: P, short: *const c_char) -> c_int;
    fn git_reference_resolve(out: *mut P, rf: P) -> c_int;

    fn git_revparse_single(out: *mut P, r: P, spec: *const c_char) -> c_int;
    fn git_revparse_ext(out: *mut P, rf: *mut P, r: P, spec: *const c_char) -> c_int;
    fn git_object_peel(out: *mut P, obj: P, kind: c_int) -> c_int;
    fn git_object_id(obj: P) -> *const Oid;
    fn git_object_type(obj: P) -> c_int;
    fn git_object_free(obj: P);
    fn git_object_lookup(out: *mut P, r: P, id: *const Oid, kind: c_int) -> c_int;

    fn git_commit_lookup(out: *mut P, r: P, id: *const Oid) -> c_int;
    fn git_commit_free(c: P);
    fn git_commit_message(c: P) -> *const c_char;
    fn git_commit_summary(c: P) -> *const c_char;
    fn git_commit_parentcount(c: P) -> c_uint;
    fn git_commit_parent_id(c: P, n: c_uint) -> *const Oid;
    fn git_commit_tree_id(c: P) -> *const Oid;
    fn git_commit_author(c: P) -> *const Sig;
    fn git_commit_committer(c: P) -> *const Sig;
    fn git_commit_create(
        id: *mut Oid,
        r: P,
        update_ref: *const c_char,
        author: *const Sig,
        committer: *const Sig,
        enc: *const c_char,
        msg: *const c_char,
        tree: P,
        n: usize,
        parents: *const P,
    ) -> c_int;
    fn git_tree_lookup(out: *mut P, r: P, id: *const Oid) -> c_int;
    fn git_tree_free(t: P);
    fn git_blob_lookup(out: *mut P, r: P, id: *const Oid) -> c_int;
    fn git_blob_rawcontent(b: P) -> *const u8;
    fn git_blob_rawsize(b: P) -> u64;
    fn git_blob_free(b: P);
    fn git_tag_lookup(out: *mut P, r: P, id: *const Oid) -> c_int;
    fn git_tag_message(t: P) -> *const c_char;
    fn git_tag_target_id(t: P) -> *const Oid;
    fn git_tag_free(t: P);

    fn git_signature_default(out: *mut *mut Sig, r: P) -> c_int;
    fn git_signature_free(s: *mut Sig);

    fn git_index_add_all(i: P, ps: *const StrArray, flags: c_uint, cb: P, payload: P) -> c_int;
    fn git_index_update_all(i: P, ps: *const StrArray, cb: P, payload: P) -> c_int;
    fn git_index_remove_bypath(i: P, path: *const c_char) -> c_int;
    fn git_index_add_bypath(i: P, path: *const c_char) -> c_int;
    fn git_index_write(i: P) -> c_int;
    fn git_index_write_tree(out: *mut Oid, i: P) -> c_int;
    fn git_index_read_tree(i: P, tree: P) -> c_int;
    fn git_index_read(i: P, force: c_int) -> c_int;
    fn git_index_has_conflicts(i: P) -> c_int;
    fn git_index_conflict_cleanup(i: P) -> c_int;
    fn git_index_free(i: P);

    fn git_branch_create(out: *mut P, r: P, name: *const c_char, target: P, force: c_int) -> c_int;
    fn git_branch_move(out: *mut P, rf: P, name: *const c_char, force: c_int) -> c_int;
    fn git_branch_upstream_name(out: *mut Buf, r: P, refname: *const c_char) -> c_int;
    fn git_branch_set_upstream(rf: P, up: *const c_char) -> c_int;

    fn git_tag_create_lightweight(out: *mut Oid, r: P, name: *const c_char, target: P, force: c_int) -> c_int;
    fn git_tag_create(out: *mut Oid, r: P, name: *const c_char, target: P, tagger: *const Sig, msg: *const c_char, force: c_int) -> c_int;

    fn git_reset(r: P, target: P, kind: c_int, opts: P) -> c_int;
    fn git_reset_default(r: P, target: P, ps: *const StrArray) -> c_int;

    fn git_reflog_read(out: *mut P, r: P, name: *const c_char) -> c_int;
    fn git_reflog_entrycount(l: P) -> usize;
    fn git_reflog_entry_byindex(l: P, i: usize) -> P;
    fn git_reflog_entry_id_new(e: P) -> *const Oid;
    fn git_reflog_entry_message(e: P) -> *const c_char;
    fn git_reflog_free(l: P);

    fn git_graph_ahead_behind(ahead: *mut usize, behind: *mut usize, r: P, local: *const Oid, up: *const Oid) -> c_int;
    fn git_graph_descendant_of(r: P, commit: *const Oid, ancestor: *const Oid) -> c_int;
    fn git_merge_base(out: *mut Oid, r: P, a: *const Oid, b: *const Oid) -> c_int;

    fn git_config_set_string(c: P, name: *const c_char, v: *const c_char) -> c_int;
    fn git_config_get_string_buf(out: *mut Buf, c: P, name: *const c_char) -> c_int;
    fn git_config_delete_entry(c: P, name: *const c_char) -> c_int;
    fn git_config_open_ondisk(out: *mut P, path: *const c_char) -> c_int;
    fn git_config_free(c: P);

    fn git_remote_create(out: *mut P, r: P, name: *const c_char, url: *const c_char) -> c_int;
    fn git_remote_lookup(out: *mut P, r: P, name: *const c_char) -> c_int;
    fn git_remote_url(rm: P) -> *const c_char;
    fn git_remote_free(rm: P);
    fn git_remote_list(out: *mut StrArray, r: P) -> c_int;
    fn git_remote_delete(r: P, name: *const c_char) -> c_int;

    fn git_oid_fromstr(out: *mut Oid, s: *const c_char) -> c_int;

    fn bw_text() -> *const c_char;
    fn bw_text_len() -> usize;
    fn bw_text_clear();
    fn bw_status(r: P) -> c_int;
    fn bw_conflicts(r: P) -> c_int;
    fn bw_index(r: P) -> c_int;
    fn bw_checkout(r: P, id: *const Oid, strategy: c_uint, paths: *const *const c_char, n: usize) -> c_int;
    fn bw_merge(r: P, id: *const Oid) -> c_int;
    fn bw_cherrypick(r: P, id: *const Oid, mainline: c_uint, revert: c_int) -> c_int;
    fn bw_stash_save(r: P, msg: *const c_char, flags: c_uint) -> c_int;
    fn bw_stash_apply(r: P, i: usize, pop: c_int, index: c_int) -> c_int;
    fn bw_stash_list(r: P) -> c_int;
    fn bw_diff(r: P, mode: c_int, a: *const Oid, b: *const Oid, what: c_int, paths: *const *const c_char, n: usize, out: *mut Oid) -> c_int;
    fn bw_objects(r: P) -> c_int;
    fn bw_tree_files(r: P, id: *const Oid) -> c_int;
    fn bw_fetch(r: P, name: *const c_char, prune: c_int, tags: c_int) -> c_int;
    fn bw_push(r: P, name: *const c_char, specs: *const *const c_char, n: usize) -> c_int;
    fn bw_clone(url: *const c_char, path: *const c_char, bare: c_int) -> c_int;
    fn bw_worktree_add(r: P, name: *const c_char, path: *const c_char, branch: *const c_char) -> c_int;
    fn bw_worktrees(r: P) -> c_int;
    fn bw_worktree_prune(r: P, name: *const c_char) -> c_int;
    fn bw_describe(r: P, id: *const Oid, tags: c_int, always: c_int) -> c_int;
    fn bw_ls_tree(r: P, id: *const Oid) -> c_int;
    fn bw_stash_drop(r: P, i: usize) -> c_int;
    fn bw_branch_delete(r: P, name: *const c_char, remote: c_int) -> c_int;
}

pub type R<T> = Result<T, String>;

pub fn init() {
    static mut DONE: bool = false;
    unsafe {
        if !DONE {
            git_libgit2_init();
            DONE = true;
        }
    }
}

fn cs(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap()
}

unsafe fn st(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// libgit2's last error message.
pub fn last_error() -> String {
    unsafe {
        let e = git_error_last();
        let m = if e.is_null() { String::new() } else { st((*e).message) };
        git_error_clear();
        m
    }
}

/// The return code, or the error as text.
fn ok(rc: c_int) -> R<c_int> {
    if rc < 0 {
        let m = last_error();
        Err(if m.is_empty() { format!("error {rc}") } else { m })
    } else {
        Ok(rc)
    }
}

/// Like `ok`, keeping the code for callers that tell errors apart.
pub fn code(rc: c_int) -> Result<c_int, (c_int, String)> {
    if rc < 0 {
        Err((rc, last_error()))
    } else {
        Ok(rc)
    }
}

/// What the shim wrote.
pub fn text() -> String {
    unsafe {
        let b = std::slice::from_raw_parts(bw_text() as *const u8, bw_text_len());
        let s = String::from_utf8_lossy(b).into_owned();
        bw_text_clear();
        s
    }
}
fn clear() {
    unsafe { bw_text_clear() }
}

impl Oid {
    pub fn hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
    pub fn parse(s: &str) -> Option<Oid> {
        if s.len() != 40 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut o = Oid::default();
        unsafe { git_oid_fromstr(&mut o, cs(s).as_ptr()) };
        Some(o)
    }
    pub fn is_zero(&self) -> bool {
        self.0 == [0; 20]
    }
}

fn oid_at(p: *const Oid) -> Oid {
    if p.is_null() {
        Oid::default()
    } else {
        unsafe { *p }
    }
}

struct Paths {
    _own: Vec<CString>,
    ptrs: Vec<*const c_char>,
}
fn paths(p: &[String]) -> Paths {
    let own: Vec<CString> = p.iter().map(|s| cs(s)).collect();
    let ptrs = own.iter().map(|c| c.as_ptr()).collect();
    Paths { _own: own, ptrs }
}
fn strarray(p: &Paths) -> StrArray {
    StrArray {
        strings: p.ptrs.as_ptr() as *mut *mut c_char,
        count: p.ptrs.len(),
    }
}
unsafe fn take_strarray(mut a: StrArray) -> Vec<String> {
    let v = (0..a.count).map(|i| st(*a.strings.add(i))).collect();
    git_strarray_dispose(&mut a);
    v
}
unsafe fn take_buf(mut b: Buf) -> String {
    let s = if b.ptr.is_null() {
        String::new()
    } else {
        String::from_utf8_lossy(std::slice::from_raw_parts(b.ptr as *const u8, b.size)).into_owned()
    };
    git_buf_dispose(&mut b);
    s
}

pub struct Commit {
    pub id: Oid,
    pub parents: Vec<Oid>,
    pub msg: String,
    pub summary: String,
    pub tree: Oid,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub ctime: i64,
}

pub struct Repo(P);

impl Drop for Repo {
    fn drop(&mut self) {
        unsafe { git_repository_free(self.0) }
    }
}

pub fn create(path: &str, bare: bool) -> R<Repo> {
    let mut r = std::ptr::null_mut();
    ok(unsafe { git_repository_init(&mut r, cs(path).as_ptr(), bare as c_uint) })?;
    Ok(Repo(r))
}

pub fn open(path: &str) -> R<Repo> {
    let mut r = std::ptr::null_mut();
    ok(unsafe { git_repository_open(&mut r, cs(path).as_ptr()) })?;
    Ok(Repo(r))
}

/// The repository holding `path` (looking in parent folders too).
pub fn discover(path: &str) -> Option<Repo> {
    let mut r = std::ptr::null_mut();
    let rc = unsafe { git_repository_open_ext(&mut r, cs(path).as_ptr(), 0, std::ptr::null()) };
    git_clear(rc);
    (rc == 0).then(|| Repo(r))
}

fn git_clear(rc: c_int) {
    if rc < 0 {
        unsafe { git_error_clear() }
    }
}

pub fn clone(url: &str, path: &str, bare: bool) -> R<()> {
    ok(unsafe { bw_clone(cs(url).as_ptr(), cs(path).as_ptr(), bare as c_int) })?;
    Ok(())
}

impl Repo {
    /// The working tree, without the final slash ("" when bare).
    pub fn workdir(&self) -> String {
        unsafe { st(git_repository_workdir(self.0)) }
            .trim_end_matches('/')
            .to_string()
    }
    /// The .git folder (a worktree's own one for a linked worktree).
    pub fn gitdir(&self) -> String {
        unsafe { st(git_repository_path(self.0)) }
            .trim_end_matches('/')
            .to_string()
    }
    pub fn commondir(&self) -> String {
        unsafe { st(git_repository_commondir(self.0)) }
            .trim_end_matches('/')
            .to_string()
    }
    pub fn is_bare(&self) -> bool {
        unsafe { git_repository_is_bare(self.0) == 1 }
    }
    pub fn detached(&self) -> bool {
        unsafe { git_repository_head_detached(self.0) == 1 }
    }
    pub fn unborn(&self) -> bool {
        unsafe { git_repository_head_unborn(self.0) == 1 }
    }
    pub fn state(&self) -> c_int {
        unsafe { git_repository_state(self.0) }
    }
    pub fn cleanup_state(&self) {
        unsafe {
            git_repository_state_cleanup(self.0);
        }
    }
    pub fn message(&self) -> Option<String> {
        let mut b = Buf { ptr: std::ptr::null_mut(), reserved: 0, size: 0 };
        let rc = unsafe { git_repository_message(&mut b, self.0) };
        git_clear(rc);
        (rc == 0).then(|| unsafe { take_buf(b) })
    }

    // ---------- refs ----------

    /// The branch HEAD is on ("main"), if any (also when unborn).
    pub fn head_branch(&self) -> Option<String> {
        let t = self.symref("HEAD")?;
        t.strip_prefix("refs/heads/").map(|s| s.to_string())
    }
    /// The commit HEAD is on.
    pub fn head(&self) -> Option<Oid> {
        self.ref_id("HEAD")
    }
    pub fn ref_id(&self, name: &str) -> Option<Oid> {
        let mut o = Oid::default();
        let rc = unsafe { git_reference_name_to_id(&mut o, self.0, cs(name).as_ptr()) };
        git_clear(rc);
        (rc == 0).then_some(o)
    }
    /// Where a symbolic ref points ("refs/heads/main" for HEAD).
    pub fn symref(&self, name: &str) -> Option<String> {
        let mut rf = std::ptr::null_mut();
        let rc = unsafe { git_reference_lookup(&mut rf, self.0, cs(name).as_ptr()) };
        git_clear(rc);
        if rc != 0 {
            return None;
        }
        let t = unsafe { git_reference_symbolic_target(rf) };
        let s = (!t.is_null()).then(|| unsafe { st(t) });
        unsafe { git_reference_free(rf) };
        s
    }
    pub fn set_ref(&self, name: &str, id: &Oid, msg: &str) -> R<()> {
        let mut rf = std::ptr::null_mut();
        ok(unsafe { git_reference_create(&mut rf, self.0, cs(name).as_ptr(), id, 1, cs(msg).as_ptr()) })?;
        unsafe { git_reference_free(rf) };
        Ok(())
    }
    pub fn new_ref(&self, name: &str, id: &Oid, msg: &str) -> R<()> {
        let mut rf = std::ptr::null_mut();
        ok(unsafe { git_reference_create(&mut rf, self.0, cs(name).as_ptr(), id, 0, cs(msg).as_ptr()) })?;
        unsafe { git_reference_free(rf) };
        Ok(())
    }
    pub fn set_symref(&self, name: &str, target: &str, msg: &str) -> R<()> {
        let mut rf = std::ptr::null_mut();
        ok(unsafe {
            git_reference_symbolic_create(&mut rf, self.0, cs(name).as_ptr(), cs(target).as_ptr(), 1, cs(msg).as_ptr())
        })?;
        unsafe { git_reference_free(rf) };
        Ok(())
    }
    pub fn delete_ref(&self, name: &str) -> R<()> {
        let mut rf = std::ptr::null_mut();
        ok(unsafe { git_reference_lookup(&mut rf, self.0, cs(name).as_ptr()) })?;
        let rc = unsafe { git_reference_delete(rf) };
        unsafe { git_reference_free(rf) };
        ok(rc)?;
        Ok(())
    }
    pub fn refs(&self) -> Vec<String> {
        let mut a = StrArray { strings: std::ptr::null_mut(), count: 0 };
        if unsafe { git_reference_list(&mut a, self.0) } < 0 {
            last_error();
            return vec![];
        }
        let mut v = unsafe { take_strarray(a) };
        v.sort();
        v
    }
    /// Points HEAD at a branch ("refs/heads/x"), not touching files.
    pub fn set_head(&self, refname: &str) -> R<()> {
        ok(unsafe { git_repository_set_head(self.0, cs(refname).as_ptr()) })?;
        Ok(())
    }
    pub fn set_head_detached(&self, id: &Oid) -> R<()> {
        ok(unsafe { git_repository_set_head_detached(self.0, id) })?;
        Ok(())
    }
    pub fn upstream(&self, branch: &str) -> Option<String> {
        let mut b = Buf { ptr: std::ptr::null_mut(), reserved: 0, size: 0 };
        let rc = unsafe { git_branch_upstream_name(&mut b, self.0, cs(&format!("refs/heads/{branch}")).as_ptr()) };
        git_clear(rc);
        (rc == 0).then(|| unsafe { take_buf(b) })
    }
    /// Sets (or with None, removes) a branch's upstream ("origin/main").
    pub fn set_upstream(&self, branch: &str, up: Option<&str>) -> R<()> {
        let mut rf = std::ptr::null_mut();
        ok(unsafe { git_reference_lookup(&mut rf, self.0, cs(&format!("refs/heads/{branch}")).as_ptr()) })?;
        let c = up.map(cs);
        let rc = unsafe { git_branch_set_upstream(rf, c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr())) };
        unsafe { git_reference_free(rf) };
        ok(rc)?;
        Ok(())
    }
    pub fn create_branch(&self, name: &str, at: &Oid, force: bool) -> R<()> {
        let mut c = std::ptr::null_mut();
        ok(unsafe { git_commit_lookup(&mut c, self.0, at) })?;
        let mut rf = std::ptr::null_mut();
        let rc = unsafe { git_branch_create(&mut rf, self.0, cs(name).as_ptr(), c, force as c_int) };
        unsafe { git_commit_free(c) };
        ok(rc)?;
        unsafe { git_reference_free(rf) };
        Ok(())
    }
    pub fn rename_branch(&self, old: &str, new: &str, force: bool) -> R<()> {
        let mut rf = std::ptr::null_mut();
        ok(unsafe { git_reference_lookup(&mut rf, self.0, cs(&format!("refs/heads/{old}")).as_ptr()) })?;
        let mut out = std::ptr::null_mut();
        let rc = unsafe { git_branch_move(&mut out, rf, cs(new).as_ptr(), force as c_int) };
        unsafe { git_reference_free(rf) };
        ok(rc)?;
        unsafe { git_reference_free(out) };
        Ok(())
    }

    // ---------- objects ----------

    /// The object a revision names, and its type.
    pub fn revparse(&self, spec: &str) -> Result<(Oid, c_int), (c_int, String)> {
        let mut o = std::ptr::null_mut();
        code(unsafe { git_revparse_single(&mut o, self.0, cs(spec).as_ptr()) })?;
        let r = unsafe { (*git_object_id(o), git_object_type(o)) };
        unsafe { git_object_free(o) };
        Ok(r)
    }
    /// The commit a revision names.
    pub fn commit_of(&self, spec: &str) -> Result<Oid, (c_int, String)> {
        let mut o = std::ptr::null_mut();
        code(unsafe { git_revparse_single(&mut o, self.0, cs(spec).as_ptr()) })?;
        let mut c = std::ptr::null_mut();
        let rc = unsafe { git_object_peel(&mut c, o, OBJ_COMMIT) };
        unsafe { git_object_free(o) };
        code(rc)?;
        let id = unsafe { *git_object_id(c) };
        unsafe { git_object_free(c) };
        Ok(id)
    }
    /// What a name means as a ref, for `git switch -` and the like: the
    /// full ref name, when it names one.
    pub fn revparse_ref(&self, spec: &str) -> Option<String> {
        let mut o = std::ptr::null_mut();
        let mut rf = std::ptr::null_mut();
        let rc = unsafe { git_revparse_ext(&mut o, &mut rf, self.0, cs(spec).as_ptr()) };
        git_clear(rc);
        if rc != 0 {
            return None;
        }
        unsafe { git_object_free(o) };
        if rf.is_null() {
            return None;
        }
        let n = unsafe { st(git_reference_name(rf)) };
        unsafe { git_reference_free(rf) };
        Some(n)
    }
    pub fn commit(&self, id: &Oid) -> R<Commit> {
        let mut c = std::ptr::null_mut();
        ok(unsafe { git_commit_lookup(&mut c, self.0, id) })?;
        let r = unsafe {
            let a = &*git_commit_author(c);
            let cm = &*git_commit_committer(c);
            Commit {
                id: *id,
                parents: (0..git_commit_parentcount(c)).map(|i| oid_at(git_commit_parent_id(c, i))).collect(),
                msg: st(git_commit_message(c)),
                summary: st(git_commit_summary(c)),
                tree: oid_at(git_commit_tree_id(c)),
                author: st(a.name),
                email: st(a.email),
                time: a.time,
                ctime: cm.time,
            }
        };
        unsafe { git_commit_free(c) };
        Ok(r)
    }
    /// An annotated tag's message and target.
    pub fn tag_info(&self, id: &Oid) -> Option<(String, Oid)> {
        let mut t = std::ptr::null_mut();
        let rc = unsafe { git_tag_lookup(&mut t, self.0, id) };
        git_clear(rc);
        if rc != 0 {
            return None;
        }
        let r = unsafe { (st(git_tag_message(t)), oid_at(git_tag_target_id(t))) };
        unsafe { git_tag_free(t) };
        Some(r)
    }
    pub fn blob(&self, id: &Oid) -> Option<Vec<u8>> {
        let mut b = std::ptr::null_mut();
        let rc = unsafe { git_blob_lookup(&mut b, self.0, id) };
        git_clear(rc);
        if rc != 0 {
            return None;
        }
        let v = unsafe { std::slice::from_raw_parts(git_blob_rawcontent(b), git_blob_rawsize(b) as usize).to_vec() };
        unsafe { git_blob_free(b) };
        Some(v)
    }
    /// Every object: (kind, id) with kind c, t, b or g.
    pub fn objects(&self) -> Vec<(char, Oid)> {
        clear();
        if unsafe { bw_objects(self.0) } < 0 {
            last_error();
        }
        text()
            .lines()
            .filter_map(|l| {
                let (k, h) = l.split_once(' ')?;
                Some((k.chars().next()?, Oid::parse(h)?))
            })
            .collect()
    }
    /// (blob, path) for the files of a commit or tree.
    pub fn tree_files(&self, id: &Oid) -> Vec<(Oid, String)> {
        clear();
        if unsafe { bw_tree_files(self.0, id) } < 0 {
            last_error();
        }
        text()
            .lines()
            .filter_map(|l| {
                let (h, p) = l.split_once('\t')?;
                Some((Oid::parse(h)?, p.to_string()))
            })
            .collect()
    }

    /// Makes a commit from the index; moves `update` ("HEAD") to it.
    pub fn commit_index(&self, update: Option<&str>, msg: &str, parents: &[Oid], author: Option<(&str, &str, i64)>) -> R<Oid> {
        let tree = self.write_tree()?;
        self.commit_tree(update, msg, &tree, parents, author)
    }
    pub fn commit_tree(&self, update: Option<&str>, msg: &str, tree: &Oid, parents: &[Oid], author: Option<(&str, &str, i64)>) -> R<Oid> {
        unsafe {
            let mut me: *mut Sig = std::ptr::null_mut();
            ok(git_signature_default(&mut me, self.0))?;
            let (an, ae) = author.map(|a| (cs(a.0), cs(a.1))).unwrap_or_else(|| (cs(""), cs("")));
            let asig = author.map(|a| Sig { name: an.as_ptr(), email: ae.as_ptr(), time: a.2, offset: 0, sign: b'+' as c_char });
            let mut t = std::ptr::null_mut();
            ok(git_tree_lookup(&mut t, self.0, tree))?;
            let mut ps = vec![];
            for p in parents {
                let mut c = std::ptr::null_mut();
                if git_commit_lookup(&mut c, self.0, p) < 0 {
                    for c in ps {
                        git_commit_free(c);
                    }
                    git_tree_free(t);
                    git_signature_free(me);
                    return Err(last_error());
                }
                ps.push(c);
            }
            let mut id = Oid::default();
            let u = update.map(cs);
            let rc = git_commit_create(
                &mut id,
                self.0,
                u.as_ref().map_or(std::ptr::null(), |u| u.as_ptr()),
                asig.as_ref().map_or(me as *const Sig, |s| s as *const Sig),
                me,
                std::ptr::null(),
                cs(msg).as_ptr(),
                t,
                ps.len(),
                ps.as_ptr(),
            );
            for c in ps {
                git_commit_free(c);
            }
            git_tree_free(t);
            git_signature_free(me);
            ok(rc)?;
            Ok(id)
        }
    }

    // ---------- index ----------

    fn index<T>(&self, f: impl FnOnce(P) -> R<T>) -> R<T> {
        let mut i = std::ptr::null_mut();
        ok(unsafe { git_repository_index(&mut i, self.0) })?;
        unsafe { git_index_read(i, 0) };
        let r = f(i);
        unsafe { git_index_free(i) };
        r
    }
    /// `git add` of pathspecs (new, changed and deleted files); returns how
    /// many index entries the specs matched.
    pub fn add(&self, specs: &[String], update_only: bool) -> R<usize> {
        let p = paths(specs);
        let a = strarray(&p);
        let mut n: usize = 0;
        self.index(|i| unsafe {
            if !update_only {
                ok(git_index_add_all(i, &a, 0, count_cb as P, &mut n as *mut usize as P))?;
            }
            ok(git_index_update_all(i, &a, count_cb as P, &mut n as *mut usize as P))?;
            ok(git_index_write(i))?;
            Ok(n)
        })
    }
    pub fn index_remove(&self, path: &str) -> R<()> {
        self.index(|i| unsafe {
            ok(git_index_remove_bypath(i, cs(path).as_ptr()))?;
            ok(git_index_write(i))?;
            Ok(())
        })
    }
    pub fn index_add_path(&self, path: &str) -> R<()> {
        self.index(|i| unsafe {
            ok(git_index_add_bypath(i, cs(path).as_ptr()))?;
            ok(git_index_write(i))?;
            Ok(())
        })
    }
    pub fn write_tree(&self) -> R<Oid> {
        self.index(|i| unsafe {
            let mut t = Oid::default();
            ok(git_index_write_tree(&mut t, i))?;
            Ok(t)
        })
    }
    /// Sets the index to a commit's tree.
    pub fn read_tree(&self, commit: &Oid) -> R<()> {
        let c = self.commit(commit)?;
        self.index(|i| unsafe {
            let mut t = std::ptr::null_mut();
            ok(git_tree_lookup(&mut t, self.0, &c.tree))?;
            let rc = git_index_read_tree(i, t);
            git_tree_free(t);
            ok(rc)?;
            ok(git_index_write(i))?;
            Ok(())
        })
    }
    pub fn has_conflicts(&self) -> bool {
        self.index(|i| Ok(unsafe { git_index_has_conflicts(i) } == 1)).unwrap_or(false)
    }
    /// (stage, mode, blob, path) for each index entry.
    pub fn index_entries(&self) -> Vec<(u32, u32, Oid, String)> {
        clear();
        if unsafe { bw_index(self.0) } < 0 {
            last_error();
        }
        text()
            .lines()
            .filter_map(|l| {
                let (a, p) = l.split_once('\t')?;
                let mut w = a.split(' ');
                let stage = w.next()?.parse().ok()?;
                let mode = u32::from_str_radix(w.next()?, 8).ok()?;
                let id = Oid::parse(w.next()?)?;
                Some((stage, mode, id, p.to_string()))
            })
            .collect()
    }

    // ---------- working tree ----------

    /// (status flags, path) for files that differ, are new, or conflict.
    pub fn status(&self) -> Vec<(u32, String)> {
        clear();
        if unsafe { bw_status(self.0) } < 0 {
            last_error();
            return vec![];
        }
        text()
            .lines()
            .filter_map(|l| {
                let (f, p) = l.split_once('\t')?;
                Some((f.parse().ok()?, p.to_string()))
            })
            .collect()
    }
    pub fn conflicts(&self) -> Vec<String> {
        clear();
        unsafe { bw_conflicts(self.0) };
        text().lines().map(|s| s.to_string()).collect()
    }
    /// Checks out a commit (or the index); Err lists the files in the way
    /// (or carries libgit2's message when it is something else).
    pub fn checkout(&self, id: Option<&Oid>, strategy: u32, specs: &[String]) -> Result<(), (Vec<String>, String)> {
        clear();
        let p = paths(specs);
        let rc = unsafe {
            bw_checkout(self.0, id.map_or(std::ptr::null(), |i| i as *const Oid), strategy, p.ptrs.as_ptr(), p.ptrs.len())
        };
        let files: Vec<String> = text().lines().map(|s| s.to_string()).collect();
        if rc < 0 {
            return Err((files, last_error()));
        }
        Ok(())
    }
    /// `git reset` of HEAD (and the branch): 1 soft, 2 mixed, 3 hard.
    pub fn reset(&self, to: &Oid, kind: c_int) -> R<()> {
        let mut o = std::ptr::null_mut();
        ok(unsafe { git_object_lookup(&mut o, self.0, to, OBJ_COMMIT) })?;
        let rc = unsafe { git_reset(self.0, o, kind, std::ptr::null_mut()) };
        unsafe { git_object_free(o) };
        ok(rc)?;
        Ok(())
    }
    /// Sets index entries to their state in a commit (None: remove them).
    pub fn reset_paths(&self, to: Option<&Oid>, specs: &[String]) -> R<()> {
        let p = paths(specs);
        let a = strarray(&p);
        let mut o = std::ptr::null_mut();
        if let Some(id) = to {
            ok(unsafe { git_object_lookup(&mut o, self.0, id, OBJ_COMMIT) })?;
        }
        let rc = unsafe { git_reset_default(self.0, o, &a) };
        if !o.is_null() {
            unsafe { git_object_free(o) };
        }
        ok(rc)?;
        Ok(())
    }

    // ---------- merging ----------

    /// Merges into the index and files; Ok(conflicted files), or Err with
    /// the files in the way.
    pub fn merge(&self, id: &Oid) -> Result<Vec<String>, (Vec<String>, String)> {
        clear();
        let rc = unsafe { bw_merge(self.0, id) };
        split_conflicts(rc)
    }
    pub fn cherrypick(&self, id: &Oid, mainline: u32, revert: bool) -> Result<Vec<String>, (Vec<String>, String)> {
        clear();
        let rc = unsafe { bw_cherrypick(self.0, id, mainline, revert as c_int) };
        split_conflicts(rc)
    }
    pub fn merge_base(&self, a: &Oid, b: &Oid) -> Option<Oid> {
        let mut o = Oid::default();
        let rc = unsafe { git_merge_base(&mut o, self.0, a, b) };
        git_clear(rc);
        (rc == 0).then_some(o)
    }
    pub fn descends(&self, commit: &Oid, ancestor: &Oid) -> bool {
        let rc = unsafe { git_graph_descendant_of(self.0, commit, ancestor) };
        git_clear(rc);
        rc == 1
    }
    pub fn ahead_behind(&self, a: &Oid, b: &Oid) -> (usize, usize) {
        let (mut x, mut y) = (0, 0);
        let rc = unsafe { git_graph_ahead_behind(&mut x, &mut y, self.0, a, b) };
        git_clear(rc);
        (x, y)
    }

    // ---------- tags ----------

    pub fn tag(&self, name: &str, at: &Oid, msg: Option<&str>, force: bool) -> R<()> {
        let mut o = std::ptr::null_mut();
        ok(unsafe { git_object_lookup(&mut o, self.0, at, OBJ_ANY) })?;
        let mut id = Oid::default();
        let rc = match msg {
            None => unsafe { git_tag_create_lightweight(&mut id, self.0, cs(name).as_ptr(), o, force as c_int) },
            Some(m) => unsafe {
                let mut me: *mut Sig = std::ptr::null_mut();
                git_signature_default(&mut me, self.0);
                let rc = git_tag_create(&mut id, self.0, cs(name).as_ptr(), o, me, cs(m).as_ptr(), force as c_int);
                git_signature_free(me);
                rc
            },
        };
        unsafe { git_object_free(o) };
        ok(rc)?;
        Ok(())
    }
    pub fn describe(&self, id: &Oid, tags: bool, always: bool) -> R<String> {
        clear();
        ok(unsafe { bw_describe(self.0, id, tags as c_int, always as c_int) })?;
        Ok(text())
    }

    // ---------- reflog ----------

    /// (new id, message), newest first.
    pub fn reflog(&self, name: &str) -> Vec<(Oid, String)> {
        let mut l = std::ptr::null_mut();
        if unsafe { git_reflog_read(&mut l, self.0, cs(name).as_ptr()) } < 0 {
            last_error();
            return vec![];
        }
        let n = unsafe { git_reflog_entrycount(l) };
        let v = (0..n)
            .map(|i| unsafe {
                let e = git_reflog_entry_byindex(l, i);
                (oid_at(git_reflog_entry_id_new(e)), st(git_reflog_entry_message(e)))
            })
            .collect();
        unsafe { git_reflog_free(l) };
        v
    }

    // ---------- stash ----------

    pub fn stash_save(&self, msg: Option<&str>, untracked: bool) -> R<()> {
        let m = msg.map(cs);
        ok(unsafe { bw_stash_save(self.0, m.as_ref().map_or(std::ptr::null(), |m| m.as_ptr()), if untracked { 2 } else { 0 }) })?;
        Ok(())
    }
    pub fn stash_apply(&self, i: usize, pop: bool, index: bool) -> Result<Vec<String>, (Vec<String>, String)> {
        clear();
        let rc = unsafe { bw_stash_apply(self.0, i, pop as c_int, index as c_int) };
        split_conflicts(rc)
    }
    /// (index, id, message), newest first.
    pub fn stashes(&self) -> Vec<(usize, Oid, String)> {
        clear();
        if unsafe { bw_stash_list(self.0) } < 0 {
            last_error();
        }
        text()
            .lines()
            .filter_map(|l| {
                let mut w = l.splitn(3, '\t');
                Some((w.next()?.parse().ok()?, Oid::parse(w.next()?)?, w.next()?.to_string()))
            })
            .collect()
    }

    pub fn stash_drop(&self, i: usize) -> R<()> {
        ok(unsafe { bw_stash_drop(self.0, i) })?;
        Ok(())
    }

    /// (mode, kind, id, name) of a tree's own entries.
    pub fn ls_tree(&self, id: &Oid) -> Vec<(u32, String, Oid, String)> {
        clear();
        if unsafe { bw_ls_tree(self.0, id) } < 0 {
            last_error();
        }
        text()
            .lines()
            .filter_map(|l| {
                let (a, n) = l.split_once('\t')?;
                let mut w = a.split(' ');
                let mode = u32::from_str_radix(w.next()?, 8).ok()?;
                let kind = w.next()?.to_string();
                Some((mode, kind, Oid::parse(w.next()?)?, n.to_string()))
            })
            .collect()
    }

    /// Deletes a branch and its config ("origin/x" with remote).
    pub fn delete_branch(&self, name: &str, remote: bool) -> R<()> {
        ok(unsafe { bw_branch_delete(self.0, cs(name).as_ptr(), remote as c_int) })?;
        Ok(())
    }

    // ---------- diffs ----------

    /// See bw_diff in shim.c for modes and kinds.
    pub fn diff(&self, mode: i32, a: Option<&Oid>, b: Option<&Oid>, what: i32, specs: &[String]) -> R<String> {
        clear();
        let p = paths(specs);
        let mut out = Oid::default();
        ok(unsafe {
            bw_diff(
                self.0,
                mode,
                a.map_or(std::ptr::null(), |x| x as *const Oid),
                b.map_or(std::ptr::null(), |x| x as *const Oid),
                what,
                p.ptrs.as_ptr(),
                p.ptrs.len(),
                &mut out,
            )
        })?;
        Ok(text())
    }
    /// What a commit changes, as git patch-id sees it.
    pub fn patch_id(&self, c: &Oid) -> Option<Oid> {
        let k = self.commit(c).ok()?;
        let parent = k.parents.first().copied();
        clear();
        let mut out = Oid::default();
        let rc = unsafe {
            bw_diff(self.0, 2, parent.as_ref().map_or(std::ptr::null(), |x| x as *const Oid), c, 4, std::ptr::null(), 0, &mut out)
        };
        git_clear(rc);
        (rc == 0).then_some(out)
    }

    // ---------- config ----------

    fn config<T>(&self, f: impl FnOnce(P) -> R<T>) -> R<T> {
        let mut c = std::ptr::null_mut();
        ok(unsafe { git_repository_config(&mut c, self.0) })?;
        let r = f(c);
        unsafe { git_config_free(c) };
        r
    }
    pub fn get_config(&self, name: &str) -> Option<String> {
        self.config(|c| {
            let mut b = Buf { ptr: std::ptr::null_mut(), reserved: 0, size: 0 };
            let rc = unsafe { git_config_get_string_buf(&mut b, c, cs(name).as_ptr()) };
            git_clear(rc);
            Ok((rc == 0).then(|| unsafe { take_buf(b) }))
        })
        .ok()
        .flatten()
    }
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        let v = self.get_config(name)?.to_ascii_lowercase();
        match v.as_str() {
            "true" | "yes" | "on" | "1" => Some(true),
            "false" | "no" | "off" | "0" | "" => Some(false),
            _ => None,
        }
    }
    /// Sets a value in this repository's own config.
    pub fn set_config(&self, name: &str, value: Option<&str>) -> R<()> {
        set_config_file(&format!("{}/config", self.commondir()), name, value)
    }

    // ---------- remotes ----------

    pub fn add_remote(&self, name: &str, url: &str) -> R<()> {
        let mut rm = std::ptr::null_mut();
        ok(unsafe { git_remote_create(&mut rm, self.0, cs(name).as_ptr(), cs(url).as_ptr()) })?;
        unsafe { git_remote_free(rm) };
        Ok(())
    }
    pub fn remove_remote(&self, name: &str) -> R<()> {
        ok(unsafe { git_remote_delete(self.0, cs(name).as_ptr()) })?;
        Ok(())
    }
    pub fn remotes(&self) -> Vec<String> {
        let mut a = StrArray { strings: std::ptr::null_mut(), count: 0 };
        if unsafe { git_remote_list(&mut a, self.0) } < 0 {
            last_error();
            return vec![];
        }
        unsafe { take_strarray(a) }
    }
    pub fn remote_url(&self, name: &str) -> Option<String> {
        let mut rm = std::ptr::null_mut();
        let rc = unsafe { git_remote_lookup(&mut rm, self.0, cs(name).as_ptr()) };
        git_clear(rc);
        if rc != 0 {
            return None;
        }
        let u = unsafe { st(git_remote_url(rm)) };
        unsafe { git_remote_free(rm) };
        Some(u)
    }
    pub fn fetch(&self, remote: &str, prune: bool, tags: bool) -> R<()> {
        ok(unsafe { bw_fetch(self.0, cs(remote).as_ptr(), prune as c_int, tags as c_int) })?;
        Ok(())
    }
    /// Pushes refspecs; Ok lists (ref, why) for refs the remote turned down.
    pub fn push(&self, remote: &str, specs: &[String]) -> R<Vec<(String, String)>> {
        clear();
        let p = paths(specs);
        let rc = unsafe { bw_push(self.0, cs(remote).as_ptr(), p.ptrs.as_ptr(), p.ptrs.len()) };
        let t = text();
        ok(rc)?;
        Ok(t.lines()
            .filter_map(|l| l.split_once('\t').map(|(a, b)| (a.to_string(), b.to_string())))
            .collect())
    }

    // ---------- worktrees ----------

    pub fn worktree_add(&self, name: &str, path: &str, branch: Option<&str>) -> R<()> {
        let b = branch.map(cs);
        ok(unsafe {
            bw_worktree_add(self.0, cs(name).as_ptr(), cs(path).as_ptr(), b.as_ref().map_or(std::ptr::null(), |b| b.as_ptr()))
        })?;
        Ok(())
    }
    /// (name, path) of each linked worktree.
    pub fn worktrees(&self) -> Vec<(String, String)> {
        clear();
        if unsafe { bw_worktrees(self.0) } < 0 {
            last_error();
        }
        text()
            .lines()
            .filter_map(|l| l.split_once('\t').map(|(a, b)| (a.to_string(), b.trim_end_matches('/').to_string())))
            .collect()
    }
    pub fn worktree_prune(&self, name: &str) -> R<()> {
        ok(unsafe { bw_worktree_prune(self.0, cs(name).as_ptr()) })?;
        Ok(())
    }
}

extern "C" fn count_cb(_path: *const c_char, _spec: *const c_char, n: *mut usize) -> c_int {
    unsafe { *n += 1 };
    0
}

fn split_conflicts(rc: c_int) -> Result<Vec<String>, (Vec<String>, String)> {
    let t = text();
    let (before, after) = match t.find("--conflicts\n") {
        Some(i) => (&t[..i], &t[i + 12..]),
        None => (t.as_str(), ""),
    };
    let lines = |s: &str| s.lines().map(|x| x.to_string()).collect::<Vec<_>>();
    if rc < 0 {
        return Err((lines(before), last_error()));
    }
    let mut c = lines(after);
    c.dedup();
    Ok(c)
}

pub fn set_config_file(path: &str, name: &str, value: Option<&str>) -> R<()> {
    let mut c = std::ptr::null_mut();
    ok(unsafe { git_config_open_ondisk(&mut c, cs(path).as_ptr()) })?;
    let rc = match value {
        Some(v) => unsafe { git_config_set_string(c, cs(name).as_ptr(), cs(v).as_ptr()) },
        None => unsafe { git_config_delete_entry(c, cs(name).as_ptr()) },
    };
    unsafe { git_config_free(c) };
    ok(rc)?;
    Ok(())
}

pub fn get_config_file(path: &str, name: &str) -> Option<String> {
    let mut c = std::ptr::null_mut();
    if unsafe { git_config_open_ondisk(&mut c, cs(path).as_ptr()) } < 0 {
        last_error();
        return None;
    }
    let mut b = Buf { ptr: std::ptr::null_mut(), reserved: 0, size: 0 };
    let rc = unsafe { git_config_get_string_buf(&mut b, c, cs(name).as_ptr()) };
    git_clear(rc);
    let v = (rc == 0).then(|| unsafe { take_buf(b) });
    unsafe { git_config_free(c) };
    v
}
