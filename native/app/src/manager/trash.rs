//! The trash's three kinds, and the three different writes behind them.
//!
//! Split from the parent for the usual reason (the 800-line ceiling) and one better
//! one: restore and purge are the only actions in this window that touch **two
//! different stores**. A note is a row in `notes`, a capture is a row in
//! `inbox_items` plus files under `assets/`, and a script is a directory under
//! `%APPDATA%\scriptmgr\trash\scripts`. Keeping them in one file makes the
//! difference visible instead of hiding it behind a trait three types implement.
//!
//! Everything here is keyed by [`RowKind`], which the row carried and the action
//! preserved. Nothing here reads view state: the pane says *what* was asked for, and
//! these answer *how*.

use hub_core::script_trash;
use hub_ui::manager::RowKind;

use super::Shell;

impl Shell {
    /// 恢复选中: one write per kind, one count in the status line.
    pub(super) fn restore_rows(&mut self, ctx: &egui::Context, kind: RowKind, ids: &[String]) {
        let count = match kind {
            RowKind::Note => self.with_store(|store| store.restore_notes(ids)).unwrap_or(0),
            RowKind::Inbox => self.with_store(|store| store.restore_inbox(ids)).unwrap_or(0),
            RowKind::Script => self.restore_scripts(ids),
        };
        let noun = match kind {
            RowKind::Note | RowKind::Inbox => "条",
            RowKind::Script => "个脚本",
        };
        self.state.status = if count > 0 {
            format!("已恢复 {count} {noun}")
        } else {
            format!("没有可恢复的{}", unit(kind))
        };
        // The count and the ids: the first is what the user was told, the second is
        // what a post-mortem needs when a row did not come back.
        self.log.mark(
            0,
            "manager_restore_many",
            &format!(
                "count={count} kind={} ids={}",
                kind.id(),
                ids.join(",")
            ),
        );
        self.state.checked.clear();
        self.reload();
        self.refresh(ctx);
    }

    /// 恢复 on one row.
    pub(super) fn restore_one(&mut self, ctx: &egui::Context, kind: RowKind, id: &str) {
        let (ok, why) = match kind {
            RowKind::Note => (
                self.with_store(|store| store.restore_note(id)) == Some(true),
                String::new(),
            ),
            RowKind::Inbox => {
                let ids = [id.to_string()];
                let changed = self
                    .with_store(|store| store.restore_inbox(&ids))
                    .unwrap_or(0);
                (changed > 0, String::new())
            }
            RowKind::Script => match script_trash::restore(&self.script_trash, id) {
                Ok(_) => (true, String::new()),
                // A refusal is the interesting case: the original path is occupied,
                // so the user has to be told *what* is in the way, not just "失败".
                Err(error) => (false, format!(" reason=\"{error}\"")),
            },
        };
        self.state.status = if ok {
            "已恢复".to_string()
        } else if why.is_empty() {
            "恢复失败".to_string()
        } else {
            "原位置已有同名文件，未恢复".to_string()
        };
        self.log.mark(
            0,
            "manager_restore",
            &format!("kind={} id={id} ok={ok}{why}", kind.id()),
        );
        self.reload();
        self.refresh(ctx);
    }

    /// 彻底删除 on one row. Only the dialog's yes reaches this.
    pub(super) fn purge_one(&mut self, ctx: &egui::Context, kind: RowKind, id: &str) {
        let (ok, why) = match kind {
            RowKind::Note => (
                self.with_store(|store| store.purge_note(id)) == Some(true),
                String::new(),
            ),
            RowKind::Inbox => (
                self.with_store(|store| store.purge_inbox(id)) == Some(true),
                String::new(),
            ),
            RowKind::Script => match script_trash::purge(&self.script_trash, id) {
                Ok(found) => (found, String::new()),
                Err(error) => (false, format!(" reason=\"{error}\"")),
            },
        };
        self.state.status = if ok {
            "已彻底删除".to_string()
        } else {
            "删除失败".to_string()
        };
        // Permanent, so it is logged as such: "where did my script go" needs an
        // answer that outlives the session.
        self.log.mark(
            0,
            "manager_purge",
            &format!("kind={} id={id} ok={ok} permanent=true{why}", kind.id()),
        );
        self.reload();
        self.refresh(ctx);
    }

    /// 清空 this sub-tab. Never the whole trash: the button says which kind it takes.
    pub(super) fn empty_kind(&mut self, ctx: &egui::Context, kind: RowKind) {
        let (count, why) = match kind {
            RowKind::Note => (
                self.with_store(|store| store.purge_trashed()).unwrap_or(0),
                String::new(),
            ),
            RowKind::Inbox => (
                self.with_store(|store| store.purge_trashed_inbox()).unwrap_or(0),
                String::new(),
            ),
            RowKind::Script => match script_trash::empty(&self.script_trash) {
                Ok(count) => (count, String::new()),
                Err(error) => (0, format!(" reason=\"{error}\"")),
            },
        };
        self.state.status = if count > 0 {
            format!("已清空 {count} 条")
        } else {
            "没有可清空的".to_string()
        };
        self.log.mark(
            0,
            "manager_empty_trash",
            &format!("count={count} kind={} permanent=true{why}", kind.id()),
        );
        self.state.checked.clear();
        self.reload();
        self.refresh(ctx);
    }

    /// Delete what has sat in the trash past the retention window, for every kind.
    ///
    /// The panel has said "保留 30 天" since it got a description, and the sentence is
    /// only true if something acts on it. It runs when the trash is opened -- which
    /// is when the sentence is being read -- and it reports what it did: a silent
    /// delete on a tab click is exactly what makes a user distrust the trash.
    pub(super) fn expire_trash(&mut self) {
        let now = hub_core::clock::now_iso();
        let days = hub_core::store::TRASH_RETENTION_DAYS;
        let sweep = self.with_store(|store| store.purge_expired(&now, days));
        let swept = sweep.unwrap_or_default();
        let scripts = script_trash::purge_expired(&self.script_trash, &now, days).unwrap_or(0);
        if swept.total() == 0 && scripts == 0 {
            return;
        }
        self.state.status = format!(
            "已清理 {} 条超过 {days} 天的内容",
            swept.total() + scripts
        );
        self.log.mark(
            0,
            "manager_trash_expire",
            &format!(
                "days={days} notes={} inbox={} scripts={scripts}",
                swept.notes, swept.inbox
            ),
        );
        self.reload();
    }

    /// Every script id that came back, for the batch.
    ///
    /// Per entry, because a restore is a file move: one occupied destination must not
    /// stop the others, and the count has to be "what actually came back".
    fn restore_scripts(&mut self, ids: &[String]) -> usize {
        let mut restored = 0;
        for id in ids {
            match script_trash::restore(&self.script_trash, id) {
                Ok(_) => restored += 1,
                Err(error) => self.log.mark(
                    0,
                    "manager_restore_many",
                    &format!("kind=scripts id={id} skipped=\"{error}\""),
                ),
            }
        }
        restored
    }

}

/// The noun the status sentence uses for a kind. Chinese counts scripts with 个 and
/// rows with 条, and "没有可恢复的条" is not a sentence.
fn unit(kind: RowKind) -> &'static str {
    match kind {
        RowKind::Note => "笔记",
        RowKind::Inbox => "收集项",
        RowKind::Script => "脚本",
    }
}
