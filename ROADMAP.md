# ratma-tg-types roadmap

Companion to [tracking issue #1](https://github.com/RATMA-Studio/ratma-tg-types/issues/1). Lists every concrete step from the current state (a renamed copy of [`fmeef/botapi-rs`](https://github.com/fmeef/botapi-rs)) to a production-grade, standalone Rust types crate for Telegram Bot API + WebApp that other Telegram crates in the ecosystem can adopt.

## Vision

One canonical Rust crate of Telegram Bot API types, auto-synced to the latest spec, idiomatic enough that `teloxide`, `frankenstein`, `rust-tg-bot` and friends can adopt it as a foundation instead of each maintaining their own hand-written copies.

## Source of truth

- Spec source: git submodule [`PaulSonOfLars/telegram-bot-api-spec`](https://github.com/PaulSonOfLars/telegram-bot-api-spec) → `telegram-bot-api-spec/api.json`.
- Currently pinned at commit `9dca8b0` (Bot API 10.0, May 8 2026).
- Submodule is bumped by infra automation (see Phase 4).

---

## Phase 0 — Fork hygiene *(in progress)*

Goal: turn the cloned upstream into a clean RATMA-branded baseline that builds green.

- [x] Rename crate `botapi` → `ratma-tg-types`, generator `tggen` → `ratma-tg-types-codegen`, repoint URLs (#1, commit `3e8a47e`).
- [ ] **chore: bump build/runtime deps to current stable**
  - Audit `reqwest`, `tokio`, `serde`, `hyper`, `hyper-util`, `ordered-float`, `rand`, `serde_stacker`, `rmp-serde`. Re-pin to latest minor compatible with our MSRV.
  - Run `cargo update`, `cargo audit`, `cargo deny check advisories`.
  - Risk: `reqwest` major-version bumps may break `multipart::Part` usage in `bot.rs`. Hold any major bump until Phase 2 (we may drop reqwest entirely).
- [ ] **chore: edition 2024**
  - Upstream is on `edition = "2021"`. Move all three sub-crates (root, `generate/`) to `edition = "2024"` to align with `fork-teloxide` and current toolchain.
- [ ] **chore: MSRV declaration**
  - Add `rust-version = "..."` to all `Cargo.toml`s. Document the floor.
- [ ] **chore: rustfmt + clippy gates**
  - Add `rustfmt.toml` matching `fork-teloxide` conventions.
  - Add `clippy.toml` with `msrv` and our preferred lints.
  - Optional `deny(missing_docs)` is **out of scope** until we control the codegen output (Phase 2).
- [ ] **chore: license attribution**
  - Keep upstream `LICENSE` (MIT) untouched.
  - Add `NOTICE.md` crediting `fmeef/botapi-rs` (Alex Ballmer) and `PaulSonOfLars/telegram-bot-api-spec`.

Deliverable: green `cargo check && cargo test && cargo clippy -- -D warnings` against latest stable Rust, no behavioral change.

---

## Phase 1 — Workspace split

Goal: separate **pure types** from **HTTP client** so types are usable without pulling reqwest/tokio/hyper.

Three-crate Cargo workspace:

```
ratma-tg-types/
├── Cargo.toml             # workspace root only
├── crates/
│   ├── codegen/           # tggen renamed; pure Rust code generator
│   ├── core/              # ratma-tg-types-core — pure types, NO http
│   └── http/              # ratma-tg-types-http — async client, depends on core
```

- [ ] **refactor: convert to Cargo workspace**
  - Root `Cargo.toml` becomes `[workspace]` only. Move current sources under `crates/`.
- [ ] **feat: `ratma-tg-types-core` crate (pure types)**
  - Codegen splits output: `gen_types.rs` → `crates/core/src/gen.rs`; methods stay in `http` for now.
  - Strip dependencies that types pull from the client: replace `use crate::bot::Part` and `reqwest::multipart::Form` usage in generated types with a feature-gated abstraction (`MultipartPart` trait in `core`, impl in `http`).
  - Output dep tree of `core`: `serde`, `serde_json`, `ordered-float`. **No** reqwest, tokio, hyper.
- [ ] **feat: `ratma-tg-types-http` crate (client)**
  - Hosts `Bot`, `BotBuilder`, `ext::LongPoller`, `ext::Webhook`, multipart upload, ratelimit handling, `gen_methods.rs`.
  - Depends on `ratma-tg-types-core` + the heavyweight async stack.
- [ ] **feat: re-export façade**
  - Top-level `ratma-tg-types` crate becomes a thin façade: `pub use ratma_tg_types_core::*;` + `#[cfg(feature = "http")] pub use ratma_tg_types_http::*;`.
  - Users wanting only types: `cargo add ratma-tg-types --no-default-features`.
- [ ] **test: workspace builds in all feature combinations**
  - `core` alone, `core + http`, `core + http + rhai`.

Deliverable: a `ratma-tg-types-core` that compiles with `serde` only and exposes every Bot API type in pure form.

---

## Phase 2 — Idiomatic codegen overhaul

Goal: stop producing flat-struct sum types. Generate enums where Telegram intends them.

- [ ] **feat: subtype-aware codegen**
  - Generator already detects `subtype_of` from the spec. Use it: produce `enum ChatMember { Owner(ChatMemberOwner), Administrator(...), ... }` instead of optional-field-soup.
  - Apply to: `ChatMember`, `MessageOrigin`, `BackgroundType`, `BackgroundFill`, `ChatBoostSource`, `ReactionType`, `MaybeInaccessibleMessage`, `MenuButton`, `InlineQueryResult`, `InputMessageContent`, `PassportElementError`, `BotCommandScope`, `BackgroundFill`, and any other `subtype_of` parent in the schema.
  - Use `#[serde(tag = "...", rename_all = "snake_case")]` driven by the discriminator field (already extracted via regex from description in upstream — extend to all known discriminators).
- [ ] **feat: enum extraction for fields with constrained string values**
  - Parse description text for `"foo"`, `"bar"` enumerations (e.g. `MessageEntity.type` → `MessageEntityKind { Mention, Hashtag, Bold, ... }`).
  - Replace raw `String` fields with the generated enum.
  - **Critical** for `MessageEntity.type`, `Chat.type`, `Update.*` discriminators, `ChatMemberStatus`, `Sticker.type`, `PollType`, `MaskPosition.point`, `ChatAction`, `DiceEmoji`.
- [ ] **feat: ID newtypes**
  - Generate `pub struct UserId(pub i64)`, `pub struct ChatId(pub i64)`, `pub struct MessageId(pub i32)`, `pub struct FileId(pub String)`, `pub struct BusinessConnectionId(pub String)`, `pub struct CallbackQueryId(pub String)`, etc.
  - Drive from a configurable mapping table (e.g. `crates/codegen/id_types.toml`): which JSON field names map to which newtype.
  - Mirror `teloxide-core` conventions for adoption-friendliness.
- [ ] **feat: `#[non_exhaustive]` on every generated enum**
  - Prevents downstream from breaking when Telegram adds variants.
  - For untagged enums, add `#[serde(other)] Unknown` catch-all so deserialization survives unknown values.
- [ ] **feat: drop `NoSkip*` doubled structs**
  - Only kept for `rmp_serde` (array-format) edge case. Gate behind `feature = "noskip"` instead of unconditional generation. Removes ~30% of `gen_types.rs` line count.
- [ ] **feat: drop `BoxWrapper<Unbox<T>>` abstraction**
  - Replace with plain `Box<T>` where the MFAS cycle-breaker decides indirection is required. Upstream's `BoxWrapper` exists to make API stable across regenerations — we get the same effect by pinning the MFAS seed (deterministic order).
- [ ] **feat: `#[serde(deny_unknown_fields)]` opt-in**
  - Strict-mode feature for users who want to detect Telegram API schema drift in tests.

Deliverable: `MessageEntity` from `core` looks like

```rust
pub struct MessageEntity {
    pub kind: MessageEntityKind,
    pub offset: u32,
    pub length: u32,
}

#[non_exhaustive]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MessageEntityKind {
    Mention, Hashtag, Cashtag, BotCommand, Url, Email, PhoneNumber,
    Bold, Italic, Underline, Strikethrough, Spoiler,
    Blockquote, ExpandableBlockquote,
    Code,
    Pre { language: Option<String> },
    TextLink { url: Url },
    TextMention { user: User },
    CustomEmoji { custom_emoji_id: CustomEmojiId },
    DateTime { unix_time: i64, date_time_format: Option<String> },
    #[serde(other)] Unknown,
}
```

---

## Phase 3 — LLM-in-the-loop refinement

Goal: handle the long tail of "the codegen produced something dumb, a human would write it differently."

- [ ] **infra: codegen-postprocess Claude pipeline**
  - After `tggen` writes `gen.rs`, a postprocess step prompts Claude with:
    - the raw generated chunk;
    - the corresponding spec fragment;
    - style anchors (existing hand-tuned types we want to mirror);
    - the project's rustfmt + clippy lints.
  - Claude returns a patch. Apply, run `cargo check`, run **golden-snapshot diff** of the public API surface (`cargo public-api`).
  - If the patch changes public API in unexpected ways → fail CI, surface diff for human review.
- [ ] **feat: deterministic, pinned model + temperature 0**
  - Pin Claude model version to avoid output drift across releases.
  - Store all postprocess prompts in `crates/codegen/prompts/` so changes are reviewable.
- [ ] **feat: human-curated overrides file**
  - `crates/codegen/overrides.toml` for cases that even Claude gets wrong; codegen consults overrides last and they win.

Deliverable: when Telegram releases Bot API 10.1, regeneration produces idiomatic Rust on the first try ≥95% of the time; the remaining ≤5% are caught by golden tests for human review.

---

## Phase 4 — Auto-update infrastructure

Goal: stay in lockstep with Telegram. No human in the loop for routine updates.

- [ ] **infra: nightly CI job**
  - `git submodule update --remote telegram-bot-api-spec` → diff `api.json` → if changed, regenerate, run full test suite, open auto-PR.
  - PR title format: `#<N> feat sync Bot API <version> (<release date>)`.
  - Reviewer: human (you) for first ~3 months, then auto-merge if all gates green.
- [ ] **infra: golden API surface diff in PR description**
  - Each auto-PR body includes a structured "what changed in the public API" section using `cargo public-api`. New types, new fields, removed items, renames. Makes review a 30-second skim.
- [ ] **infra: integration smoke test**
  - Optional: real bot token in CI secrets, deserialize a live response from `getMe`, `getUpdates`, sample of every method's response type. Catches subtle JSON-shape drifts the schema misses.
- [ ] **infra: cargo-deny + cargo-audit on schedule**
  - Independent of Telegram changes, weekly run to catch vulnerabilities in deps.
- [ ] **infra: docs.rs build + dependent-crate smoke check**
  - On every release, confirm docs.rs builds and known reverse-deps still compile (start with `teloxide-types-shim` from Phase 5).

Deliverable: nightly auto-PR mechanism; from Telegram release to crates.io publish ≤ 24h.

---

## Phase 5 — Adoption & ecosystem

Goal: make it trivial for existing Rust Telegram crates to use `ratma-tg-types` as their foundation.

- [ ] **feat: `teloxide-types-shim` adapter crate in `RATMA-Studio/fork-teloxide`**
  - Lives at `crates/teloxide-types-shim/`, depends on `ratma-tg-types-core`.
  - Re-exports under teloxide-native names: `pub use ratma_tg_types_core::MessageEntity` etc.
  - Where teloxide's idioms diverge (e.g. teloxide has `MessageEntityRef<'a>` with UTF-16↔UTF-8 conversion), the shim provides the converter functions on top of `ratma-tg-types-core` types — keeps teloxide's API stable.
  - Switch `fork-teloxide` to consume the shim instead of `crates/teloxide-core/src/types/`. Big-bang or progressive (per-type) — TBD in Phase 5 planning.
- [ ] **docs: migration guide**
  - `MIGRATION.md` for users of teloxide / frankenstein / rust-tg-bot: how to swap to ratma-tg-types as the type backend.
- [ ] **outreach: PR to upstream `fmeef/botapi-rs`**
  - Optional: propose merging the `core/http` split upstream if the maintainer is interested. Maintains good ecosystem citizenship.
- [ ] **outreach: heads-up issues in `teloxide/teloxide`, `ayrat555/frankenstein`**
  - Once the crate is stable, file polite "FYI a centralized types crate now exists, here's a migration path" issues. Adoption is their choice; we don't push.

Deliverable: at least the RATMA fork of teloxide is fully backed by `ratma-tg-types-core`. Other crates have a documented migration path.

---

## Phase 6 — Release engineering

Goal: stable 0.1.0 on crates.io.

- [ ] **feat: SemVer policy doc**
  - Document what is/isn't a breaking change. Telegram adding a variant to an enum → patch (covered by `#[non_exhaustive]` + catch-all). Telegram removing a field → minor (Optional → not present). Telegram renaming a type → major.
- [ ] **chore: 0.1.0 release**
  - Yank-proof tagging: tag commits, attach release notes referencing the Bot API version covered.
- [ ] **chore: trusted publishing**
  - Use crates.io OIDC-based publishing from GitHub Actions; no long-lived API tokens.

Deliverable: `cargo add ratma-tg-types` works, `ratma-tg-types-core` works, both with current Bot API.

---

## Out of scope

- **MTProto / TDLib types.** Different schema universe (TL, not JSON), different abstractions, thousands of types. Use `grammers` for that.
- **A full bot framework.** This crate is types-first. The `http` client crate is a convenience for current users, not the focus.
- **Replacing teloxide's dispatcher, handlers, DPTree integration.** Those stay in teloxide.
- **Backporting to old Bot API versions.** We always track HEAD.

## Open questions

- Do we want a `serde-json-only` minimal variant of `core` (no `serde_json` even, just `serde`)? Probably not — `serde_json` is universally available.
- Should `core` expose a parser for raw entity arrays back into AST (relevant for inbound-message processing)? Probably yes but lives downstream of `core`, not in it.
- Codegen language: keep Rust (`tggen`) or consider rewriting in a smaller scripting language for faster iteration? Rust wins — same toolchain as the output.

## Status snapshot

| Phase | Status | Issue |
|---|---|---|
| 0 — Fork hygiene | in progress | #1 tracking |
| 1 — Workspace split | not started | tbd |
| 2 — Idiomatic codegen | not started | tbd |
| 3 — LLM-in-the-loop | not started | tbd |
| 4 — Auto-update infra | not started | tbd |
| 5 — Adoption | not started | tbd |
| 6 — Release | not started | tbd |

Last updated: 2026-05-17.
