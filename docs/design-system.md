# Design system

Tokens live in apps/desktop/src/design/tokens.css. Base, panel and raised surfaces
use cool near-black tones, restrained hairlines and at most 2px corners. Amber is
the single accent, controlled by --accent. Fault red is reserved for errors.

UI uses locally installed Inter/Segoe UI; readouts use JetBrains Mono/Cascadia Code
with system fallbacks. No external fonts are fetched. Readouts use tabular numbers.

Panel, Button, Field, Readout, StatusLine and Tile live in design/primitives.tsx.
Keyboard focus uses the accent outline. Tiles use corner ticks and a faint hatch.
No pills, gradients, glow, bounce, emoji or decorative simulated telemetry.
Feature CSS must use tokens for every color. Add meaningful states when the
underlying feature is functional; never present fake camera feeds or live metrics.
