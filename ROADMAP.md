# ratma-tg-types roadmap

Companion to [tracking issue #1](https://github.com/RATMA-Studio/ratma-tg-types/issues/1). Concrete steps from the current state (a renamed fork of [`fmeef/botapi-rs`](https://github.com/fmeef/botapi-rs)) to a production-grade, standalone Rust types crate family covering the **entire Telegram surface** — Bot API and WebApp / Mini Apps — and consumed by every RATMA Rust repo.

## Vision

**Telegram itself is the only source of truth.** Not `teloxide`, not `frankenstein`, not `botapi-rs`. Each of those maintains its own hand-written copy of Telegram's types, drifts from spec on its own schedule, and exposes its own slightly-incompatible API surface. The cost is duplicated effort across the ecosystem and quiet bugs when a project's copy lags Telegram's docs.

`ratma-tg-types` collapses that into one Rust crate family:

- **One codegen pipeline.** Reads Telegram's published surface (Bot API JSON via `PaulSonOfLars/telegram-bot-api-spec`; WebApp HTML via a scraper that lives here). Emits idiomatic Rust.
- **One set of types, two domains.** `ratma-tg-types-core` for Bot API, `ratma-tg-types-webapp` for WebApp / Mini Apps. Shared ID newtypes (`UserId`, `ChatId`, …) sit in a small common crate.
- **Auto-sync.** When Telegram ships a new version, a nightly CI job bumps the spec, regenerates, runs the test suite, and opens an auto-PR. Drift goes from "discovered when someone files a bug" to "visible in a green/red review queue within 24 h".
- **Every RATMA Rust project consumes one source.** No hand-written copies anywhere in the org.

We are not waiting for the wider ecosystem to converge on a common types crate — every project has done its own thing for years and won't stop. We build the unified crate for ourselves and ship it; if `teloxide` or others want to adopt, the migration path will exist (Phase 5).

## Source of truth — two surfaces

| Surface | Doc URL | Spec source | Status |
|---|---|---|---|
| **Bot API** | `core.telegram.org/bots/api` | git submodule [`PaulSonOfLars/telegram-bot-api-spec`](https://github.com/PaulSonOfLars/telegram-bot-api-spec) → `telegram-bot-api-spec/api.json` | submodule pinned at commit `9dca8b0` (Bot API 10.0, May 8 2026) |
| **WebApp / Mini Apps** | `core.telegram.org/bots/webapps` | **TBD** — no community spec exists. We write a scraper that emits `webapp.json` in the same shape as `api.json`. | not started |

Submodules are bumped by infra automation (see Phase 4).

## Consumer matrix

Concrete downstream Rust crates that will/do depend on `ratma-tg-types`:

| Consumer | Surface needed | Current state | Target |
|---|---|---|---|
| [`RATMA-Studio/fork-teloxide`](https://github.com/RATMA-Studio/fork-teloxide) | Bot API types | Hand-written `crates/teloxide-core/src/types/*.rs` (~6000 LOC) | `crates/teloxide-types-shim/` re-exports from `ratma-tg-types-core` (Phase 5) |
| [`RATMA-Studio/ratma-auth-rs`](https://github.com/RATMA-Studio/ratma-auth-rs) | WebApp `init_data`, `User` | Depends on third-party `init_data_rs` (its own hand-written copy of WebApp types + HMAC validator) | Drop `init_data_rs`, depend on `ratma-tg-types-webapp` (which gains the `init_data::{parse,validate}` helpers) |
| `RATMA-Studio/ratma-studio-tg-bot` and other bot backends | Bot API types | Already on `fork-teloxide` | Inherits via fork-teloxide shim |
| `RATMA-Studio/ratma-studio-tma`, `rori-shop-tma`, `Roa-Mini-App` TMA backends | WebApp types | Hand-written or via `init_data_rs` | Direct dep on `ratma-tg-types-webapp` |

The fork-teloxide shim is the single largest piece of work in Phase 5; the WebApp consumers are smaller and switch over once `ratma-tg-types-webapp` exists at parity with `init_data_rs`.

---

## Phase 0 — Fork hygiene *(done)*

Goal: clean RATMA-branded baseline that builds green.

- [x] Rename crate `botapi` → `ratma-tg-types`, generator `tggen` → `ratma-tg-types-codegen`, repoint URLs (#1, commit `3e8a47e`).
- [x] Workspace split structure (#3, three crates land: `codegen`, `core`, `http`).
- [x] **chore: bump build/runtime deps to current stable** (#8). `cargo update` is a no-op on current pins; major-version bumps (`reqwest`, etc.) intentionally deferred until Phase 2 codegen overhaul, since `core` will likely drop `reqwest` entirely.
- [x] **chore: edition 2024 / `rust-version = "1.95"`** (in workspace).
- [x] **chore: rustfmt + clippy gates in CI** (#8). Full matrix: `fmt` on nightly (rustfmt.toml uses unstable features), `clippy --workspace --all-targets` on stable with `-Dwarnings`, `test` matrix on stable + MSRV 1.95.0, `doc` with `-Dwarnings`, `audit`, aggregate `ci-pass` gate via `re-actors/alls-green`.
- [x] **chore: license attribution** — upstream `LICENSE` (MIT) preserved; `NOTICE.md` credits `fmeef/botapi-rs` (Alex Ballmer) and `PaulSonOfLars/telegram-bot-api-spec`.

Deliverable: green `cargo check && cargo test && RUSTFLAGS="-Dwarnings" cargo clippy --all-targets` against latest stable, no behavioral change. ✓

---

## Phase 1 — Workspace cleanup *(done)*

Goal: real separation of **pure types** from **HTTP client**. `core` usable with nothing but `serde`.

Current crate layout:

```
ratma-tg-types/
├── Cargo.toml             # workspace root + façade crate
├── crates/
│   ├── codegen/           # ratma-tg-types-codegen — build-time generator
│   ├── core/              # ratma-tg-types-core — pure types (Bot API)
│   └── http/              # ratma-tg-types-http — async client
├── telegram-bot-api-spec/ # submodule, source of truth for Bot API
└── (Phase 2) crates/webapp/   # ratma-tg-types-webapp — WebApp types
```

- [x] Convert to Cargo workspace, three crates wired (#3).
- [x] **chore: strip `core/Cargo.toml` to types-only deps** (#11, #13). Dropped `enum_dispatch`, `log`, `serde_stacker` (unreferenced); moved `rmp-serde` to `[dev-dependencies]` (only used in generated `#[cfg(test)]` round-trip checks). Same cleanup in `http`: dropped `enum_dispatch`, `serde_stacker`, `rmp-serde`; moved `tokio-test` to `[dev-dependencies]`.
  - `anyhow` retained in `core` because generated `gen_types` subtype helpers return `Result<_, anyhow::Error>` and `bail!`. Replacing it requires Phase 2 codegen (own error enum).
- [ ] **test: every feature combination compiles** *(follow-up — wire a feature-matrix CI step)*
  - `cargo check -p ratma-tg-types-core --no-default-features`
  - `cargo check -p ratma-tg-types-core --features multipart`
  - `cargo check -p ratma-tg-types-core --features rhai`
  - `cargo check -p ratma-tg-types-http`
  - `cargo check -p ratma-tg-types --features http`

Deliverable: `cargo tree -p ratma-tg-types-core --no-default-features -e normal --depth 1` shows `anyhow`, `ordered-float`, `serde`, `serde_json`. ✓ (Phase 2 will retire `anyhow` from `core` as part of the codegen overhaul.)

---

## Phase 2 — Idiomatic codegen overhaul

Goal: stop producing flat-struct sum types. Generate enums where Telegram intends them. Apply to **both** Bot API and WebApp.

- [ ] **feat: subtype-aware codegen**
  - Generator already detects `subtype_of` from `api.json`. Use it: produce `enum ChatMember { Owner(ChatMemberOwner), Administrator(...), ... }` instead of optional-field-soup with a `tg_status: String` discriminator.
  - Apply to: `ChatMember`, `MessageOrigin`, `BackgroundType`, `BackgroundFill`, `ChatBoostSource`, `ReactionType`, `MaybeInaccessibleMessage`, `MenuButton`, `InlineQueryResult`, `InputMessageContent`, `PassportElementError`, `BotCommandScope`, `RevenueWithdrawalState`, `TransactionPartner`, `OwnedGift`, `StoryAreaType`, plus any `subtype_of` parent the schema reports.
  - Use `#[serde(tag = "...", rename_all = "snake_case")]` driven by the discriminator field already extracted by upstream codegen.
- [ ] **feat: enum extraction for fields with constrained string values**
  - Parse field descriptions for `"foo"`, `"bar"` enumerations (e.g. `MessageEntity.type` → `MessageEntityKind { Mention, Hashtag, Bold, ... }`).
  - Replace raw `String` fields with the generated enum.
  - **Critical fields**: `MessageEntity.type`, `Chat.type`, `Update.*` discriminators, `ChatMemberStatus`, `Sticker.type`, `PollType`, `MaskPosition.point`, `ChatAction`, `DiceEmoji`, `ParseMode`.
- [ ] **feat: ID newtypes**
  - Generate `pub struct UserId(pub i64)`, `pub struct ChatId(pub i64)`, `pub struct MessageId(pub i32)`, `pub struct FileId(pub String)`, `pub struct BusinessConnectionId(pub String)`, `pub struct CallbackQueryId(pub String)`, etc.
  - Driven by `crates/codegen/id_types.toml` mapping table — what JSON field name maps to what newtype.
  - Mirror `teloxide-core` conventions for adoption-friendliness (Phase 5 fork-teloxide shim becomes trivial).
- [ ] **feat: `#[non_exhaustive]` on every generated enum**
  - Prevents downstream from breaking when Telegram adds variants.
  - For untagged enums, add `#[serde(other)] Unknown` catch-all so deserialization survives unknown values from a server running ahead of our pinned spec.
- [ ] **feat: drop `NoSkip*` doubled structs**
  - Only useful for `rmp_serde` array-format edge case. Gate behind `feature = "noskip"` instead of unconditional generation. Removes ~30% of `gen.rs` line count.
- [ ] **feat: drop `BoxWrapper<Unbox<T>>` abstraction**
  - Replace with plain `Box<T>` where the MFAS cycle-breaker decides indirection is required. The original BoxWrapper exists to make API stable across regenerations — we get the same effect by pinning the MFAS seed (deterministic order across runs).
- [ ] **feat: `#[serde(deny_unknown_fields)]` opt-in**
  - Strict-mode feature for users who want to detect spec drift in tests.
- [ ] **feat: `Default` where every required field has a Default**
  - Same predicate logic we shipped in fork-teloxide PR #163 (extended payload codegen). Applies here too.

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

## Phase 2½ — WebApp ingestion *(new track)*

Goal: cover the WebApp / Mini Apps surface with the **same codegen** that handles Bot API.

- [ ] **infra: WebApp scraper**
  - No community spec exists for `core.telegram.org/bots/webapps`. We write a parser that lives at `crates/codegen/src/webapp_scrape.rs` and emits `telegram-bot-api-spec/webapp.json` (or a sibling location) in the same JSON shape `api.json` uses.
  - Targeted surface: `WebApp`, `WebAppInitData`, `WebAppUser`, `WebAppChat`, `WebAppMessage`, `WebAppData`, `MainButton`, `SecondaryButton`, `BackButton`, `SettingsButton`, `HapticFeedback`, `CloudStorage`, `BiometricManager`, `LocationManager`, `DeviceStorage`, `SecureStorage`, plus the `Telegram.WebApp` namespace's methods (`expand`, `close`, `sendData`, `requestFullscreen`, etc.) where Rust bindings make sense (i.e. data structures and event payloads, not the JS methods themselves unless wasm-bindgen wraps them — separate question).
  - Output cadence: run on the same nightly CI bump as `api.json`.
- [ ] **feat: `ratma-tg-types-webapp` crate**
  - Add `crates/webapp/` to the workspace.
  - `build.rs` consumes `webapp.json` via `ratma-tg-types-codegen` (same code generator, second input).
  - Same idiomatic-Rust rules as Phase 2: enums for sum types, ID newtypes, `#[non_exhaustive]`, etc.
- [ ] **feat: `init_data::{parse, validate}` helpers**
  - Bonus to make `ratma-auth-rs` migration tractable. HMAC-SHA256 validation per [Telegram docs](https://core.telegram.org/bots/webapps#validating-data-received-via-the-mini-app), `auth_date` TTL check.
  - Lives behind `feature = "init-data"` so callers that only want the types don't pay for `hmac`/`sha2`.
- [ ] **feat: shared ID newtypes between Bot API and WebApp**
  - `WebAppUser.id` and Bot API `User.id` are the same Telegram user id. Both fields use `UserId` from a shared dependency (`ratma-tg-types-ids` micro-crate or a re-export from `-core`).
  - WebApp `User` vs Bot API `User` are **not** unified — different field sets, different serialization contexts. Don't try.
- [ ] **test: round-trip a real `init_data` against `ratma-auth-rs`'s current expected output**
  - Pin a known-good `init_data` payload, parse with both `init_data_rs` (current dep) and `ratma-tg-types-webapp::init_data::parse`, assert structural equivalence.

Deliverable: `ratma-tg-types-webapp` at parity with `init_data_rs` plus the broader WebApp surface (buttons, storage, haptics) generated.

---

## Phase 3 — LLM-in-the-loop refinement *(deferred)*

Defer until Phase 2 base codegen consistently produces ≥90% idiomatic Rust on its own. The LLM is for the long tail of "codegen wrote something dumb, a human would write it differently."

When we get to it:

- [ ] **infra: codegen-postprocess Claude pipeline**
  - After `tggen` writes `gen.rs`, postprocess prompts Claude with the raw chunk, the spec fragment, style anchors, and rustfmt/clippy config. Claude returns a patch. Apply, `cargo check`, golden-snapshot-diff the public API surface (`cargo public-api`). If unexpected breakage → fail CI for human review.
- [ ] **feat: deterministic, pinned model + temperature 0**
  - Pin model version. Store prompts under `crates/codegen/prompts/` so they're reviewable.
- [ ] **feat: human-curated overrides file**
  - `crates/codegen/overrides.toml` consulted last; overrides win over both codegen and LLM.

---

## Phase 4 — Auto-update infrastructure

Goal: stay in lockstep with Telegram. No human in the loop for routine updates.

- [ ] **infra: nightly CI job**
  - `git submodule update --remote telegram-bot-api-spec` for Bot API.
  - Re-run WebApp scraper for `webapps.json`.
  - If anything changed: regenerate, `cargo test`, `cargo public-api diff`, open auto-PR.
  - PR title: `#<N> feat sync Telegram (Bot API <ver> / WebApp <date>)`.
  - Reviewer: human for the first ~3 months. Auto-merge once we trust the gates.
- [ ] **infra: golden API surface diff in PR description**
  - `cargo public-api` produces structured "what changed in the public API" summary. New types, new fields, removed items, renames. PR review becomes a 30-second skim.
- [ ] **infra: integration smoke test**
  - Optional: real bot token in CI secrets, deserialize a live response from a sample of every Bot API method. Catches JSON-shape drifts that the schema misses.
- [ ] **infra: `cargo-deny + cargo-audit` on schedule**
  - Weekly run, independent of Telegram changes.
- [ ] **infra: docs.rs build + dependent-crate smoke check**
  - On each release, confirm `docs.rs` builds and known consumers (`fork-teloxide-types-shim`, `ratma-auth-rs`) still compile against the new version.

Deliverable: nightly auto-PR mechanism. From Telegram release to crates.io publish ≤ 24 h.

---

## Phase 5 — Adoption & ecosystem

Goal: every RATMA Rust repo runs on `ratma-tg-types`. External adoption is a bonus, not a goal.

### Internal adoption (priority)

- [ ] **feat: `teloxide-types-shim` adapter crate in `RATMA-Studio/fork-teloxide`**
  - Lives at `fork-teloxide/crates/teloxide-types-shim/`, depends on `ratma-tg-types-core`.
  - Re-exports under teloxide-native names: `pub use ratma_tg_types_core::MessageEntity` etc.
  - Where teloxide's idioms diverge (`MessageEntityRef<'a>` with UTF-16↔UTF-8 conversion is the canonical example), shim provides converter functions on top of `ratma-tg-types-core` types — keeps teloxide's public API stable across the swap.
  - Switch `fork-teloxide` to consume the shim instead of `crates/teloxide-core/src/types/`. Progressive: per-type swap, gated by golden-snapshot diff.
  - **Erases**: most of fork-teloxide's manual types-side techdebt (Default-derives, `#[non_exhaustive]` adds, MediaKind discriminator dispatch, Bot API 10.0 drift fixes, `multipart` wiring) becomes properties of the generator, applied automatically on every regeneration.
- [ ] **feat: migrate `RATMA-Studio/ratma-auth-rs` off `init_data_rs`**
  - Replace `init_data_rs::{parse, validate}` calls with `ratma_tg_types_webapp::init_data::{parse, validate}`.
  - Drop the `init_data_rs` dep from `crates/ratma-auth-rs/Cargo.toml`.
  - The internal `ResolvedIdentity` mapping (in `src/identity/telegram.rs`) stays as-is — only the type source changes.
- [ ] **feat: migrate TMA backends**
  - `ratma-studio-tma`, `rori-shop-tma`, `Roa-Mini-App` backends switch their hand-written WebApp types (if any) to `ratma-tg-types-webapp`.

### External (optional)

- [ ] **docs: migration guide**
  - `MIGRATION.md` for users of `teloxide`, `frankenstein`, `tgbotapi`: how to swap their types backend for `ratma-tg-types`.
- [ ] **outreach: heads-up issues**
  - Once 0.1.0 ships and the fork-teloxide shim is live, file polite FYI issues in upstream `teloxide/teloxide` and `ayrat555/frankenstein`. We don't push adoption; we just make the option discoverable.
- [ ] **outreach: PR to upstream `fmeef/botapi-rs`** *(deferred / discretionary)*
  - Possibly propose the `core/http` split back upstream. Low priority; upstream maintainer activity unclear.

Deliverable: every Rust repo in `RATMA-Studio` org runs on `ratma-tg-types`. Outside adoption documented but unenforced.

---

## Phase 6 — Release engineering

Goal: stable 0.1.0 on crates.io.

- [ ] **feat: SemVer policy doc**
  - Document what is/isn't a breaking change. Telegram adding an enum variant → patch (covered by `#[non_exhaustive]` + `#[serde(other)]`). Telegram removing a field → minor (Optional → not present). Telegram renaming a type → major.
- [ ] **chore: 0.1.0 release**
  - Yank-proof tagging: tag commits, attach release notes referencing the Bot API + WebApp versions covered.
- [ ] **chore: trusted publishing**
  - OIDC-based crates.io publish from GitHub Actions; no long-lived API tokens stored.

Deliverable: `cargo add ratma-tg-types{,-core,-http,-webapp}` works, current Bot API + WebApp covered, RATMA repos on the new types.

---

## Out of scope

- **MTProto / TDLib types.** Different schema universe (TL, not JSON), different abstractions, thousands of types. Use `grammers` for that.
- **A full bot framework.** This crate family is types-first. The `http` crate is a convenience for current users of `botapi-rs`, not the focus.
- **Replacing teloxide's dispatcher, handlers, DPTree integration.** Those stay in `fork-teloxide`.
- **WebApp JS bindings via `wasm-bindgen`.** Rust types for WebApp init_data + payloads are in scope; wrapping `Telegram.WebApp.expand()` etc. for browser-side use is its own project.
- **Backporting to old Bot API versions.** We always track HEAD.

## Open questions

- Should `ratma-tg-types-ids` (`UserId`/`ChatId`/etc.) be a separate crate or live in `-core`? Argument for separate: WebApp crate avoids depending on full `-core` if it only needs ids. Argument against: yet another crate. Decision pending Phase 2 design.
- Codegen language: keep Rust (`tggen`) or rewrite in a scripting language for faster iteration? Rust wins — same toolchain as the output, easier to share types between generator and runtime tests.
- `serde-json-only` minimal variant of `core` (no `serde_json` even, just `serde`)? Probably not — `serde_json` is universally available and `ordered-float`'s serde impls assume it anyway.

## Status snapshot

| Phase | Status | Issue |
|---|---|---|
| 0 — Fork hygiene | **done** | #7 (closed via #8) |
| 1 — Workspace cleanup | **done** | #3 (structural), #11+#13 (deps cleanup) |
| 2 — Idiomatic codegen | not started | tbd |
| 2½ — WebApp ingestion | not started | tbd |
| 3 — LLM-in-the-loop | deferred | tbd |
| 4 — Auto-update infra | not started | tbd |
| 5 — Adoption | not started | tbd |
| 6 — Release | not started | tbd |

Last updated: 2026-05-17.
