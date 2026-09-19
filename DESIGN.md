# DAIW / Studio Design System

## 0. Research Log

- Embedded refs: shortlisted Linear, ElevenLabs, and Sentry; picked Minimalist UI + Linear because the workstation needs dense engineered hierarchy, restrained elevation, and technical precision. Linear's violet is intentionally replaced by DAIW's warm amber story.
- Lazyweb: 3 queries, 3 screens viewed (Veed desktop editor, Gizmo instrument, Videoleap tablet editor) -> persistent desktop rail + contextual browser, transport adjacent to timeline, independently scrolling track field, and a tablet clip-focused workspace with bottom-priority controls.
- Interaction refs: inspected beui.dev `tabs` and `button` source -> selected-state continuity, interruptible micro feedback, mounted tab panels, and async button feedback; implemented with CSS transitions rather than adding a motion dependency.
- UI/UX database: attempted the required palette/type design-system lookup; skipped because Python is unavailable in the environment.
- Imagen drafts: skipped because no image-generation tool is available in this team runtime.

## 1. Atmosphere & Identity

DAIW feels like a mastering desk at 2 a.m.: graphite metal, low ambient light, exact calibration marks, and one warm amber signal path. The signature is the **amber playhead**—a thin illuminated line that connects transport, timeline, selected notes, pending AI edits, and active channel state without turning the interface into neon decoration. The memorable moment is selecting a linked clip and seeing its source relationship, modifiers, and effective result resolve in one continuous workspace.

Primary persona: a keyboard-first producer arranging and revising linked musical ideas. Stress personas: a touch user on a narrow tablet; a low-vision user at 200% zoom; a motion-sensitive user; and a user reviewing risky AI changes who needs unambiguous scope, dependencies, and recovery.

## 2. Color

Dark-only workstation chrome for Stage 1. All implementation color values must be CSS variables from this table.

| Role | Token | Value | Usage |
|---|---|---|---|
| Canvas | `--ink-0` | `#0b0c0e` | Deep application background |
| Rail | `--ink-1` | `#111216` | Global chrome |
| Panel | `--ink-2` | `#17191e` | Primary panels |
| Raised | `--ink-3` | `#1d2026` | Controls and selected rows |
| Hover | `--ink-4` | `#252932` | Hover and active-neutral surfaces |
| Text primary | `--text-1` | `#f2f0ea` | Titles and primary values |
| Text secondary | `--text-2` | `#b5b1a8` | Labels and body copy |
| Text tertiary | `--text-3` | `#77756f` | Metadata and disabled copy |
| Hairline | `--line-1` | `#2c3038` | Structural dividers |
| Subtle hairline | `--line-2` | `#22252b` | Grid and recessed divisions |
| Amber primary | `--amber-1` | `#f0a83a` | Primary action and playhead |
| Amber bright | `--amber-2` | `#ffc768` | Hover, focus, selected text |
| Amber dim | `--amber-3` | `#6e4a1d` | Selected surface and preview diff |
| Success | `--green` | `#67c59a` | Connected, accepted, healthy |
| Warning | `--yellow` | `#e2bd62` | Partial, stale-risk, caution |
| Error | `--red` | `#ef7e7e` | Blocking and destructive |
| Info | `--blue` | `#74a8dc` | Source links and informational state |
| Track coral | `--clip-coral` | `#d88476` | Drum/percussive clips |
| Track mint | `--clip-mint` | `#7cbaa3` | Bass clips |
| Track lilac | `--clip-lilac` | `#a99ac8` | Chord/pad clips |
| Track sky | `--clip-sky` | `#78a9c2` | Melody/audio clips |
| Scrim | `--scrim` | `#07080acc` | Dialog backdrop |

Rules: amber is interactive or temporal, never filler. Track pastels always include labels/glyphs so color is not the only carrier. Text/background pairs target WCAG 2.2 AA.

## 3. Typography

Font stacks avoid generic Inter. No network font is required.

- Primary: `"Avenir Next", "Segoe UI Variable", "Helvetica Neue", sans-serif`
- Technical: `"IBM Plex Mono", "SFMono-Regular", Consolas, monospace`
- Numeric values use tabular figures.

| Token | Size / line | Weight | Tracking | Usage |
|---|---|---|---|---|
| `--type-title` | `1.25rem / 1.2` | 600 | `-0.02em` | Project and modal titles |
| `--type-h2` | `1rem / 1.3` | 600 | `-0.01em` | Panel title |
| `--type-body` | `0.875rem / 1.45` | 450 | normal | Body and controls |
| `--type-small` | `0.75rem / 1.35` | 500 | normal | Dense rows and metadata |
| `--type-micro` | `0.6875rem / 1.2` | 600 | `0.06em` | Uppercase overlines |
| `--type-numeric` | `0.75rem / 1.2` | 500 | normal | Timeline and meter values |

At narrow widths form controls remain at least `1rem` to prevent browser zoom; compact labels may remain small when they are not editable body content.

## 4. Spacing & Layout

Base unit: **4px**. Tokens: `--s1: 4px`, `--s2: 8px`, `--s3: 12px`, `--s4: 16px`, `--s5: 20px`, `--s6: 24px`, `--s8: 32px`.

Geometry: `--rail: 56px`, `--browser: 240px`, `--inspector: 276px`, `--topbar: 52px`, `--transport: 44px`, `--editor: 220px`, `--track-row: 72px`. The 56px rail accommodates 44px controls with side spacing. Radius scale: `--r1: 4px`, `--r2: 6px`, `--r3: 10px`. Icons: `--icon-sm: 14px`, `--icon-md: 17px`, `--icon-lg: 20px`.

Desktop uses a fixed-sidenav, three-pane shell: rail + browser + center + inspector. The app is bounded by `100dvb`; the center canvas, browser, inspector, and mixer own named internal scrolling regions. At <=1100px the inspector becomes a tab/sheet. At <=760px the browser and inspector are routed through the bottom workspace tabs, the timeline prioritizes the selected track, and all actionable targets become at least 44px. At 375px primary content never horizontally overflows; only explicitly labeled timeline/piano-roll reels may scroll inline.

## 5. Components

### IconButton / ActionButton
- **Structure**: semantic button, Lucide icon, optional label and shortcut.
- **Variants**: ghost, subtle, primary, danger; icon-only requires `aria-label`.
- **Spacing**: `--s2` and `--s3`; desktop compact visual may use expanded invisible hit area.
- **States**: default, hover, active, focus-visible, disabled, pending, success, error.
- **Motion**: opacity/color plus `scale(.97)` press using micro timing; reduced motion removes scale.

### Panel
- **Structure**: header cluster, optional tabs/actions, scroll body, optional footer.
- **Variants**: level 1 rail, level 2 panel, level 3 floating.
- **States**: loading skeleton, empty explanation/action, inline error/retry.
- **Layout**: scroll-body-shell with `min-block-size: 0`; header/footer stay fixed.

### SegmentedTabs
- **Structure**: `role=tablist`, buttons with selected state, labelled panel.
- **States**: selected uses amber underline/tonal shift; focus uses focus ring; overflow becomes an accessible reel.
- **Motion**: beui-inspired state continuity via transform/opacity only; inactive panels may unmount when they represent alternate heavy workspaces.

### Field / KnobLikeRange
- **Structure**: visible label, native input/select/range, numeric readout and unit.
- **States**: default, hover, focus, disabled, invalid with nearby recovery copy.
- **Accessibility**: native semantics, explicit min/max/step, no drag-only requirement.

### TrackRow / Clip
- **Structure**: fixed track header + time-grid lane; clip is a labelled semantic button with waveform/note glyph.
- **States**: selected, muted, soloed, pending diff, conflict, disabled.
- **Accessibility**: label includes track/clip name, bar start, duration, and linked-source status; keyboard move actions have visible alternatives.

### StatusNotice
- **Structure**: icon, concise cause, recovery action, dismiss when non-blocking.
- **Variants**: info, success, warning, error.
- **Accessibility**: `role=status` for progress/success; `role=alert` for blocking errors.

### Dialog / Drawer
- **Structure**: scrim, labelled surface, header, scroll body, action footer.
- **States**: opening, open, pending, error, closing.
- **Accessibility**: escape and explicit close, focus placement and return, no background interaction.

### Meter
- **Structure**: labelled channel value, CSS bar, peak marker, numeric dB.
- **States**: idle, signal, peak, muted. Color is reinforced by value and labels.

## 6. Motion & Interaction

| Token | Value | Usage |
|---|---|---|
| `--motion-micro` | `120ms` | Press, hover, mute/solo |
| `--motion-standard` | `180ms` | Tabs, selection, notice |
| `--motion-panel` | `260ms` | Drawer/dialog enter |
| `--ease-out` | `cubic-bezier(.16,1,.3,1)` | Entrances and emphasis |
| `--ease-ui` | `cubic-bezier(.2,.8,.2,1)` | State transitions |

Keyboard shortcuts ignore editable targets and IME composition. Space controls transport only outside inputs; Delete removes the selected instance only with an explicit selection. All async controls retain their label context and expose pending/success/error. Animate only transform, opacity, color, filter. `prefers-reduced-motion` disables spatial transforms and preserves instant state clarity.

## 7. Depth & Surface

Strategy: **mixed tonal shift + precision borders**. The canvas, panels, raised controls, and floating overlays step through `--ink-0` to `--ink-4`. Default surfaces use `1px solid var(--line-1)`; recessed timeline/editor fields use inset shading; shadows are reserved for dialogs/dropdowns: `--shadow-float: 0 16px 48px #00000066, inset 0 1px #ffffff0a`. A subtle non-interactive noise texture may be built from CSS opacity, never at the cost of text contrast.

## 8. Accessibility Constraints & Accepted Debt

Target WCAG 2.2 AA: 4.5:1 normal text, 3:1 large text and essential UI graphics; visible focus on every control; complete keyboard reachability; 44px touch targets in tablet/narrow layouts; reduced motion; 200% zoom without loss of actions; errors state cause and recovery. Dense DAW regions have semantic summaries and native controls alongside direct manipulation.

| Item | Location | Why accepted | Owner / Exit |
|---|---|---|---|
| Piano-roll pointer editing is not a full screen-reader note-authoring experience | Piano roll | Stage 1 scope; native note list editing is not in contract | Add structured note table in Stage 2 |
| No high-contrast theme toggle | Whole app | Dark workstation direction is fixed for Stage 1 | Add forced-colors and high-contrast theme after real user audit |
