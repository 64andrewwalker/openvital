1. Update README.md:
   - CLI commands table shows `goal set/status/remove`, `config show/set`, etc.
   - I need to check if there are other commands in `src/cli.rs` that aren't in README.md like `anomaly`, `context`, `med`.
   - Update `goal set` example in README.md. It says `openvital goal set water --target 2000 --direction above --timeframe daily` but `cli.rs` says `GoalAction::Set` has `target_pos`, `direction_pos`, `timeframe_pos` and `target`, `direction`, `timeframe`. Oh wait, `openvital goal set water 2000 above daily` is also supported.
   - Wait, `README.md` `Commands` section is missing `anomaly`, `context`, `med`.
2. Update CLAUDE.md:
   - CLI commands table is missing `med`.
   - The "Architecture" section needs to match the source tree: e.g., missing `med` in CLI, core, db, cmd, models.
   - Missing `units.rs` in `core/`?
3. Docs / Config File Comments:
   - Check `config.toml` structure in `docs/openvital-spec.md` and see if it aligns with `src/models/config.rs`. `models/config.rs` does NOT have `[goals]` or `[agent]` sections. It only has `[profile]`, `[units]`, `[aliases]`, `[alerts]`. Wait, in `docs/openvital-spec.md` under `## Appendix B: Config File Example`, it shows `[goals]` and `[agent]` but the actual implementation of `Config` in `src/models/config.rs` has `profile`, `units`, `aliases`, `alerts`. The goals are actually stored in the DB, not config.
4. I will replace the incorrect parts of `README.md`, `CLAUDE.md`, and `docs/openvital-spec.md`.
