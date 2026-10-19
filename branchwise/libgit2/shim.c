/* The game's glue to libgit2: the calls that take option structs or
 * callbacks, flattened into plain arguments. Results that are lists come
 * back as text lines in one buffer (bw_text). */

#include <git2.h>
#include <git2/sys/repository.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* WASI has no users or process groups; libgit2 asks for them */
int getppid(void) { return 1; }
int getpgid(int pid) { (void)pid; return 1; }
int getsid(int pid) { (void)pid; return 1; }
unsigned getuid(void) { return 0; }
unsigned getgid(void) { return 0; }
unsigned geteuid(void) { return 0; }

static char *buf;
static size_t len, cap;

static void put(const char *s, size_t n)
{
	if (len + n + 1 > cap) {
		cap = (len + n + 1) * 2;
		buf = realloc(buf, cap);
	}
	memcpy(buf + len, s, n);
	len += n;
	buf[len] = 0;
}

static void putf(const char *fmt, ...)
{
	char tmp[1024];
	va_list ap;
	va_start(ap, fmt);
	int n = vsnprintf(tmp, sizeof tmp, fmt, ap);
	va_end(ap);
	if (n > 0)
		put(tmp, (size_t)n < sizeof tmp ? (size_t)n : sizeof tmp - 1);
}

const char *bw_text(void) { return buf ? buf : ""; }
size_t bw_text_len(void) { return len; }
void bw_text_clear(void) { len = 0; if (buf) buf[0] = 0; }

static void hex(const git_oid *id)
{
	char s[GIT_OID_SHA1_HEXSIZE + 1];
	git_oid_tostr(s, sizeof s, id);
	put(s, strlen(s));
}

static git_strarray paths(const char **p, size_t n)
{
	git_strarray a = { (char **)p, n };
	return a;
}

/* ---------- status ---------- */

/* "flags\tpath" for every changed, untracked or conflicted file */
int bw_status(git_repository *r)
{
	git_status_options o = GIT_STATUS_OPTIONS_INIT;
	git_status_list *l;
	size_t i, n;
	o.show = GIT_STATUS_SHOW_INDEX_AND_WORKDIR;
	o.flags = GIT_STATUS_OPT_INCLUDE_UNTRACKED | GIT_STATUS_OPT_RECURSE_UNTRACKED_DIRS |
		GIT_STATUS_OPT_SORT_CASE_SENSITIVELY;
	if (git_status_list_new(&l, r, &o) < 0)
		return -1;
	n = git_status_list_entrycount(l);
	for (i = 0; i < n; i++) {
		const git_status_entry *e = git_status_byindex(l, i);
		const git_diff_delta *d = e->head_to_index ? e->head_to_index : e->index_to_workdir;
		const char *p = d->new_file.path ? d->new_file.path : d->old_file.path;
		putf("%u\t%s\n", (unsigned)e->status, p);
	}
	git_status_list_free(l);
	return 0;
}

/* "path" for each conflicted file */
int bw_conflicts(git_repository *r)
{
	git_index *idx;
	git_index_conflict_iterator *it;
	const git_index_entry *a, *o, *t;
	if (git_repository_index(&idx, r) < 0)
		return -1;
	if (git_index_conflict_iterator_new(&it, idx) < 0) {
		git_index_free(idx);
		return -1;
	}
	while (git_index_conflict_next(&a, &o, &t, it) == 0) {
		const char *p = o ? o->path : t ? t->path : a->path;
		putf("%s\n", p);
	}
	git_index_conflict_iterator_free(it);
	git_index_free(idx);
	return 0;
}

/* "stage oid\tpath" for each index entry */
int bw_index(git_repository *r)
{
	git_index *idx;
	size_t i, n;
	if (git_repository_index(&idx, r) < 0)
		return -1;
	n = git_index_entrycount(idx);
	for (i = 0; i < n; i++) {
		const git_index_entry *e = git_index_get_byindex(idx, i);
		putf("%d %o ", git_index_entry_stage(e), e->mode);
		hex(&e->id);
		putf("\t%s\n", e->path);
	}
	git_index_free(idx);
	return 0;
}

/* ---------- checkout ---------- */

static int notify(git_checkout_notify_t why, const char *path, const git_diff_file *b,
	const git_diff_file *t, const git_diff_file *w, void *p)
{
	(void)b; (void)t; (void)w; (void)p;
	if (why == GIT_CHECKOUT_NOTIFY_CONFLICT)
		putf("%s\n", path);
	return 0;
}

static void notify_on(git_checkout_options *co)
{
	co->notify_flags = GIT_CHECKOUT_NOTIFY_CONFLICT;
	co->notify_cb = notify;
}

/* Checks out a commit or tree (or the index, when id is NULL); files that
 * stop it are listed. */
int bw_checkout(git_repository *r, const git_oid *id, unsigned strategy, const char **p, size_t n)
{
	git_checkout_options co = GIT_CHECKOUT_OPTIONS_INIT;
	git_object *obj = NULL;
	int err;
	co.checkout_strategy = strategy;
	notify_on(&co);
	if (n)
		co.paths = paths(p, n);
	if (!id)
		return git_checkout_index(r, NULL, &co);
	if (git_object_lookup(&obj, r, id, GIT_OBJECT_ANY) < 0)
		return -1;
	err = git_checkout_tree(r, obj, &co);
	git_object_free(obj);
	return err;
}

/* ---------- merging ---------- */

static void list_conflicts(git_repository *r)
{
	put("--conflicts\n", 12);
	bw_conflicts(r);
}

int bw_merge(git_repository *r, const git_oid *id)
{
	git_annotated_commit *ac;
	git_merge_options mo = GIT_MERGE_OPTIONS_INIT;
	git_checkout_options co = GIT_CHECKOUT_OPTIONS_INIT;
	int err;
	if (git_annotated_commit_lookup(&ac, r, id) < 0)
		return -1;
	co.checkout_strategy = GIT_CHECKOUT_SAFE | GIT_CHECKOUT_ALLOW_CONFLICTS;
	notify_on(&co);
	err = git_merge(r, (const git_annotated_commit **)&ac, 1, &mo, &co);
	git_annotated_commit_free(ac);
	if (err == 0)
		list_conflicts(r);
	return err;
}

int bw_cherrypick(git_repository *r, const git_oid *id, unsigned mainline, int revert)
{
	git_commit *c;
	int err;
	if (git_commit_lookup(&c, r, id) < 0)
		return -1;
	if (revert) {
		git_revert_options o = GIT_REVERT_OPTIONS_INIT;
		o.mainline = mainline;
		o.checkout_opts.checkout_strategy = GIT_CHECKOUT_SAFE | GIT_CHECKOUT_ALLOW_CONFLICTS;
		notify_on(&o.checkout_opts);
		err = git_revert(r, c, &o);
	} else {
		git_cherrypick_options o = GIT_CHERRYPICK_OPTIONS_INIT;
		o.mainline = mainline;
		o.checkout_opts.checkout_strategy = GIT_CHECKOUT_SAFE | GIT_CHECKOUT_ALLOW_CONFLICTS;
		notify_on(&o.checkout_opts);
		err = git_cherrypick(r, c, &o);
	}
	git_commit_free(c);
	if (err == 0)
		list_conflicts(r);
	return err;
}

/* ---------- stash ---------- */

int bw_stash_save(git_repository *r, const char *msg, unsigned flags)
{
	git_signature *s;
	git_oid id;
	int err;
	if (git_signature_default(&s, r) < 0)
		return -1;
	err = git_stash_save(&id, r, s, msg, flags);
	git_signature_free(s);
	return err;
}

int bw_stash_apply(git_repository *r, size_t i, int pop, int index)
{
	git_stash_apply_options o = GIT_STASH_APPLY_OPTIONS_INIT;
	int err;
	o.flags = index ? GIT_STASH_APPLY_REINSTATE_INDEX : 0;
	o.checkout_options.checkout_strategy = GIT_CHECKOUT_SAFE;
	notify_on(&o.checkout_options);
	err = pop ? git_stash_pop(r, i, &o) : git_stash_apply(r, i, &o);
	if (err == 0)
		list_conflicts(r);
	return err;
}

static int stash_cb(size_t i, const char *msg, const git_oid *id, void *p)
{
	(void)p;
	putf("%u\t", (unsigned)i);
	hex(id);
	putf("\t%s\n", msg);
	return 0;
}

int bw_stash_list(git_repository *r)
{
	return git_stash_foreach(r, stash_cb, NULL);
}

/* ---------- diffs ---------- */

static int line_cb(const git_diff_delta *d, const git_diff_hunk *h, const git_diff_line *l, void *p)
{
	(void)d; (void)h; (void)p;
	char o = l->origin;
	if (o == GIT_DIFF_LINE_ADDITION || o == GIT_DIFF_LINE_DELETION || o == GIT_DIFF_LINE_CONTEXT)
		put(&o, 1);
	put(l->content, l->content_len);
	if (l->content_len && l->content[l->content_len - 1] != '\n')
		put("\n\\ No newline at end of file\n", 29);
	return 0;
}

static int tree_of(git_tree **out, git_repository *r, const git_oid *id)
{
	git_object *o;
	int err;
	*out = NULL;
	if (!id)
		return 0;
	if (git_object_lookup(&o, r, id, GIT_OBJECT_ANY) < 0)
		return -1;
	err = git_object_peel((git_object **)out, o, GIT_OBJECT_TREE);
	git_object_free(o);
	return err;
}

/* mode 0: index to working tree; 1: tree a to index; 2: tree a to tree b;
 * 3: tree a to working tree. what 0: patch, 1: --stat, 2: short summary
 * (for commit), 3: names with status, 4: patch id into out */
int bw_diff(git_repository *r, int mode, const git_oid *a, const git_oid *b, int what,
	const char **p, size_t n, git_oid *out)
{
	git_diff_options o = GIT_DIFF_OPTIONS_INIT;
	git_diff *diff = NULL;
	git_tree *ta = NULL, *tb = NULL;
	int err = 0;
	if (n)
		o.pathspec = paths(p, n);
	if (tree_of(&ta, r, a) < 0 || tree_of(&tb, r, b) < 0)
		return -1;
	switch (mode) {
	case 0: err = git_diff_index_to_workdir(&diff, r, NULL, &o); break;
	case 1: err = git_diff_tree_to_index(&diff, r, ta, NULL, &o); break;
	case 2: err = git_diff_tree_to_tree(&diff, r, ta, tb, &o); break;
	default: err = git_diff_tree_to_workdir_with_index(&diff, r, ta, &o); break;
	}
	if (err == 0) {
		if (what == 0) {
			err = git_diff_print(diff, GIT_DIFF_FORMAT_PATCH, line_cb, NULL);
		} else if (what == 4) {
			err = git_diff_patchid(out, diff, NULL);
		} else if (what == 3) {
			size_t i, k = git_diff_num_deltas(diff);
			for (i = 0; i < k; i++) {
				const git_diff_delta *d = git_diff_get_delta(diff, i);
				putf("%c\t%s\n", git_diff_status_char(d->status), d->new_file.path);
			}
		} else {
			git_diff_stats *st;
			git_buf sb = GIT_BUF_INIT;
			err = git_diff_get_stats(&st, diff);
			if (err == 0) {
				git_diff_stats_format_t f = what == 1 ? GIT_DIFF_STATS_FULL :
					GIT_DIFF_STATS_SHORT | GIT_DIFF_STATS_INCLUDE_SUMMARY;
				if (git_diff_stats_files_changed(st) == 0 && what == 1)
					f = GIT_DIFF_STATS_NONE;
				err = git_diff_stats_to_buf(&sb, st, f, 48);
				if (err == 0)
					put(sb.ptr, sb.size);
				git_buf_dispose(&sb);
				git_diff_stats_free(st);
			}
		}
	}
	git_diff_free(diff);
	git_tree_free(ta);
	git_tree_free(tb);
	return err;
}

/* ---------- objects ---------- */

typedef struct {
	git_odb *odb;
} each_ctx;

static int each_cb(const git_oid *id, void *p)
{
	each_ctx *c = p;
	size_t sz;
	git_object_t t;
	if (git_odb_read_header(&sz, &t, c->odb, id) < 0)
		return 0;
	putf("%c ", t == GIT_OBJECT_COMMIT ? 'c' : t == GIT_OBJECT_TREE ? 't' :
		t == GIT_OBJECT_BLOB ? 'b' : 'g');
	hex(id);
	put("\n", 1);
	return 0;
}

/* "kind oid" for every object: c commit, t tree, b blob, g tag */
int bw_objects(git_repository *r)
{
	each_ctx c;
	int err;
	if (git_repository_odb(&c.odb, r) < 0)
		return -1;
	err = git_odb_foreach(c.odb, each_cb, &c);
	git_odb_free(c.odb);
	return err;
}

static int walk_cb(const char *root, const git_tree_entry *e, void *p)
{
	(void)p;
	if (git_tree_entry_type(e) != GIT_OBJECT_BLOB)
		return 0;
	hex(git_tree_entry_id(e));
	putf("\t%s%s\n", root, git_tree_entry_name(e));
	return 0;
}

/* "blob\tpath" for every file in a tree (or a commit's tree) */
int bw_tree_files(git_repository *r, const git_oid *id)
{
	git_tree *t;
	int err;
	if (tree_of(&t, r, id) < 0)
		return -1;
	err = git_tree_walk(t, GIT_TREEWALK_PRE, walk_cb, NULL);
	git_tree_free(t);
	return err;
}

/* "mode type oid\tname" for each entry of a tree (or a commit's tree) */
int bw_ls_tree(git_repository *r, const git_oid *id)
{
	git_tree *t;
	size_t i, n;
	if (tree_of(&t, r, id) < 0)
		return -1;
	n = git_tree_entrycount(t);
	for (i = 0; i < n; i++) {
		const git_tree_entry *e = git_tree_entry_byindex(t, i);
		putf("%06o %s ", git_tree_entry_filemode(e), git_object_type2string(git_tree_entry_type(e)));
		hex(git_tree_entry_id(e));
		putf("\t%s\n", git_tree_entry_name(e));
	}
	git_tree_free(t);
	return 0;
}

int bw_stash_drop(git_repository *r, size_t i)
{
	return git_stash_drop(r, i);
}

int bw_branch_delete(git_repository *r, const char *name, int remote)
{
	git_reference *ref;
	int err;
	if (git_branch_lookup(&ref, r, name, remote ? GIT_BRANCH_REMOTE : GIT_BRANCH_LOCAL) < 0)
		return -1;
	err = git_branch_delete(ref);
	git_reference_free(ref);
	return err;
}

/* ---------- remotes ---------- */

int bw_fetch(git_repository *r, const char *name, int prune, int tags)
{
	git_remote *rm;
	git_fetch_options o = GIT_FETCH_OPTIONS_INIT;
	int err;
	if (git_remote_lookup(&rm, r, name) < 0)
		return -1;
	o.prune = prune ? GIT_FETCH_PRUNE : GIT_FETCH_NO_PRUNE;
	o.download_tags = tags ? GIT_REMOTE_DOWNLOAD_TAGS_ALL : GIT_REMOTE_DOWNLOAD_TAGS_AUTO;
	err = git_remote_fetch(rm, NULL, &o, NULL);
	git_remote_free(rm);
	return err;
}

static int push_cb(const char *ref, const char *status, void *p)
{
	(void)p;
	if (status)
		putf("%s\t%s\n", ref, status);
	return 0;
}

/* Lists "ref\tproblem" for each ref the remote refused. */
int bw_push(git_repository *r, const char *name, const char **specs, size_t n)
{
	git_remote *rm;
	git_push_options o = GIT_PUSH_OPTIONS_INIT;
	git_strarray a = paths(specs, n);
	int err;
	if (git_remote_lookup(&rm, r, name) < 0)
		return -1;
	o.callbacks.push_update_reference = push_cb;
	err = git_remote_push(rm, &a, &o);
	git_remote_free(rm);
	return err;
}

int bw_clone(const char *url, const char *path, int bare)
{
	git_clone_options o = GIT_CLONE_OPTIONS_INIT;
	git_repository *r;
	int err;
	o.bare = bare;
	o.checkout_opts.checkout_strategy = GIT_CHECKOUT_SAFE;
	err = git_clone(&r, url, path, &o);
	if (err == 0)
		git_repository_free(r);
	return err;
}

/* ---------- worktrees ---------- */

int bw_worktree_add(git_repository *r, const char *name, const char *path, const char *branch)
{
	git_worktree_add_options o = GIT_WORKTREE_ADD_OPTIONS_INIT;
	git_reference *ref = NULL;
	git_worktree *wt;
	int err;
	if (branch && git_reference_lookup(&ref, r, branch) < 0)
		return -1;
	o.ref = ref;
	o.checkout_options.checkout_strategy = GIT_CHECKOUT_FORCE;
	err = git_worktree_add(&wt, r, name, path, &o);
	git_reference_free(ref);
	if (err == 0)
		git_worktree_free(wt);
	return err;
}

/* "name\tpath" for each linked worktree */
int bw_worktrees(git_repository *r)
{
	git_strarray a;
	size_t i;
	if (git_worktree_list(&a, r) < 0)
		return -1;
	for (i = 0; i < a.count; i++) {
		git_worktree *wt;
		if (git_worktree_lookup(&wt, r, a.strings[i]) < 0)
			continue;
		putf("%s\t%s\n", a.strings[i], git_worktree_path(wt));
		git_worktree_free(wt);
	}
	git_strarray_dispose(&a);
	return 0;
}

int bw_worktree_prune(git_repository *r, const char *name)
{
	git_worktree *wt;
	git_worktree_prune_options o = GIT_WORKTREE_PRUNE_OPTIONS_INIT;
	int err;
	if (git_worktree_lookup(&wt, r, name) < 0)
		return -1;
	o.flags = GIT_WORKTREE_PRUNE_VALID | GIT_WORKTREE_PRUNE_WORKING_TREE;
	err = git_worktree_prune(wt, &o);
	git_worktree_free(wt);
	return err;
}

/* ---------- describe ---------- */

int bw_describe(git_repository *r, const git_oid *id, int tags, int always)
{
	git_describe_options o = GIT_DESCRIBE_OPTIONS_INIT;
	git_describe_format_options f = GIT_DESCRIBE_FORMAT_OPTIONS_INIT;
	git_describe_result *res;
	git_object *obj;
	git_buf b = GIT_BUF_INIT;
	int err;
	o.describe_strategy = tags ? GIT_DESCRIBE_TAGS : GIT_DESCRIBE_DEFAULT;
	o.show_commit_oid_as_fallback = always;
	f.abbreviated_size = 4;
	if (git_object_lookup(&obj, r, id, GIT_OBJECT_COMMIT) < 0)
		return -1;
	err = git_describe_commit(&res, obj, &o);
	git_object_free(obj);
	if (err < 0)
		return err;
	err = git_describe_format(&b, res, &f);
	if (err == 0)
		put(b.ptr, b.size);
	git_buf_dispose(&b);
	git_describe_result_free(res);
	return err;
}
