# cargo-pup-clean-architecture-example

RustのClean Architectureを、[cargo-pup](https://github.com/DataDog/cargo-pup)でどこまで検査できるか試すサンプル。

**正常なアプリだけでなく、コンパイルは通る設計違反と、検査をすり抜ける既知のケースも自動テストする。** 「CIが緑だから設計が完全に守られている」と誤解しないための実験用レポジトリです。

> 実機検証で、`StructRule::ImplementsTrait`がAPIには存在するものの、0.1.8では検査を実行しないことも確認しました。traitの必須実装をcargo-pupが保証するサンプルではありません。[調査記録](docs/findings.md)を参照してください。

## まず動かす

Rust（rustup）があれば、通常のアプリとテストはstableで実行できます。アプリ本体・テストコードともに外部crateへの依存はありません。

```sh
cargo run --locked --bin task-demo -- '設計をテストする'
cargo test --locked --all-targets
```

出力例:

```text
Created task #1: 設計をテストする
```

タスクのタイトルを検証してメモリに保存する最小例です。空白だけのタイトルや120文字（Unicode scalar value）を超えるタイトルを拒否し、同一IDのタスクを上書きしません。保存先はプロセス内メモリのみで、CLI終了時に消えます。HTTPサーバー、DB、認証などは含めていません。

## 設計を検査する

このサンプルの検証対象を **cargo-pup 0.1.8 + nightly-2026-01-22** に固定しています。普段のRustをnightlyに切り替える必要はありません。

```sh
# 専用nightlyとcargo-pup / pup-driverをインストールする
bash scripts/install-cargo-pup.sh

# アプリのライブラリを検査する
cargo pup --lib

# 対象モジュール・traitを確認する
cargo pup print-modules --lib
cargo pup print-traits --lib

# 正常例・違反例・既知のすり抜けをまとめて検証する
cargo architecture-test
```

`cargo architecture-test`は次のコマンドのaliasです。

```sh
cargo test --test architecture -- --ignored --nocapture
```

通常の`cargo test`ではnightly依存のこのテストを明示的に`ignored`にしています。**通常のテストが通っただけでは、設計検査を実行したことにはなりません。** 専用コマンドとCIの別ジョブで実行します。専用コマンドはツール未導入・ビルドエラー・設定エラーを成功扱いしません。

## Clean Architectureの構成

矢印は「ソースコード上の依存」を表します。

```text
presentation ───────> application ───────> domain
                           ^                ^
                           │                │
infrastructure ────────────┴────────────────┘

main.rs: 具象実装を選択し、各層を接続するcomposition root
```

| ファイル | 役割 |
| --- | --- |
| `src/domain.rs` | `Task`とタイトルの不変条件。DBやUIを知らない |
| `src/application.rs` | `CreateTask`ユースケースと`TaskRepository` port。具象保存先を知らない |
| `src/infrastructure.rs` | portを実装する`InMemoryTaskRepository` |
| `src/presentation.rs` | 入出力DTOとcontroller。ユースケースを呼び出す |
| `src/main.rs` | 具体的なRepositoryを注入してCLIを動かす |

portを内側に置き、外側のadapterに実装させるのが依存性逆転です。`CreateTask<R: TaskRepository>`の型制約により、**実際に注入されるadapterのtrait実装はRustコンパイラが要求**します。テストでは別のRepository実装を渡して、保存失敗も再現します。

今回は**意図的に1 crate内のモジュールで構成**しています。Rustとして合法な層間参照をcargo-pupが検出する様子を見るためです。実際の大きなプロジェクトではcrate分割・可視性・Cargo依存関係の検査も併用する設計が候補になります。このサンプルにCargo依存グラフの検査は実装していません。

## 守るルールと観測用ルール

ルールは[`pup.ron`](pup.ron)に一元化しています。アプリと全fixtureが同じ設定を使います。RONはcargo-pupの設定形式です。

| ルール名 | 対象と制約 |
| --- | --- |
| `domain_no_outer_imports` | domainと子モジュールからapplication / infrastructure / presentationなどのimportを禁止 |
| `application_no_adapters` | applicationからinfrastructure / presentationなどのimportを禁止 |
| `presentation_no_infrastructure` | presentationからinfrastructure / sqlxのimportを禁止 |
| `infrastructure_no_presentation` | infrastructureからpresentationのimportを禁止 |
| `repository_adapters_public` | 名前が`TaskRepository`で終わるstructは`pub`とする、このサンプルの公開範囲ルール |

公開範囲のルールはClean Architecture一般の必須条件ではなく、別のbinary crateであるCLIからadapterを構築するこのサンプルの規約です。5つのルールについて、違反の名前付き診断をテストします。

さらに、`probe_repository_trait_requirement`を**観測専用**で置いています。`StructRule::ImplementsTrait`の未実装を再現するための設定であり、現行版で設計を強制するルールには数えません。ツールの更新で挙動が変わったら、テストと説明を見直します。

importルールには、将来のうっかりした導入に備えて一部フレームワーク名やdomainからの直接I/Oも禁止パターンとして含めています。ただし、**任意の外部依存を網羅する許可リストではありません**。fixtureで確認する中心は上記の層間参照です。

例:

```ron
Module((
    name: "domain_no_outer_imports",
    matches: Module("^clean_architecture::domain(::|$)"),
    rules: [
        RestrictImports(
            allowed_only: None,
            denied: Some(["(^|::)(application|infrastructure|presentation)(::|$)"]),
            severity: Error,
        ),
    ],
)),
```

パターンはglobではなく正規表現です。`(::|$)`でルートモジュール自身と子モジュールの両方を対象にしています。上は説明用の抜粋で、実行時には完全な`pup.ron`を使用します。

## 「検査が効くこと」をテストする

[`tests/architecture.rs`](tests/architecture.rs)は、通常のRust統合テストからcargo-pupを子プロセスとして実行します。追加のテストフレームワークは不要です。

| ケース | 普通のRustコンパイル | cargo-pupの期待結果 |
| --- | --- | --- |
| 実際の`src/`のアプリ | 成功 | 成功 |
| domain → infrastructure | 成功 | 名前付きルールで失敗 |
| application → infrastructure | 成功 | 名前付きルールで失敗 |
| presentation → infrastructure | 成功 | 名前付きルールで失敗 |
| infrastructure → presentation | 成功 | 名前付きルールで失敗 |
| domainの子モジュール → presentation | 成功 | 名前付きルールで失敗 |
| `pub`ではないRepository | 成功 | 名前付きルールで失敗 |
| 完全修飾パスでdomain → infrastructure | 成功 | **成功してしまう（既知の限界）** |
| portを実装していない、未使用のRepository | 成功 | **成功してしまう（未実装ルール）** |

各ケースを独立した一時crateへコピーし、**先に同じnightlyで通常の`cargo check`が通ることを確認**します。その後、6つの違反例ではcargo-pupの非ゼロ終了だけでなく、期待したルール名の診断まで確認します。構文エラーやツールのクラッシュを「検出できた」と数えません。

末尾の2件は「設計上正しい」というテストではなく、ツールの現状を固定するcharacterization testです。この2件以外の違反を見逃したら、テスト全体が失敗します。

一時ディレクトリを使うのでソースを自動改変せず、Cargoの親プロセスとビルドロックも共有しません。失敗時には調査用の一時ディレクトリの場所を表示して残します。専用テストのログに表示される6件のlintエラーは**期待する出力**で、最終的なRustテストの結果が成功かどうかを見てください。

## 手で壊してみる

`src/domain.rs`のimport部分に次を追加して、`cargo pup --lib`を実行してください。

```rust
use crate::infrastructure::InMemoryTaskRepository;
```

Rustでは未使用importの警告になるだけですが、cargo-pupでは`domain_no_outer_imports`のエラーになる想定です。確認後は追加した行を削除してください。

## このツールに任せきれないこと

**`RestrictImports`は型や呼び出しの完全な依存解析ではありません。** 現行実装は`use`のパスを検査しており、次のような参照には`use`がありません。

```rust
pub fn forbidden_dependency() -> crate::infrastructure::Database {
    crate::infrastructure::Database
}
```

これを[`fully_qualified_bypass.rs`](tests/fixtures/fully_qualified_bypass.rs)に残しています。

**`StructRule::ImplementsTrait`も0.1.8では未処理です。** traitで対象structを選ぶ`StructMatch::ImplementsTrait`とは別の機能です。必須実装はRustの型制約で保証し、cargo-pupに任せきらないでください。[再現fixture](tests/fixtures/missing_repository_trait.rs)と[調査記録](docs/findings.md)があります。

aliasやre-export、名前の変え方による抜け道の網羅検査はしていません。Repositoryのルールも命名規約に一致するstructだけが対象です。`cargo pup --lib`は選択したライブラリ・ビルド条件を対象とし、すべてのfeature / target / `cfg(test)`の組み合わせを検証するものではありません。`main.rs`は外側を接続するため、層ルールの対象にしていません。

cargo-pup 0.1.8の`ModuleRule::And / Or / Not`には検査をスキップする実装があるため、ここでは使用していません。複数の独立ルールを並べています。matcherの`AndMatches`などとは別の話です。

**設計ルールの回帰検出を補助するサンプルであり、依存関係やセキュリティの完全な保証ではありません。**

## CI

`.github/workflows/ci.yml`は次の2ジョブを独立して実行します。

- **Rust tests and lint**: stableでfmt / clippy / 通常テスト10件 / CLI起動。
- **cargo-pup contract tests**: 指定nightlyとcargo-pupを導入し、上記9ケースを実行。

バージョンを変更する際は、`scripts/install-cargo-pup.sh`、`tests/architecture.rs`、このREADMEの組を更新し、既知の限界を含めて再確認してください。

## 参照したupstream

設定と注意点はDataDog/cargo-pupの次の公開実装を参照しています。リンクは調査時のcommitに固定しています。CIで導入する配布物はcrates.ioの`cargo_pup = 0.1.8`です。

- [READMEと導入手順](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/README.md)
- [Module lint実装](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_impl/src/lints/module_lint/lint.rs)
- [Struct rule定義](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_config/src/struct_lint/types.rs)と[検査実装](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_impl/src/lints/struct_lint/lint.rs)

## License

[MIT](LICENSE)
