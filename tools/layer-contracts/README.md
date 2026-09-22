# 同一crateの解決済みレイヤー契約

`use`の文字列だけでなく、**Rustコンパイラが解決した参照先**で同一crate内の依存方向を検査する実験用ツールです。既存のcargo-pup版を置き換えず、独立した検査として追加しています。

マクロ・ポリシー評価・コンパイラ連携はこのサンプルの自作実装です。cargo-pupの機能追加やupstream修正ではありません。外部crate依存はなく、評価エンジンはstable Rust、参照の収集だけは固定nightlyの`rustc_private`を使います。

## ルールを書く

実際のサンプル用ルールは [tests/resolved.rs](tests/resolved.rs) の先頭にあります。

```rust
use layer_contracts::layer_rules;

layer_rules! {
    CLEAN {
        acyclic: true,
        root("clean_architecture") => [domain, application, infrastructure, presentation],
        domain("clean_architecture::domain") => [],
        application("clean_architecture::application") => [domain],
        infrastructure("clean_architecture::infrastructure") => [domain],
        presentation("clean_architecture::presentation") => [application, domain],
    }
}
```

`layer_rules!`は`Policy`定数を生成します。収集した`Graph`に対して、通常のRustテストから`CLEAN.check(&graph)`を実行します。このマクロだけでコンパイラを実行したり、テストを登録したりするわけではありません。違反例は別の`resolved_violation!`から通常の`#[test]`を生成します。

矢印の右側は**直接参照してよいレイヤー**です。`domain => []`は他のレイヤーへの参照禁止。同一レイヤー内の参照は許可します。`root`という名前は任意で、crate rootに置いた宣言の参照範囲も明示的に定義できます。

## 追加した検査

| 検査 | 内容 |
| --- | --- |
| 解決済みの依存方向 | 完全修飾パス、import、別名、グループ・glob import、再export、型・trait境界などの参照先を評価 |
| メソッド・推論型 | 解決したメソッド、式の具体型と一部の型引数も収集。facadeが返す具体型の利用を検査 |
| 循環依存 | `acyclic: true`でレイヤー間の循環を拒否。個々の方向を許可していても循環があれば失敗 |
| 分類漏れ防止 | 収集した全モジュールに所属レイヤーを要求。未分類や重複・重なった指定を拒否 |
| 空振り防止 | 対象モジュールが存在しないレイヤーは失敗。誤字やcfgで消えた対象を成功扱いにしない |
| 診断・グラフ | 依存元・依存先・参照シンボル・ソース位置を保持。DOTと機械可読のグラフを出力 |

通常のレイヤー指定は、そのモジュールと`::`で区切られた子モジュールに一致します。`domain`は`domain_extra`に一致しません。**crate rootの指定だけはrootに限定**し、未知のトップレベルモジュールを全部吸収する扱いにはしません。

循環の検査はレイヤー単位であり、同一レイヤー内のモジュール循環は対象外です。循環が複数あれば、まず一つの決定的な経路を報告します。全循環列挙ではありません。

## コンパイラ連携の仕組み

[driver.rs](src/driver.rs)は、固定コンパイラの`LateLintPass`で名前解決済みのパスと式の型情報を収集します。参照先の`DefId`から所属モジュールを調べるため、別名や`pub use`経由でも、元の型・関数へ解決された参照を検査できます。

```text
検査対象を通常のrustcでコンパイル
  → 同じ引数のコンパイラ連携で参照を収集
  → 完了したグラフを検証・読み込み
  → 全モジュールをレイヤーに分類
  → 許可方向と循環のポリシーを評価
```

通常コンパイルの失敗、コンパイラ連携の異常終了、ツール未導入、途中までしか出力されなかったグラフは、すべてテスト失敗です。これらを「設計違反を検出できた」として扱いません。違反ケースでは、通常コンパイルとグラフ収集に成功したうえで、期待した依存元・依存先の`Finding::Forbidden`を確認します。

出力は一時ディレクトリ内の新しいファイルに限定します。以前のグラフを再利用せず、各ケースのログも前回のPASSを残さないよう作り直します。処理は既存の180秒タイムアウト付き実行ヘルパーを共有しています。

## 比較用テスト

[tests/resolved.rs](tests/resolved.rs)と[tests/resolved/extra.rs](tests/resolved/extra.rs)を参照してください。

完全修飾パスの呼び出し・戻り値、別名・glob import、facade・crate root・多段再export、メソッド、型引数、trait境界、型aliasの推論、展開後の宣言マクロ、ネストしたモジュールを小さなfixtureで確認します。同名のローカル型、文字列やコメント、許可方向の参照は正常例として別に検証します。

さらに**元のサンプルの`src/`をコピーし、cargo-pup版で見逃していた`known_gap()`をそのまま追加するテスト**を用意しています。既存のcargo-pup版の`KNOWN GAP`テストは残し、新しい検査との違いを比較します。

featureの例では、同じソースを`leak`なし・ありでコンパイルし、後者だけで追加される違反を検査します。これは指定した構成の比較であり、すべてのfeature組み合わせを自動列挙しているわけではありません。

## 実行方法

リポジトリのルートから実行します。`rustup`が必要です。

```sh
# 評価エンジンの単体テストのみ。nightly不要
cargo +stable test --manifest-path tools/layer-contracts/Cargo.toml --locked

# コンパイラ連携の準備
compiler=$(cat tools/layer-contracts/toolchain.txt)
rustup toolchain install "$compiler" --profile minimal \
  --component rustc-dev --component llvm-tools-preview
cargo +"$compiler" build --manifest-path tools/layer-contracts/Cargo.toml \
  --locked --features compiler --bin layer-driver

# 実サンプルとコンパイラの対照実験
cargo +stable test --manifest-path tools/layer-contracts/Cargo.toml \
  --locked --features integration --test resolved

# 一件だけ
cargo +stable test --manifest-path tools/layer-contracts/Cargo.toml \
  --locked --features integration --test resolved \
  extra::original_cargo_pup_known_gap_is_rejected_by_new_collector -- --exact

# 整形・stable側のClippy
cargo fmt --manifest-path tools/layer-contracts/Cargo.toml --all -- --check
cargo +stable clippy --manifest-path tools/layer-contracts/Cargo.toml \
  --locked --lib --tests --features integration -- -D warnings
```

`compiler` featureをstableの`--all-features`で有効化すると、コンパイラ内部APIのためビルドできません。通常の評価・対照実験のランナーはstableでビルドし、外部プロセスとして固定nightlyのdriverを起動します。別の出力先にビルドした場合は`LAYER_DRIVER`にdriverのパスを指定できます。

ルートの`cargo test`は独立workspaceを自動では巡回しません。[専用workflow](../../.github/workflows/resolved-layers.yml)が明示的に実行します。元のcargo-pup CIもそのまま実行します。

## ログとArtifact

`.test-artifacts/resolved-layers/<case>/`に、通常コンパイルと収集時のログ、`graph.txt`、`graph.dot`、必要なケースの`policy.md`を保存します。`graph.txt`はバージョン付きの内部形式で、UTF-8フィールドをhexにして制御文字も保持し、終端マーカーで不完全な出力を拒否します。

`graph.dot`はGraphvizで表示できます。参照グラフはモジュール単位であり、実行時の呼び出し順序を示しません。元コードが同じでも一時ファイルのパスは実行ごとに変わるため、別実行のグラフが常にバイト単位で一致するという保証ではありません。

違反fixtureの`policy.md`にFAILと書かれているのは、そのポリシーがfixtureを拒否したという意味です。**テストが成功したかどうかはlibtestとActionsの結果**を確認してください。Artifactは7日間保持します。

## 現在の範囲と限界

これはコンパイラ連携を試すプロトタイプで、完全な依存解析器ではありません。

- 収集対象は一つのcrate内の参照です。外部crateへの依存、crateをまたぐ経路の追跡は扱いません。既存のcargo-pup・複数crateの契約と役割が異なります。
- 現在のランナーは依存のないサンプルlibraryを直接rustcに渡します。任意のCargoプロジェクト、外部crate・build.rs・外部proc macroの引数を自動で用意する汎用Cargo wrapperではありません。
- 通常の型・関数の名前解決と式の一部の具体型を使います。generic/dyn dispatchの実行先特定、関数ポインタやopaque型を含む全型の再帰的解析、型aliasの全ケースでの正規化、間接呼び出しの到達性までは保証しません。
- `pub use`の再exportが元の定義へ解決される場合は検査します。一方、型aliasを介した全シグネチャの漏出検査や、公開API全体の型検査は別機能です。
- 検査するのはその実行でコンパイルされたfeature・target構成です。無効なcfg領域や全OSでの動作は含みません。宣言マクロのfixtureはありますが、あらゆるマクロの完全な網羅を意味しません。
- 参照先の定義モジュールを所有者として扱います。別レイヤーで実装したtrait implや動的ディスパッチの実行時依存を、すべて同じように追えるわけではありません。
- テストやポリシー自体の悪意ある変更を防ぐセキュリティ境界ではありません。新機能ごとに正常例・違反例を追加して検査範囲を確認してください。

## 一次資料

- [Rust Compiler Development Guide: rustc_driver](https://rustc-dev-guide.rust-lang.org/rustc-driver/intro.html)
- [固定コンパイラのLateLintPass定義](https://github.com/rust-lang/rust/blob/eda76d9d1/compiler/rustc_lint/src/passes.rs)
- [固定コンパイラの名前解決・型検査補助API](https://github.com/rust-lang/rust/blob/eda76d9d1/compiler/rustc_lint/src/context.rs)
