# cargo-pup-clean-architecture-example

RustのClean Architectureを、**Rustマクロ + `cargo test` + cargo-pup** で検証するサンプルです。設計ルールだけでなく「わざと壊したコードを本当に止められるか」もテストします。

## テストはここ

| 読むファイル | 内容 |
| --- | --- |
| [`tests/type_contracts.rs`](tests/type_contracts.rs) | Rustの型境界によるtrait / Send / Syncの契約とコンパイル失敗の対照実験 |
| [`tests/architecture.rs`](tests/architecture.rs) | import制約と構造体の公開範囲を宣言するマクロ |
| [`tests/architecture/violations.rs`](tests/architecture/violations.rs) | 禁止importの違反例、完全修飾パスの既知の見逃し |
| [`tests/architecture/extended.rs`](tests/architecture/extended.rs) | 公開範囲違反、対象名の誤字、cfgで消える対象、確認用lintの抑制 |
| [`tests/architecture/support.rs`](tests/architecture/support.rs) | cargo-pup呼び出し、設定生成、検査対象の空振り防止 |
| [`tests/common/type_contracts.rs`](tests/common/type_contracts.rs) | 型契約マクロ本体 |
| [`tests/common/runtime.rs`](tests/common/runtime.rs) | タイムアウト、一時ディレクトリ、コンパイラ実行の共通処理 |
| [`tests/behavior.rs`](tests/behavior.rs) | 業務・CLIテスト |

すべてサンプルローカルの `macro_rules!` です。cargo-pup標準のマクロでも、公開crateでもありません。

### 1. 型の契約はRustコンパイラで守る

```rust
type_contracts! {
    repository_contract {
        type: InMemoryTaskRepository,
        implements: [TaskRepository, Send, Sync],
    }
}
```

通常の `#[test]` に展開し、ジェネリックのtrait境界で具体型を検証します。traitの実装を削除したり、型が `Send` / `Sync` を満たさなくなれば、**テストのコンパイル時点で失敗**します。値の生成やメソッド呼び出しは不要です。

この契約はstableで動き、通常の `cargo test` に含まれます。cargo-pupの `StructRule::ImplementsTrait` は使いません。確認した0.1.8の実装ではそのruleは処理されておらず、trait実装済みの型を選ぶmatcherと混同できないためです。

対照実験は、同じマクロを使う小さな独立fixtureを `rustc --test --emit=metadata` でコンパイルします。まず契約なしのfixtureがコンパイルできることを確認し、その後、trait未実装・非Send・非Syncが **E0277と期待した境界の診断** で失敗することを検証します。成功するfixtureも用意しています。実アプリの具体型への契約と、マクロ自体の対照実験は別のテストです。

### 2. importの依存方向を守る

```rust
architecture_rules! {
    domain_inward_only {
        module: clean_architecture::domain,
        deny_imports: [
            crate::application,
            crate::infrastructure,
            crate::presentation,
            std::fs,
        ],
    }
}
```

実際のルールは [`tests/architecture.rs`](tests/architecture.rs) にあります。層のルートと子モジュールを対象に、実行時にRONを生成してcargo-pupを呼びます。手書き `pup.ron` との二重管理はありません。

```rust
architecture_violation! {
    domain_to_infrastructure {
        rule: domain_inward_only,
        in_file: "src/domain.rs",
        add: { pub use crate::infrastructure::InMemoryTaskRepository; }
    }
}
```

`add` はRustトークンを `stringify!` でfixtureへ追加するマクロです。元の作業ツリーは変更せず、違反コードが正常にコンパイルできた後にcargo-pupで拒否されることを確認します。

### 3. 内部型の公開範囲を守る

```rust
visibility_rules! {
    task_store_stays_internal {
        struct_name: TaskStore,
        visibility: PubCrate,
    }
}
```

`TaskStore` はRepository内部の保存形式です。`PubCrate` は `MustBePubCrate(Error)` を使い、crate内に公開する契約を検査します。`Public` / `Private` も指定できます。

`pub(crate)` を `pub` に広げる変更、privateやそのモジュール内だけへ狭める変更は、fixtureとして通常コンパイルできても、この契約では拒否します。ここでは「公開上限」ではなく**指定した可視性の契約**です。`pub(super)` などがcrate rootに解決される場合は、cargo-pupの実装上 `pub(crate)` 相当として扱われます。

**cargo-pup 0.1.8のstruct matcherは短い型名だけを見ます。** そのためフィールド名も `struct_name` とし、完全修飾パスは受け付けません。解析対象library内の同名structはすべて対象になります。パスで厳密に一つの型を指定する機能ではありません。

## 「対象がないのに成功」を防ぐ

import・公開範囲ルールは、実ルールより先に **同じselectorに対する確認用lint（canary）** を実行します。

```text
現在のソースを新しい一時コピーへ
  → 通常のcargo checkが成功することを確認
  → 同じselectorにMustBeNamed("^$", Error)を適用
  → 存在するRust名は空文字にならないため、対象があれば必ず診断が出る
  → 確認用ルール名と想定したエラーを確認
  → .pupを削除し、実際の設計ルールで再検査
```

モジュール・struct名の誤字、cfgで対象が消えるケース、確認用lintを抑制したケースを「成功」として通しません。空振りの対照実験では専用の `MissingTarget` エラーのみを期待し、ツール未導入・設定不正・通常コンパイル失敗は対照実験の成功になりません。

これは**少なくとも一つの対象を実際に検査できた証拠**を取る仕組みで、対象件数を数える機能や、すべての子モジュールでlintが有効だという保証ではありません。ルールと確認用lintを意図的に異なる方法で抑制する変更や、テストごと削除する変更を防ぐセキュリティ境界でもありません。

確認用lintと実ルールは同じデフォルトfeature・`--lib`スコープで実行します。設定変更のキャッシュ漏れを避けるため、各実行の前にfixture内の `.pup` を削除します。通常の事前コンパイルは引き続き `--all-targets` です。

## 実行する

```sh
# stableのみ。型契約とコンパイル失敗の対照実験も含む
cargo test --locked --all-targets
cargo test --locked --test type_contracts

# 初回だけ。rustupとPython 3.11以上が必要
python3 scripts/setup_pup.py

# cargo-pupを使う設計ルール・違反例・空振り対策
cargo test --locked --features architecture-tests --test architecture

# 1件だけ / 一覧
cargo test --locked --features architecture-tests --test architecture task_store_stays_internal -- --exact
cargo test --locked --features architecture-tests --test architecture -- --list

# ハーネス単体テストだけならcargo-pupもPythonも不要
cargo test --locked --features architecture-tests --test architecture support::

cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

`architecture-tests` は外部検査の明示的なopt-inです。**通常の `cargo test` に入るのは型契約と業務テストで、cargo-pupの検査は含まれません。** CIの `Architecture contracts` は必ずfeatureを指定します。ツール未導入やバージョン不一致はスキップせず失敗します。

外部crate依存は追加していません。Pythonは既存のツール導入スクリプトだけに残っています。型契約の対照実験では `rustup run stable rustc` を使うため、stable toolchainを導入してください。

## サンプルの構成と依存方針

```text
src/main.rs                  composition root / 実装の注入
  ├── presentation           Controller / 表示用データ
  │     └── application      タスク作成・取得のユースケース
  │           └── domain    Task / 検証 / TaskRepository trait
  └── infrastructure
        └── domain          InMemoryTaskRepositoryがtraitを実装
```

| ルール | 禁止するimport |
| --- | --- |
| `domain_inward_only` | 他の3層、指定したI/O系crate、`std::fs` / `net` / `process` |
| `application_inward_only` | infrastructure / presentation、指定したHTTP・DB系crate |
| `presentation_no_direct_storage` | infrastructure、sqlx |
| `infrastructure_uses_domain_ports` | application / presentation |

`TaskService<R: TaskRepository>` に保存先を注入します。portをdomainに置くこと、infrastructureからapplicationを禁止することはこのサンプルの選択であり、すべてのClean Architectureへの要求ではありません。メモリ内Repositoryなので、CLIを終了するとタスクは消えます。

単一crateなのは意図的です。Rustとして合法な設計違反をcargo-pupで検査する差を観察できます。複数crateの依存許可リストや `cargo metadata` 検査は次の拡張範囲で、今回には含めていません。

## 保証しないこと

`RestrictImports` は解決済みの完全な依存グラフを検査しません。`use` を書かない次の直接参照は、既知の見逃しとして残ります。

```rust
pub fn known_gap() -> crate::infrastructure::InMemoryTaskRepository {
    crate::infrastructure::InMemoryTaskRepository::default()
}
```

`fully_qualified_path_known_gap` は **`KNOWN GAP confirmed (not protection)`** と区別します。許容する依存という意味ではありません。解析改善で検出されるようになれば、テストを更新します。

import selectorは単純なASCIIパスを正規表現化します。`crate::infrastructure` は `(^|::)infrastructure(::|$)` に変換するため、同名区間の過検出や再exportの見逃しがあり得ます。generic / raw identifier / Unicodeのモジュール指定は未対応で、黙って解釈せずエラーにします。axum / sqlx等は設定例で、この依存なしサンプルでは実際の外部crateをリンクして検証していません。

型契約は指定した具体型とtrait境界の関係だけを保証します。業務仕様、traitの意味的正しさ、セキュリティ、全feature / target、マクロや再export経由の参照、実DB、デプロイ成功は別の検証が必要です。

## CIとログ

[`pup-toolchain.toml`](pup-toolchain.toml) で `cargo_pup = 0.1.8` / `nightly-2026-01-22` を固定します。通常のテストと型契約はstable、cargo-pupの別コンパイルだけnightlyです。

| ジョブ | 検査 |
| --- | --- |
| **Rust quality** | rustfmt、全featureのClippy、業務・CLI、型契約とコンパイル対照実験、ハーネス単体テスト |
| **Architecture contracts** | cargo-pupのimport・公開範囲・空振り・既知の見逃しを検証 |

PR・mainへのpush・手動実行に対応。診断は `.test-artifacts/architecture/` と `.test-artifacts/type-contracts/` に保存します。Actions Artifactは7日間保持し、設計ルールはJob Summaryにも表示します。各コマンドは180秒でタイムアウトし、Unixでは子孫プロセスも停止させます。

違反判定では、**同じエラーブロック内のルール名と診断内容**を確認し、無関係のエラーと別の警告を寄せ集めて成功にはしません。変異は新しい一時コピーだけに適用し、置換対象は必ず一箇所であることを確認します。

ActionsはSHA固定、トークンは `contents: read` のみ。インストール済みツールだけをキャッシュし、解析結果は再利用しません。CI失敗をマージ禁止にするには、mainのRulesetで `Rust quality` / `Architecture contracts` を必須にしてください。このPRはブランチ保護を変更しません。

## 参考

- [Rust Reference: Trait and lifetime bounds](https://doc.rust-lang.org/reference/trait-bounds.html)
- [DataDog / cargo-pup](https://github.com/DataDog/cargo-pup)
- [確認したstruct lint実装](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_impl/src/lints/struct_lint/lint.rs)
