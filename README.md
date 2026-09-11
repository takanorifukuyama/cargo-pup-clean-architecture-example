# cargo-pup-clean-architecture-example

RustのClean Architectureのルールを **cargo-pupでどこまで検査できるか** を試す、小さな注文アプリ。
「正しいコードが通る」だけでなく、**コンパイル可能な設計違反が検出されること**と、**検出されない書き方**もテストする。

> 実験用サンプル。cargo-pupの `RestrictImports` は `use` 文のパスを正規表現で検査する仕組みで、解決済みの依存グラフを保証するものではない。この制約を隠さず、回帰テストとして残している。

## まず動かす

[rustup](https://rustup.rs/) が必要。アプリ本体とテストハーネスに外部crate依存はない。DB・Webサーバーも不要。

```sh
cargo run --locked
# order #1: 1200 JPY

cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

通常の `cargo test` は8件の業務テストを実行する。cargo-pupが必要な13件は明示的な `#[ignore]` とし、別のコマンドとCIジョブで必ず実行する。ツールが未導入だから黙って成功する、という実装ではない。

## 構成と依存方向

```text
src/main.rs  (composition root: 具体実装の組み立て)
    ├── presentation ──→ application ──→ domain
    └── infrastructure ──→ application::OrderRepository
                         └────────────→ domain
```

| 層 | このサンプルでの役割 | 禁止するimport |
| --- | --- | --- |
| `domain` | 注文の値と不変条件、金額のoverflowチェック | application / infrastructure / presentation |
| `application` | 注文ユースケース、保存用のport（trait） | infrastructure / presentation |
| `infrastructure` | portを実装するインメモリ保存adapter | presentation |
| `presentation` | 入力をuse caseへ渡し、出力を整形するadapter | infrastructure |
| `main` | adapterの選択と依存の注入 | 上の制約の対象外 |

保存の呼び出しは実行時にはapplicationからinfrastructureへ進むが、**ソースコードの依存方向はinfrastructureからapplicationが定義するtraitへ向く**。これがこの例で示したい依存性逆転。

層をあえて **1つのcrate内のmodule** にしている。Rustの型チェックだけでは通る「設計上は不正な依存」を作り、cargo-pupの効果を観察するため。Clean Architectureはフォルダを4つ作ること自体が目的ではない。

```text
src/                    実際に動く注文アプリ（4層 + composition root）
pup.ron                 4つのimport制約の唯一の定義元
tests/use_cases.rs      振る舞いとrepository差し替えのテスト
tests/architecture.rs   正常系・違反・既知の見逃しを実コードのコピーで検査
scripts/install-pup.sh  nightlyと上流commitを固定してcargo-pupを導入
.github/workflows/      通常のRust検査とarchitecture検査を別ジョブで実行
```

HTTPやSQLの仕組みを追わなくてよいように、初期版はframework-freeにしている。後からAxumやSQLxを足すなら外側のadapterを置き換え、内側の層を変えずに済む構造を目指す。

## cargo-pupを動かす

通常の開発は `rust-toolchain.toml` のRust **1.93.0**。cargo-pup用の **nightly-2026-01-22** は別に導入する。

```sh
bash scripts/install-pup.sh

# どのmoduleにどのルールが当たるかを見る
cargo pup print-modules

# アプリの設計ルールを検査
cargo pup

# ルール自体のテストを明示的に実行
cargo test --locked --test architecture -- --ignored --nocapture --test-threads=1

# 1ケースだけ試す
cargo test --locked --test architecture domain_rejects_infrastructure -- --ignored --nocapture
```

cargo-pupはバージョン表記 **0.1.8** の上流commit `a2c06497096123d4d37f622ddadb934831c60e92` から `--locked` でインストールする。「その時点のlatest」へ勝手に追従しない。インストールにはネットワークとコンパイル環境が必要。

テストは一時ディレクトリにアプリの `src/` と同じ `pup.ron` をコピーし、違反コードを追加する。元のソースは変更しない。各ケースでnightlyによる普通の `cargo check` が成功してからcargo-pupを実行し、違反時は **終了コードだけでなく、想定したルール名とimport拒否の診断** を確認する。設定ミス、未インストール、単なるコンパイルエラーを「検出できた」と数えない。

fixtureは外部依存なしでオフライン検査する。cargo-pupの解析ビルドは通常ビルドと別の `.pup/` を使う。assertion失敗時のfixtureパスはログに表示される。

## 何を試しているか

| 分類 | ケース数 | 期待するcargo-pupの結果 |
| --- | ---: | --- |
| 正しいアプリ | 1 | 成功 |
| 各層からの禁止import | 7 | 対象の設計ルールで失敗 |
| domainの子module / `super::` / グループimport | 3 | 対象の設計ルールで失敗 |
| 完全修飾パス / rootで別名再export | 2 | **現状は成功してしまう** |

最後の2件は「許可された設計」ではなく、既知の見逃しを記録する **characterization test**。上流の検出能力が改善されてテストが落ちた場合、改善を確認して期待値と説明を更新する。検査を無効化して緑に戻さない。

### 検出したい例

`src/domain.rs` に以下を加えると、Rustとしてはコンパイルできるが `domain_no_outer_imports` で拒否されることをテストする。

```rust
pub use crate::infrastructure::InMemoryOrderRepository as ArchitectureProbe;
```

該当ルールの抜粋:

```ron
Module((
    name: "domain_no_outer_imports",
    matches: Module("^clean_architecture_example::domain(::|$)"),
    rules: [
        RestrictImports(
            allowed_only: None,
            denied: Some(["(^|::)(application|infrastructure|presentation)(::|$)"]),
            severity: Error,
        ),
    ],
))
```

`(::|$)` によって層そのものと子moduleを含め、`domain_extra` のような別名は対象にしない。import禁止パターンは `crate::` と `super::` の両方に対応するため完全なpath segmentを照合する。パターンはglobではなく正規表現。

設定にはdomain/applicationから `axum` / `sqlx` / `tokio` / `reqwest`、presentationから `sqlx` をimportする場合の禁止も含む。ただしこの初期版にそれらの依存はなく、**外部crateの実importは回帰テストの対象外**。導入時には実依存を使う違反ケースも追加すること。

### 現状の見逃しを再現する例

```rust
// use文がないため、このルールでは検査されない。
pub fn architecture_probe() {
    let _repository = crate::infrastructure::InMemoryOrderRepository::default();
}
```

```rust
// lib.rs: 元の型を、層名を含まない別名で公開
pub use infrastructure::InMemoryOrderRepository as Database;

// domain.rs: useパスにはinfrastructureという語がない
pub use crate::Database;
```

## この結果をどう使うか

**cargo-pupは設計レビューを補助するガードレールで、完全な境界保証として単独採用しない。** この設定では完全修飾パスや再export先の型・関数の解決、全依存の循環検出までは保証しない。cfg / feature / platformによって未コンパイルのコードも別途検査が必要。

実サービスではcrate分割とRustの可視性で境界を作り、必要に応じて `cargo metadata` に基づく直接依存のallowlist検査も足すのが次の実験候補。traitや可視性のcargo-pupルールは今回の初期版の対象外。上流で未実装の `ModuleRule::And / Or / Not` にも依存しない（ModuleMatchの論理条件とは別物）。

CIはPRごとに、通常のテスト・Clippy・formatと、cargo-pup本体の検査・ルールの回帰テストを実行する設定。nightlyや上流commitを更新する際は、**正しいコードだけでなく違反10件と見逃し2件の変化**を確認する。

## 参照

- [cargo-pup README（固定commit）](https://github.com/DataDog/cargo-pup/tree/a2c06497096123d4d37f622ddadb934831c60e92)
- [RestrictImportsと未実装の論理ルール](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_impl/src/lints/module_lint/lint.rs)
- [nightlyの指定](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/rust-toolchain.toml)

このサンプルのライセンスは既存の [MIT LICENSE](LICENSE)。cargo-pup自体は上流のApache-2.0ライセンスに従う。
