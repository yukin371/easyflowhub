# Shared Data Types

> Last updated: 2026-10-18

## Responsibility

This module owns shared frontend data contracts that cross feature boundaries, including the normalized Capture envelope and its attachment variants.

## Data flow

Launcher or Manager context -> `Capture` / `CaptureAttachment` -> Inbox persistence adapter -> `InboxItem` and native attachment records.

## Conventions & pitfalls

- Capture attachments include text, transcript, links and local assets; persisted `InboxAttachment` remains the native binary-record DTO.
- Keep `source`, `createdAt` and an explicit nullable `inboxItemId` on each attachment so pending and persisted captures have the same shape.
- Do not interpret the normalized capture types as proof that a payload is already persisted or that a future transcript has been produced.
