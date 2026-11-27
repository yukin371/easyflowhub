//! The panel's translation actions: ask, collect, write back, copy.
//!
//! Split out of `launcher.rs` when that file passed the guard's 800-line cap. The
//! seam is real rather than cosmetic: everything here is about one feature whose
//! state arrives asynchronously from another thread, while the rest of the shell is
//! about a window that moves and a list that is redrawn. The only thing they share
//! is `Shell`'s fields, which a child module can already see.
//!
//! ## The two clocks
//!
//! A translation has a request clock (`Engine`, on its own threads) and a paint
//! clock (the panel's frames). Nothing here blocks: `start` hands the job off and
//! the card says 正在翻译; `poll` is called once per frame from `frame` and does
//! nothing until an answer is there. That is why the panel stays responsive while a
//! slow provider thinks, and why closing it mid-flight costs nothing.

use egui::ViewportId;

use super::{viewport_id, Shell};
use hub_ui::launcher::Translate;

impl Shell {
    /// Ask the Relay to translate what the panel captured.
    ///
    /// The checks here are the same ones `hub_core::translation::body` makes, on
    /// purpose: they are cheap, and failing them before the spinner means the user
    /// reads "先选一段文字" instead of watching a spinner that has nothing to do.
    pub(super) fn start_translation(&mut self, ctx: &egui::Context) {
        let request = hub_core::translation::Request {
            model: self.translation.model.trim().to_string(),
            target_language: self.translation.target_language.trim().to_string(),
            text: self
                .state
                .selection
                .clone()
                .unwrap_or_default()
                .trim()
                .to_string(),
        };
        if let Err(problem) = hub_core::translation::body(&request) {
            self.fail_translation(problem, "preflight", None);
            return;
        }
        let chars = request.text.chars().count();
        let model = request.model.clone();
        let relay = self.translation.relay_url.trim().to_string();
        self.engine.start(&relay, request);
        self.state.translate = Some(Translate::Running);
        self.log.mark(
            0,
            "launcher_translate",
            &format!("mode=start chars={chars} model=\"{model}\" relay=\"{relay}\""),
        );
        ctx.request_repaint_of(viewport_id());
    }

    /// Take the answer if one has arrived. Returns whether the panel changed.
    pub(super) fn poll_translation(&mut self) -> bool {
        let Some(finished) = self.engine.poll() else {
            return false;
        };
        let ms = finished.elapsed.as_millis();
        match finished.result {
            Ok(reply) => {
                let chars = reply.text.chars().count();
                self.log.mark(
                    0,
                    "launcher_translate",
                    &format!("mode=done chars={chars} ms={ms} model=\"{}\"", reply.model),
                );
                self.state.translate = Some(Translate::Done { text: reply.text });
            }
            Err(problem) => self.fail_translation(problem, "request", Some(ms)),
        }
        true
    }

    /// Record a failure in both places it matters: the token for the log, the
    /// sentence for the user.
    fn fail_translation(
        &mut self,
        problem: hub_core::translation::Problem,
        stage: &str,
        ms: Option<u128>,
    ) {
        let ms = ms.map(|ms| format!(" ms={ms}")).unwrap_or_default();
        self.log.mark(
            0,
            "launcher_translate",
            &format!("mode=failed code={} stage={stage}{ms}", problem.code()),
        );
        self.state.translate = Some(Translate::Failed {
            message: problem.message().to_string(),
        });
    }

    /// Write the translation back over the text it came from.
    ///
    /// The panel is dismissed first: `selection::replace` returns the foreground to
    /// the target window, and a panel still on screen would be the thing the paste
    /// lands in. The paste itself then runs on **its own thread**, which is not an
    /// optimisation -- the target is a note window on *this* thread, and a blocking
    /// paste here would hold its message loop exactly while it needs to process the
    /// keystroke. See `hub_win::selection::replace`.
    ///
    /// On failure the translation is copied instead: the panel is already gone, so
    /// there is nowhere to show an error, and losing work the user watched arrive is
    /// worse than an undocumented clipboard entry.
    pub(super) fn replace_selection(&mut self, ctx: &egui::Context) {
        let Some(Translate::Done { text }) = self.state.translate.clone() else {
            return;
        };
        let target = self.handle.previous();
        self.dismiss("translate:replace");
        let log = self.log.clone();
        let spawned = std::thread::Builder::new()
            .name("translate-replace".to_string())
            .spawn(move || {
                let chars = text.chars().count();
                match hub_win::selection::replace(target, &text) {
                    Ok(()) => log.mark(
                        0,
                        "launcher_translate_replace",
                        &format!("target={target} chars={chars} ok=true"),
                    ),
                    Err(error) => {
                        let copied = hub_win::clipboard::set_text(&text);
                        log.mark(
                            0,
                            "launcher_translate_replace",
                            &format!(
                                "target={target} chars={chars} ok=false copied={copied} error=\"{error}\""
                            ),
                        );
                    }
                }
            });
        // Nothing to reap if the thread could not start, and the user's next move is
        // the same either way (`复制`), but silence here would be a replace that
        // vanished without a line in the log.
        if let Err(error) = spawned {
            self.log.mark(
                0,
                "launcher_translate_replace",
                &format!("target={target} ok=false error=\"spawn: {error}\""),
            );
        }
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    /// Put the translation on the clipboard and keep the panel up.
    ///
    /// Kept open on purpose: copying is the action a user takes when they want the
    /// text *somewhere else*, and closing the panel would take away the only place
    /// the translation is currently visible.
    pub(super) fn copy_translation(&mut self) {
        let Some(Translate::Done { text }) = self.state.translate.clone() else {
            return;
        };
        let copied = hub_win::clipboard::set_text(&text);
        self.log.mark(
            0,
            "launcher_translate_copy",
            &format!("chars={} ok={copied}", text.chars().count()),
        );
    }
}
