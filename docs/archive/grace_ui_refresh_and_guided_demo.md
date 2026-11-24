# Grace interface refresh and guided demo

## Goal and context

Make Grace's controls easier to recognize while keeping the classic menus, toolbar labels and
positions, shortcuts, and plotting workflow familiar. Give newcomers a short hands-on example.

## Design philosophy

Use subtle glass on menus and dialogs, a consistent teal accent, and a solid white plot canvas.

## Scope and contracts

- Style the existing **Edit > Data sets... > Create new** disclosure as a button with a chevron
  and visible focus and expanded states. Keep its native behavior and choices.
- Extend the light palette with shared surface, border, and shadow values. Give translucent
  surfaces an opaque fallback and respect reduced motion.
- Open **Help > Guided demo...** and the README demo link in a fresh practice tab. Keep guide
  progress and preferences session-local. **Back** and **Continue** move the guide manually.
  **Pause** collapses the guide, **Resume** restores its current instruction, and **Finish**
  dismisses it. Each action preserves the editable practice project.
- Teach formula creation with `X = $t`, `Y = sin(5*$t)+0.5*sin(13*$t)`, 128 points, and range
  `0` to `2*pi*127/128`. Then calculate the FFT magnitude at each index and save the AGR through
  the classic menus. Repeat the relevant recipe inside the formula and Fourier dialogs.

## File scope

| Area | Changes |
| --- | --- |
| Browser UI | Stylesheet, Help menu, guide module, and integration with existing dialogs and preferences. |
| Playwright | Guided-demo regression test and reproducible animated WebP capture. |
| Documentation | README screenshot block, capture instructions, and changelog. |

## Milestones

| M | Deliverable | Acceptance |
| --- | --- | --- |
| 1 | Restyle controls and surfaces | Compare desktop, toolbar-wrap, and narrow renders. Check contrast, focus, and classic controls. |
| 2 | Ship the guided demo | Exercise pause, edit, and resume. Verify spectrum bins 5 and 13, AGR download, and project and preference isolation in Chromium, Firefox, and WebKit. |
| 3 | Refresh the README | Recapture the cooling plot and encode the waveform-to-spectrum demo as a one-play quality-90 WebP under 1 MB and five seconds. Check playback and readability. |

*Plan created with `vosslab-skills:modest-plan-drafter`.*
