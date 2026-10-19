# Translation

## Responsibility

`features/translation` owns the text translation request and response adapter.
It uses the existing ScriptMgr Relay runtime URL and OpenAI-compatible Chat
Completions interface. Provider credentials, routing and failover remain owned
by `scriptmgr-go/internal/relay`.

## Boundaries and conventions

- The caller supplies text, target language, model and an optional cancellation
  signal. This module does not read settings, clipboard or Tauri APIs.
- Relay URLs may end at the host or `/v1`; the adapter appends the completion
  path once. No API key is stored or sent by this feature.
- The runtime also accepts the existing `/relay/v1` alias. This is useful when
  a caller keeps the Relay prefix in its configured base URL; the adapter does
  not discover providers or rewrite routes itself.
- The request is a non-streaming OpenAI-compatible `POST /v1/chat/completions`
  with `model`, a low translation temperature, a system instruction and the
  source text as a separate user message. Relay owns provider credentials,
  route matching, failover and upstream authorization.
- Requests are non-streaming with a 60-second deadline including response
  parsing. Cancellation and timeout have distinct `TranslationError.code`s.
- Errors exposed to the UI are fixed Chinese messages; raw Relay/provider
  bodies, request errors and credentials must never be echoed.
- The caller owns result display, copying and writing to Inbox. A returned
  model identifies the requested model, which Relay may route to an alias.
- Selection capture is owned by the Windows Launcher trigger, not this
  adapter: the native layer may provide `selectedText` before focusing the
  palette. Translation never replaces source text automatically; the result
  surface must request the explicit native replace action and show failures.
- In a Tauri build, this module only uses `fetch` to the local Relay runtime;
  text clipboard reads/writes and selection restoration stay behind the native
  clipboard/selection bridges. Browser previews may use the Relay's CORS
  response and clipboard fallback, but that fallback is not used to store
  provider credentials or to bypass a native clipboard error in a packaged
  runtime.
