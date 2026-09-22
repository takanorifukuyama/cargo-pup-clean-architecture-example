# 複数crate版：宣言された依存関係を契約にする

ルートの単一crateサンプルを置き換えず、同じタスク管理CLIを5つのcrateへ分割した比較用サンプルです。既存と同じ9件の業務・CLIテストを用意しています。コードの重複は比較のためのもので、共通Repository契約テストへの統合は別の拡張です。

```text
task-cli（composition root）
  ├── task-presentation → task-application → task-domain
  │                  └───────────────────→ task-domain
  └── task-infrastructure ────────────────→ task-domain
```

| package | 通常依存の許可 | 開発用依存の許可 | ビルド用依存の許可 |
| --- | --- | --- | --- |
| task-domain | なし | なし | なし |
| task-application | task-domain | なし | なし |
| task-infrastructure | task-domain | なし | なし |
| task-presentation | task-application、task-domain | なし | なし |
| task-cli | task-application、task-infrastructure、task-presentation | task-domain | なし |

portをdomainに置く方針はこのサンプルの設計判断で、あらゆるClean Architectureに強制するものではありません。CLIの開発用task-domainは統合テスト向けであり、通常コードでの直接依存は許可しません。

## ルールの本体

[`../../tools/dependency-contracts/tests/dependencies.rs`](../../tools/dependency-contracts/tests/dependencies.rs) の `crate_dependencies!` が通常の `#[test]` を生成します。実際にはこのファイルはリポジトリルートの `tools/dependency-contracts/tests/dependencies.rs` にあります。

```rust
crate_dependencies! {
    application_uses_domain_only {
        package: "task-application",
        allow_normal: ["task-domain"],
        allow_dev: [],
        allow_build: [],
    }
}
```

検査は `cargo metadata --format-version 1 --no-deps --locked --offline` の **`packages[].dependencies`** を使います。依存をまだ使っていなくても、宣言した時点で契約に違反すれば失敗します。

`resolve.nodes` だけを見る方法とは異なり、未有効のoptional依存やLinuxではビルドされないWindows向け依存も省略しません。通常・開発用・ビルド用は別々に判定します。これは依存宣言の検査であり、Windows上で実際に動くことを試したという意味ではありません。

`name` はパッケージ名、`rename` は利用側の別名です。別名で許可リストを迂回できず、逆に許可されたパッケージの別名指定は通ります。`workspace = true` で継承した依存も検査します。

このサンプルの許可名は **同じworkspaceにあるpackage** を指します。canonicalなローカルディレクトリも照合するため、別のパスにある同名packageやregistry/gitの同名packageは代替になりません。外部packageを積極的に許可するためのsource/version指定DSLは、今回の範囲外です。

全workspace memberに一つのルールを要求します。未分類のcrate追加、ルール対象の誤字、重複、存在しない許可先を失敗させます。metadataの欠損や未知の依存種別も黙って無視しません。

## 検査自体の対照実験

各ケースで新しい一時コピーを作ります。元のソースやCargo.lockは変更しません。

1. Cargo.tomlへ禁止された依存を追加する。
2. 一時コピー内だけでlockを更新し、通常の `cargo check --workspace --all-targets` が成功することを確認する。
3. 同じコピーのmetadataを取得する。
4. 期待したルール・依存元・依存先・種別の `Problem::Forbidden` が出ることを確認する。

「コンパイルに失敗しただけ」「Cargoの循環依存でmetadataが作れないだけ」は契約テストの成功ではありません。domainの禁止依存には、循環を作らないローカルの `forbidden-io` ダミーpackageを使います。実DBやHTTPライブラリを接続するテストではありません。

正常対照には許可された依存の別名指定・workspace継承を含めます。同名の異なるローカルpackageは実際のCargo graphで検査し、非ローカルsourceのケースはmetadataを加工する評価器の単体テストとして区別しています。

別のコンパイラ対照実験では、依存宣言なしでdomainから `task_infrastructure::InMemoryTaskRepository` を完全修飾で参照し、JSON診断の `E0433` と対象名を確認します。これは **crate分割によるコンパイル境界** の検証で、cargo-pupの解析精度を変えたものではありません。

## 実行

以下はリポジトリのルートから実行します。`rustup` とstable Rustが必要です。

```sh
cargo test --manifest-path examples/multi-crate/Cargo.toml --workspace --all-targets --locked
cargo test --manifest-path tools/dependency-contracts/Cargo.toml --locked

# 1件だけ実行
cargo test --manifest-path tools/dependency-contracts/Cargo.toml --locked inactive_optional_dependency_is_rejected -- --exact

cargo fmt --manifest-path examples/multi-crate/Cargo.toml --all -- --check
cargo fmt --manifest-path tools/dependency-contracts/Cargo.toml --all -- --check
```

ルートの `cargo test` は、この独立workspaceまで自動では巡回しません。新しい [GitHub Actions workflow](../../.github/workflows/dependency-contracts.yml) が明示的に両方のコマンドを実行します。cargo-pup・nightly・Pythonは追加検査には不要です。

ログはルートの `.test-artifacts/dependency-contracts/` にケース別で保存し、Actions Artifactで7日間保持します。コンパイラ処理は共通の180秒タイムアウト付きランナーを利用します。CIの必須チェックにする際は、既存チェックに加えて `Crate dependency contracts` と `Multi-crate formatting` を指定してください。リポジトリの保護設定は変更しません。

## 限界

今回は宣言された **直接依存** の契約です。間接依存の到達性や依存先内部の利用方法を追跡しません。全workspace memberの直接依存を検査しても、一般的な依存グラフ解析と同じ保証にはなりません。

同一crate内の参照、許可された依存先を経由する再export、公開APIに含まれる型、コード生成・任意のbuild scriptによる挙動、全feature/OSでの実行、悪意のあるルール改変を防ぐセキュリティ境界は範囲外です。workspaceから意図的に除外したpackageへのルール強制も今回の仕組みでは行いません。

## 一次資料

- [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html)
- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Specifying Dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
