# cargo-pup-clean-architecture-example

**Rustマクロで設計上の約束をテストし、わざと違反させて検出できることも確かめる**サンプルです。同じタスク管理CLIを、単一crate版と複数crate版で比較できます。

| サンプル | 主な検査 | 詳細 |
| --- | --- | --- |
| 単一crate（ルートの `src/`） | cargo-pupのimport・公開範囲、Rustの型契約、空振り防止 | [単一crate版](SINGLE_CRATE.md) |
| 複数crate（`examples/multi-crate/`） | Cargoに宣言された直接依存の許可リスト、crate間のコンパイル境界 | [複数crate版](examples/multi-crate/README.md) |

## テストを読む

- [import・公開範囲のルール](tests/architecture.rs)
- [TaskRepository / Send / Syncの型契約](tests/type_contracts.rs)
- [crate依存の許可リストと対照実験](tools/dependency-contracts/tests/dependencies.rs)

```rust
crate_dependencies! {
    domain_is_independent {
        package: "task-domain",
        allow_normal: [],
        allow_dev: [],
        allow_build: [],
    }
}
```

マクロはこのレポ内のサンプル実装です。cargo-pup公式のマクロや公開ライブラリではありません。

## 実行（リポジトリのルートから）

```sh
# 既存の単一crate版。型契約・業務・CLIテスト
cargo test --locked --all-targets

# 複数crate版。外部crate依存のないタスク管理アプリ
cargo test --manifest-path examples/multi-crate/Cargo.toml --workspace --all-targets --locked
cargo run --manifest-path examples/multi-crate/Cargo.toml -p task-cli --locked -- "Write architecture tests"

# 宣言された直接依存の検査。stableのみ、cargo-pup不要
cargo test --manifest-path tools/dependency-contracts/Cargo.toml --locked

# cargo-pupによる単一crateの設計検査
python3 scripts/setup_pup.py
cargo test --locked --features architecture-tests --test architecture
```

アプリと検査用packageは独立しています。両方のアプリは外部crate依存なし、JSON解析用の `serde_json` は `tools/dependency-contracts` のdev-dependencyだけです。初回は依存取得にネットワークが必要ですが、直接依存の対照実験自体はローカルpath dependencyだけを使います。

## 保証の違い

単一crate版の `RestrictImports` は `use` の検査です。完全修飾パスの直接参照を見逃す例は、引き続き既知の限界として残しています。

複数crate版は、宣言していない別crateの直接参照をRustコンパイラで拒否し、禁止された依存の追加をCargo metadataの契約で拒否します。ただし、同じcrate内の参照、許可されたcrateによる再export、間接依存の追跡、全feature/OSでの動作は別問題です。

CIは [.github/workflows/ci.yml](.github/workflows/ci.yml) と [dependency-contracts.yml](.github/workflows/dependency-contracts.yml) に分離しています。マージを制限するRulesetは別設定です。
