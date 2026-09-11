# cargo-pup-clean-architecture-example

RustのClean Architectureを **cargo-pupで検査し、その検査が本当に違反を検出するかもGitHub Actionsで試す** サンプルです。

> これは設計の完全性を保証するツールではありません。特に `RestrictImports` は完全な依存グラフ検査ではなく、`use` の制約です。このレポでは、検出できる違反と既知の見逃しを両方実行して確認します。

## サンプルの構成

依存方向を読み取りやすくするため、タスクの作成・取得だけを扱う、外部crate依存なしの小さなCLIにしています。HTTPサーバーや実DBの起動は不要です。

```text
src/main.rs                  構成の組み立て（composition root）
  ├── presentation           Controller / 表示用データ
  │     └── application      タスク作成・取得のユースケース
  │           └── domain    Task / 検証ルール / TaskRepository trait
  └── infrastructure
        └── domain          InMemoryTaskRepositoryがtraitを実装
```

| 層 | 役割 | このサンプルでの依存方針 |
| --- | --- | --- |
| `domain` | エンティティ、業務上の検証、永続化のport | 他の3層を知らない |
| `application` | ユースケース | domainのtraitを通して永続化する |
| `presentation` | 入出力の変換 | applicationを呼び、具体的なストレージを知らない |
| `infrastructure` | 永続化のadapter | domainのtraitを実装し、application / presentationを知らない |
| `main.rs` | 具体実装の注入 | 各層を組み立てる例外的な場所 |

`TaskService<R: TaskRepository>` に保存先を注入します。テストでは利用不能なRepositoryに差し替え、ユースケースを変更せずにエラーを扱えることを確認します。この例ではportをdomainに置いています。すべてのプロジェクトにこの配置を要求するものではありません。

**単一crate・複数モジュールなのは意図的です。** Rustのコンパイル自体は通る設計違反を作り、cargo-pupとの差を観察できます。実製品の強い境界には、crate分割、依存許可リスト、可視性制御も併用してください。

## 動かす

通常の開発・テストはstable Rustだけで実行できます。

```sh
cargo run --locked -- "Write architecture tests"
# 1: Write architecture tests

cargo test --locked --all-targets
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

毎回メモリ内のRepositoryを作るデモなので、プロセスを終了するとタスクは消えます。

## cargo-pupを実行する

前提は `rustup` と **Python 3.11以上**。CIはLinuxで検証します。

```sh
python3 scripts/setup_pup.py
python3 scripts/check_architecture.py
```

ツールの組み合わせは [`pup-toolchain.toml`](pup-toolchain.toml) の一箇所で固定しています。

- `cargo_pup = 0.1.8`
- `nightly-2026-01-22` + `rust-src` / `rustc-dev` / `llvm-tools-preview`

アプリの通常ビルドをnightlyに変更する必要はありません。cargo-pupのための別のコンパイルでnightlyを使います。CLIはこのレポの `.tools/cargo-pup` にインストールします。

設計ルールだけを対話的に確認する場合：

```sh
export PATH="$PWD/.tools/cargo-pup/bin:$PATH"
# 設定変更後も古い解析を再利用しないよう、解析用のキャッシュを消す。
rm -rf .pup
cargo pup check --locked --all-targets
cargo pup print-modules --locked --lib
```

## 定義した設計ルール

設定の本体は [`pup.ron`](pup.ron) です。すべて `severity: Error` にしています。

| ルール名 | 禁止するimport |
| --- | --- |
| `domain_inward_only` | application / infrastructure / presentation、指定した外部I/O系crate、`std::fs` / `net` / `process` |
| `application_inward_only` | infrastructure / presentation、指定したHTTP・DB系crate |
| `presentation_no_direct_storage` | infrastructure、sqlx |
| `infrastructure_uses_domain_ports` | application / presentation |

モジュール名は `^clean_architecture::domain(::|$)` のように、層のルートと子モジュールの両方に一致させています。crateを改名したらこの指定も更新してください。標準ライブラリ等を全面禁止する設定ではなく、記載したパターンだけを禁止する例です。

例えばdomainに次を足す変更は、Rustとしてはコンパイル可能ですが、cargo-pupでは拒否する想定です。

```rust
pub use crate::infrastructure::InMemoryTaskRepository;
```

## 「検査が効いていること」もテストする

[`scripts/check_architecture.py`](scripts/check_architecture.py) は、各ケースで現在のコードと設定を新しい一時ディレクトリにコピーします。作業ツリーは変更しません。

1. 通常のnightlyコンパイラで `cargo check --locked --all-targets` が成功することを確認する。
2. 同じコードに `cargo-pup` を実行する。
3. 違反例では **終了コードが非ゼロで、期待したルール名とimport禁止の診断が出ること** を確認する。

単なるコンパイルエラー、設定ファイルの構文エラー、ツール未導入、警告だけで終了コード0の場合を「検出できた」と扱いません。毎回新しい `.pup` を作るため、設定変更がキャッシュに隠れることも避けます。

**13ケース**を実行します。正常例1件、禁止依存の検出11件、既知の見逃しの確認1件です。各層の禁止方向に加え、ネストしたモジュール、相対パス＋別名import、domainからファイルI/Oへのimportも含めています。外部crate名の禁止パターンは設定例であり、この依存なしサンプルでは実際のaxum/sqlx等をリンクして検証していません。

特定のケースだけ試すには：

```sh
python3 scripts/check_architecture.py --case domain_to_infrastructure
python3 scripts/check_architecture.py --case fully_qualified_path_known_gap
```

診断ログと一覧は `.test-artifacts/architecture/` に保存します。判定ロジック自体の単体テストはRustなしでも実行できます。

```sh
python3 -m unittest discover -s scripts/tests -v
```

### 既知の限界を隠さない

`RestrictImports` は、次のような **`use` を書かない完全修飾パス参照を禁止するものではありません**。

```rust
pub fn known_gap() -> crate::infrastructure::InMemoryTaskRepository {
    crate::infrastructure::InMemoryTaskRepository::default()
}
```

`fully_qualified_path_known_gap` は、この違反が検出されずに通る挙動を記録するテストです。CIの表示も `KNOWN GAP confirmed (not protection)` とし、「この依存を許してよい」という意味にしません。将来cargo-pupの改善で検出されるようになったら、テストと説明を更新してください。

ほかにも再exportを経由した参照、マクロ、解析対象にしていないfeature / targetなどは別途検証が必要です。このCIはUbuntu上のデフォルトfeatureで `--all-targets` を検査します。機能仕様の正しさ、セキュリティ、実DB接続、デプロイ・起動の成功までは保証しません。また、ルールとテストを同時に弱める変更には、CODEOWNERS等を含むレビュー運用が必要です。

## GitHub Actions

[`CI`](.github/workflows/ci.yml) はPR、mainへのpush、手動実行に対応しています。

| ジョブ | 検査 |
| --- | --- |
| **Rust quality** | rustfmt、Clippy、業務・CLIのテスト、検査スクリプトの単体テスト |
| **Architecture contracts** | 固定nightly + cargo-pup、正常例・違反例・既知の限界の検証 |

違反例は **期待した違反を検出できたときにCIが緑になる** 仕組みです。一方、実コードに禁止importが入れば正常例の検査でCIが赤になります。

cargo-pupのインストール済みバイナリはキャッシュしますが、解析結果の `.pup` は再利用しません。実行の重複はキャンセルし、タイムアウトを設定。ActionsはコミットSHAで固定し、GitHubトークンは `contents: read` のみです。診断ログはActions Artifactに7日間保存し、ケース一覧をJob Summaryに出します。

**CIの失敗をマージ禁止条件にする設定は別です。** GitHubのSettings → Rules → Rulesets（またはBranchesの保護設定）でmainを対象に、`Rust quality` と `Architecture contracts` を必須ステータスチェックに指定してください。このPRからリポジトリの保護設定は変更しません。手動実行はワークフローがデフォルトブランチに入ってから利用できます。

## 参考

- [DataDog / cargo-pup](https://github.com/DataDog/cargo-pup)
- [公式の導入手順・Rustによるルール定義例](https://github.com/DataDog/cargo-pup#readme)
- [RestrictImportsの実装を確認したupstreamソース](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_impl/src/lints/module_lint/lint.rs)

このレポはツールの評価・学習用です。ルールを増やす前に、小さな違反例を追加して「何を保証できるか」を確認することを推奨します。
