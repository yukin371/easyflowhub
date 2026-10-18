# Windows Theme Smoke (2026-10-18)

> status: pending
> owner: EasyFlowHub maintainers
> scope: v0.2.0 current theme and opaque fallback after token migration

## Environment

- Build source: local release or CI artifact
- Validation date:
- Windows version:
- Validator:

## Checklist

- [ ] Manager is readable in light and dark themes; primary text, controls, focus and status colors remain distinguishable.
- [ ] Quick Note and Todo child windows use the selected theme and remain readable after closing and reopening.
- [ ] Launcher uses the selected theme and remains readable after keyboard or middle-click activation.
- [ ] With blur enabled, Manager and Launcher glass surfaces do not obscure text, inputs, selection or error feedback.
- [ ] Turning blur off changes Manager and Launcher to opaque surfaces and visibly removes backdrop blur; turning it back on restores the configured appearance.
- [ ] Windows High Contrast / forced-colors mode keeps text, borders, focus and primary actions distinguishable across Manager, Launcher and Quick Note.
- [ ] Window resizing and supported display scaling do not cause clipped text, overlapping controls or inaccessible actions.

## Record

Record the build source, Windows version, validator, passed items, failed items and any accepted non-blocking exception. Attach screenshots for visual failures or exceptions.

## Exit Rule

The v0.2.0 P0/P1 release gate requires every applicable item to pass or have an explicit accepted exception. Voice remains separately governed by the P2 section of [windows-package-smoke.md](./windows-package-smoke.md).
