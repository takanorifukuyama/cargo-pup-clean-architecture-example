# cargo-pup-clean-architecture-example

RustのClean Architectureを、**Rustマクロで宣言した設計ルール + `cargo test` + cargo-pup** で検証するサンプルです。

## テストはここ

**ルールの本体は [`tests/architecture.rs`](tests/architecture.rs) です。** PythonやRONではなく、Rustのテストとして読み書きします。

```rust
architecture_rules! {
    domain_inward_only {
        module: clean_architecture::domain,
        deny_imports: [
            crate::application,
            crate::infrastructure,
            crate::presentation,
            axum,
            sqlx,
            tokio,
            reqwest,
            std::fs,
            std::net,
            std::process,
        ],
    }
}
```

`domain_inward_only` という通常の `#[test]` 関数が生成されます。現在のソースを一時コピーしてこのルールを検査し、禁止importがあればテストが失敗します。層のルートだけでなく子モジュールも対象です。

**このマクロは、このサンプルで実装した薄い `macro_rules!` ラッパーです。cargo-pupの標準マクロではありません。** マクロが依存関係を解析するのではなく、実行時に設定を生成してcargo-pupを呼び出します。設定は一時ディレクトリだけに生成するため、手書きの `pup.ron` とRustルールを二重管理しません。

## 実行する

初回は `rustup` とPython 3.11以上で、固定版の外部ツールをインストールします。Pythonが必要なのはこのセットアップだけで、テスト本体・ケース定義・判定ロジックはRustです。

```sh
python3 scripts/setup_pup.py

# 設計ルール・違反検出・既知の限界・判定ロジックを実行
cargo test --locked --features architecture-tests --test architecture

# 通常のRustテストフィルターで1件だけ実行
cargo test --locked --features architecture-tests --test architecture domain_inward_only -- --exact
cargo test --locked --features architecture-tests --test architecture violations::domain_to_infrastructure -- --exact

# テスト名の一覧
cargo test --locked --features architecture-tests --test architecture -- --list
```

普段の業務テストはstableだけで実行できます。

```sh
cargo run --locked -- "Write architecture tests"
cargo test --locked --all-targets
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings

# ハーネスの単体テストだけならcargo-pupもPythonも不要
cargo test --locked --features architecture-tests --test architecture support::tests
```

`architecture-tests` は重い外部検査を明示的に有効化するfeatureです。通常の `cargo test` には設計テストが含まれません。**GitHub Actionsの `Architecture contracts` はこのfeatureを必ず指定します。** feature有効時にツール未導入・バージョン不一致なら明確に失敗し、黙ってスキップしません。

## 違反を本当に止められるかも、Rustでテストする

[`tests/architecture/violations.rs`](tests/architecture/violations.rs) に違反例を置いています。

```rust
architecture_violation! {
    domain_to_infrastructure {
        rule: domain_inward_only,
        in_file: "src/domain.rs",
        add: { pub use crate::infrastructure::InMemoryTaskRepository; }
    }
}
```

これも通常の `#[test]` になります。`add` 内のRustトークンは `stringify!` でテスト用のコードに変換し、新しい一時コピーだけに追加します。元の作業ツリーは変更しません。

1. 同じ固定nightlyで通常の `cargo check --locked --all-targets` が成功することを確認。
2. **同じRustルール**から設定を生成してcargo-pupを実行。
3. 違反例は非ゼロの終了コード、期待したルール名、import禁止のエラー診断を確認。

「想定した設計違反で落ちること」がテスト成功です。構文エラー、設定エラー、警告のみ、シグナル終了やパニックを検出成功にはしません。毎回新しい解析用ディレクトリを使い、ルール変更が古い `.pup` キャッシュに隠れないようにしています。通常コンパイルにも一時コピー専用のtargetを指定し、外側の `cargo test` のロックと競合させません。

アーキテクチャの実行ケースは **16件（実コードへの4ルール + 違反11件 + 既知の見逃し1件）**。これに、判定・パターン生成・固定バージョン読み取り・一時ディレクトリなどのRust単体テスト15件が加わります。並列実行してもfixtureとログをケースごとに分離します。

## サンプルの構成

アプリもテストハーネスも外部crate依存なしです。HTTPサーバーや実DBは不要で、タスク作成・取得だけの小さなCLIです。

```text
src/main.rs                  composition root / 実装の注入
  ├── presentation           Controller / 表示用データ
  │     └── application      タスク作成・取得のユースケース
  │           └── domain    Task / 検証 / TaskRepository trait
  └── infrastructure
        └── domain          InMemoryTaskRepositoryがtraitを実装

tests/architecture.rs                  マクロによる4つのルール
  └── architecture/violations.rs        違反11件 + 既知の見逃し
      architecture/support.rs          マクロ・cargo-pup実行ハーネス
      architecture/support_tests.rs    ハーネスの単体テスト

tests/behavior.rs                      通常の業務・CLIテスト9件
```

`TaskService<R: TaskRepository>` に保存先を注入します。portをdomainに置くのはこの例の方針であり、全プロジェクトへの要求ではありません。`infrastructure` から `application` を禁止しているのも、この配置に基づくサンプル固有の方針です。

| ルール | 禁止するimport |
| --- | --- |
| `domain_inward_only` | 他の3層、指定したI/O系crate、`std::fs` / `net` / `process` |
| `application_inward_only` | infrastructure / presentation、指定したHTTP・DB系crate |
| `presentation_no_direct_storage` | infrastructure、sqlx |
| `infrastructure_uses_domain_ports` | application / presentation |

単一crateなのは意図的です。Rustのコンパイル自体は通る設計違反を作り、cargo-pupとの差を観察します。実製品の強い境界にはcrate分割、依存許可リスト、可視性制御も併用してください。メモリ内Repositoryなので、CLIを終了するとタスクは消えます。

## マクロで改善すること・しないこと

改善するのは **ルールの書きやすさ、テストの見つけやすさ、`cargo test` / IDEとの統合** です。コンパイル時の型チェックだけで設計を保証する仕組みではありません。

- `module` と `deny_imports` は単純なASCII Rustパスを受け取り、正規表現に変換します。ジェネリック、raw identifier、Unicode識別子には未対応で、黙って解釈せずエラーにします。
- `crate::infrastructure` は従来と同じ `(^|::)infrastructure(::|$)` に変換します。相対importも対象にしますが、解決済みの型・モジュールIDを追うものではなく、同名のパス区間への過検出や再export経由の見逃しはあり得ます。
- axum / sqlxなどは設定例です。この依存なしサンプルでは、実際の外部crateをリンクするケースまでは検証していません。

### 既知の見逃しは残る

`RestrictImports` は完全な依存グラフ検査ではありません。例えば、`use` を使わない直接参照は検出されません。

```rust
pub fn known_gap() -> crate::infrastructure::InMemoryTaskRepository {
    crate::infrastructure::InMemoryTaskRepository::default()
}
```

`fully_qualified_path_known_gap` でこの挙動を明示的に記録し、結果も **`KNOWN GAP confirmed (not protection)`** と表示します。「許してよい依存」という意味ではありません。改善で検出されるようになれば、このテストを変更します。マクロ化しても解析精度やnightlyへの依存は変わりません。

feature / targetの網羅、再exportやマクロ展開を経由する参照などは別途検証が必要です。今回の解析はUbuntuのデフォルトfeatureで `--all-targets` を対象とし、機能仕様、セキュリティ、実DB、デプロイ・起動の成功までは保証しません。ルールとテストを同時に弱める変更にはレビューが必要です。

## CIとツール固定

[`pup-toolchain.toml`](pup-toolchain.toml) の1箇所で `cargo_pup = 0.1.8` / `nightly-2026-01-22` を固定しています。アプリとテストランナーはstableで動き、cargo-pupによる別コンパイルだけでnightlyを使います。

| GitHub Actionsのジョブ | 検査 |
| --- | --- |
| **Rust quality** | rustfmt、全featureのClippy、業務テスト、Rustハーネス単体テスト |
| **Architecture contracts** | 固定cargo-pup導入 + `cargo test --features architecture-tests --test architecture` |

PR・mainへのpush・手動実行に対応しています。インストール済みツールだけをキャッシュし、解析結果は再利用しません。各外部コマンドのタイムアウトとジョブのタイムアウトを設定しています。

診断ログ、実際に生成したRON、ケースごとの結果を `.test-artifacts/architecture/` に出力します。Actions Artifactで7日間保存し、Job Summaryにも結果を表示します。ActionsはコミットSHA固定、トークンは `contents: read` のみです。

CI失敗をマージ禁止にするには、mainのRulesetで `Rust quality` と `Architecture contracts` を必須ステータスチェックにしてください。この変更ではブランチ保護設定を変更しません。

## 参考

- [DataDog / cargo-pup](https://github.com/DataDog/cargo-pup)
- [RestrictImportsのupstream実装](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_impl/src/lints/module_lint/lint.rs)

マクロはサンプルローカルの実装です。crateとして公開したものではありません。
